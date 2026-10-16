use std::{io::Read, path::Path, sync::Arc};

use russh::{client, keys::PrivateKeyWithHashAlg};
use tokio::sync::{mpsc, oneshot};
use zeroize::Zeroizing;

use super::{
    AuthMethod, ChallengeField, ConnectionConfig, Error, Prompt, SecretKind, host::Client,
};

pub(super) async fn authenticate(
    handle: &mut client::Handle<Client>,
    config: &ConnectionConfig,
    prompts: &mpsc::Sender<Prompt>,
) -> Result<(), Error> {
    match &config.auth {
        AuthMethod::Password => password(handle, &config.username, prompts).await,
        AuthMethod::KeyFile(path) => key(handle, &config.username, path, prompts).await,
        AuthMethod::Agent => super::agent::authenticate(handle, &config.username).await,
        AuthMethod::KeyboardInteractive => keyboard(handle, &config.username, prompts).await,
    }
}

async fn password(
    handle: &mut client::Handle<Client>,
    username: &str,
    prompts: &mpsc::Sender<Prompt>,
) -> Result<(), Error> {
    let mut error = None;
    loop {
        let (reply, response) = oneshot::channel();
        let secret = super::prompt(
            prompts,
            Prompt::Secret {
                kind: SecretKind::Password,
                error,
                reply,
            },
            response,
        )
        .await?
        .ok_or(Error::Cancelled)?;
        let result = handle
            .authenticate_password(username, secret.as_str())
            .await?;
        drop(secret);
        if result.success() {
            return Ok(());
        }
        error = Some("Password rejected. Enter it again to retry.".into());
    }
}

async fn read_key(
    path: &Path,
    passphrase: Option<super::Secret>,
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

async fn key(
    handle: &mut client::Handle<Client>,
    username: &str,
    path: &Path,
    prompts: &mpsc::Sender<Prompt>,
) -> Result<(), Error> {
    let private = match read_key(path, None).await {
        Ok(key) => key,
        Err(russh::keys::Error::KeyIsEncrypted) => {
            let mut error = None;
            loop {
                let (reply, response) = oneshot::channel();
                let secret = super::prompt(
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
                match read_key(path, Some(secret)).await {
                    Ok(key) => break key,
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

async fn keyboard(
    handle: &mut client::Handle<Client>,
    username: &str,
    prompts: &mpsc::Sender<Prompt>,
) -> Result<(), Error> {
    use client::KeyboardInteractiveAuthResponse as Response;
    let mut response = handle
        .authenticate_keyboard_interactive_start(username, None)
        .await?;
    for _ in 0..16 {
        match response {
            Response::Success => return Ok(()),
            Response::Failure { .. } => return Err(Error::Authentication),
            Response::InfoRequest {
                name,
                instructions,
                prompts: fields,
            } => {
                if name.len() > 4096
                    || instructions.len() > 4096
                    || fields.len() > 16
                    || fields.iter().any(|field| field.prompt.len() > 4096)
                {
                    return Err(Error::Challenge);
                }
                let count = fields.len();
                let (reply, receiver) = oneshot::channel();
                let values = super::prompt(
                    prompts,
                    Prompt::KeyboardInteractive {
                        name,
                        instructions,
                        fields: fields
                            .into_iter()
                            .map(|field| ChallengeField {
                                label: field.prompt,
                                echo: field.echo,
                            })
                            .collect(),
                        reply,
                    },
                    receiver,
                )
                .await?
                .ok_or(Error::Cancelled)?;
                if values.len() != count {
                    return Err(Error::Challenge);
                }
                // russh owns its authentication response copies until transmission finishes.
                let answers = values.iter().map(|value| value.to_string()).collect();
                response = handle
                    .authenticate_keyboard_interactive_respond(answers)
                    .await?;
            }
        }
    }
    Err(Error::Challenge)
}
