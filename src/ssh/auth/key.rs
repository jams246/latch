use std::{io::Read, path::Path, sync::Arc};

use russh::{client, keys::PrivateKeyWithHashAlg};
use tokio::sync::{mpsc, oneshot};
use zeroize::Zeroizing;

use crate::ssh::{Error, Prompt, Secret, SecretKind, host::Client};

async fn read_key(
    path: &Path,
    passphrase: Option<Secret>,
) -> Result<russh::keys::PrivateKey, russh::keys::Error> {
    let path = path.to_owned();
    tokio::task::spawn_blocking(move || {
        const MAX_KEY_BYTES: u64 = 1024 * 1024;
        let file = std::fs::File::open(path)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.len() > MAX_KEY_BYTES {
            return Err(russh::keys::Error::CouldNotReadKey);
        }
        let mut contents = Zeroizing::new(String::new());
        file.take(MAX_KEY_BYTES + 1).read_to_string(&mut contents)?;
        if contents.len() as u64 > MAX_KEY_BYTES {
            return Err(russh::keys::Error::CouldNotReadKey);
        }
        decode_key(&contents, passphrase.as_deref().map(String::as_str))
    })
    .await
    .map_err(|_| russh::keys::Error::CouldNotReadKey)?
}

fn decode_key(
    contents: &str,
    passphrase: Option<&str>,
) -> Result<russh::keys::PrivateKey, russh::keys::Error> {
    let decoded = russh::keys::decode_secret_key(contents, passphrase);
    let encrypted = contents
        .trim_start()
        .starts_with("-----BEGIN ENCRYPTED PRIVATE KEY-----")
        || (contents.trim_start().starts_with("PuTTY-User-Key-File-")
            && contents
                .lines()
                .any(|line| line.trim_end() == "Encryption: aes256-cbc"));
    // Supported loaders do not consistently classify a missing passphrase themselves.
    if decoded.is_err() && passphrase.is_none() && encrypted {
        Err(russh::keys::Error::KeyIsEncrypted)
    } else {
        decoded
    }
}

pub(super) async fn authenticate(
    handle: &mut client::Handle<Client>,
    username: &str,
    path: &Path,
    prompts: &mpsc::Sender<Prompt>,
    cached: &mut Option<Secret>,
) -> Result<(), Error> {
    let private = match read_key(path, cached.clone()).await {
        Ok(key) => key,
        Err(russh::keys::Error::KeyIsEncrypted) => {
            let mut error = None;
            loop {
                let (reply, response) = oneshot::channel();
                let secret = crate::ssh::prompt(
                    prompts,
                    Prompt::Secret {
                        kind: SecretKind::KeyPassphrase {
                            path: path.to_owned(),
                        },
                        error,
                        reply,
                    },
                    response,
                )
                .await?
                .ok_or(Error::Cancelled)?;
                match read_key(path, Some(secret.clone())).await {
                    Ok(key) => {
                        *cached = Some(secret);
                        break key;
                    }
                    Err(_) => {
                        error = Some("Cannot unlock this key. Try the passphrase again.".into())
                    }
                }
            }
        }
        Err(_) => return Err(Error::PrivateKey),
    };
    let hash = handle.best_supported_rsa_hash().await?.flatten();
    let result = handle
        .authenticate_publickey(
            username,
            PrivateKeyWithHashAlg::new(Arc::new(private), hash),
        )
        .await?;
    if result.success() {
        Ok(())
    } else {
        Err(Error::Authentication)
    }
}
