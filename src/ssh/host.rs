use std::{path::Path, sync::Mutex};

use russh::keys::{HashAlg, PublicKey, PublicKeyOrCertificate, known_hosts};
use tokio::sync::{mpsc, oneshot, watch};

use super::{Error, Prompt, prompt};

static HOSTS_LOCK: Mutex<()> = Mutex::new(());

pub(super) struct Client {
    pub host: String,
    pub port: u16,
    pub path: std::path::PathBuf,
    pub prompts: mpsc::Sender<Prompt>,
    pub cancel: watch::Receiver<bool>,
}

pub(super) struct CancelOnDrop(pub watch::Sender<bool>);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        let _ = self.0.send(true);
    }
}

fn check(host: &str, port: u16, key: &PublicKey, path: &Path) -> Result<bool, Error> {
    match std::fs::File::open(path) {
        Ok(_) => (),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(_) => return Err(Error::KnownHosts),
    }
    match known_hosts::check_known_hosts_path(host, port, key, path) {
        Ok(true) => Ok(true),
        Err(russh::keys::Error::KeyChanged { .. }) => Err(Error::HostKeyChanged),
        Err(_) => Err(Error::KnownHosts),
        Ok(false) => {
            let existing = known_hosts::known_host_keys_path(host, port, path)
                .map_err(|_| Error::KnownHosts)?;
            if existing.is_empty() {
                Ok(false)
            } else {
                Err(Error::HostKeyChanged)
            }
        }
    }
}

impl russh::client::Handler for Client {
    type Error = Error;

    async fn check_server_key(&mut self, key: &PublicKeyOrCertificate) -> Result<bool, Error> {
        let PublicKeyOrCertificate::PublicKey { key, .. } = key else {
            return Err(Error::HostCertificate);
        };
        let (host, port, path, check_key) =
            (self.host.clone(), self.port, self.path.clone(), key.clone());
        let known = tokio::task::spawn_blocking(move || {
            let _guard = HOSTS_LOCK.lock().map_err(|_| Error::KnownHosts)?;
            check(&host, port, &check_key, &path)
        })
        .await
        .map_err(|_| Error::KnownHosts)??;
        if known {
            return Ok(true);
        }
        let (reply, response) = oneshot::channel();
        let response = prompt(
            &self.prompts,
            Prompt::HostKey {
                host: self.host.clone(),
                port: self.port,
                algorithm: key.algorithm().to_string(),
                fingerprint: key.fingerprint(HashAlg::Sha256).to_string(),
                reply,
            },
            response,
        );
        let trusted = tokio::select! {
            result = response => result?,
            _ = self.cancel.changed() => return Err(Error::Cancelled),
        };
        if !trusted {
            return Err(Error::Cancelled);
        }
        let (host, port, path, key) =
            (self.host.clone(), self.port, self.path.clone(), key.clone());
        tokio::task::spawn_blocking(move || {
            let _guard = HOSTS_LOCK.lock().map_err(|_| Error::KnownHosts)?;
            if !check(&host, port, &key, &path)? {
                known_hosts::learn_known_hosts_path(&host, port, &key, path)
                    .map_err(|_| Error::KnownHosts)?;
            }
            Ok::<_, Error>(true)
        })
        .await
        .map_err(|_| Error::KnownHosts)?
    }
}
