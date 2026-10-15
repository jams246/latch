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
    pub closed: watch::Sender<bool>,
    pub disconnected: watch::Receiver<bool>,
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.closed.send(true);
    }
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
            _ = self.disconnected.changed() => return Err(Error::Transport),
        };
        if !trusted {
            return Err(Error::Cancelled);
        }
        let (host, port, path, key) =
            (self.host.clone(), self.port, self.path.clone(), key.clone());
        tokio::task::spawn_blocking(move || {
            let _guard = HOSTS_LOCK.lock().map_err(|_| Error::KnownHosts)?;
            if !check(&host, port, &key, &path)? {
                known_hosts::learn_known_hosts_path(&host, port, &key, &path)
                    .map_err(|_| Error::KnownHosts)?;
                // russh's writer flushes on drop; verify the record before sending credentials.
                if !check(&host, port, &key, &path)? {
                    return Err(Error::KnownHosts);
                }
            }
            Ok::<_, Error>(true)
        })
        .await
        .map_err(|_| Error::KnownHosts)?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path() -> std::path::PathBuf {
        let directory = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(".verification/latch-plan/ssh-host-tests")
            .join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&directory).unwrap();
        directory.join("known_hosts")
    }

    #[test]
    fn host_identity_is_matched_by_both_host_and_port() {
        let path = path();
        let key = super::super::fixture::key(1);
        let key = PublicKey::new(key.public_key().key_data().clone(), "");
        known_hosts::learn_known_hosts_path("synthetic.example", 2222, &key, &path).unwrap();
        assert!(check("synthetic.example", 2222, &key, &path).unwrap());
        assert!(!check("synthetic.example", 22, &key, &path).unwrap());
        assert!(!check("other.example", 2222, &key, &path).unwrap());
    }

    #[test]
    fn hashed_openssh_host_keys_use_established_matching() {
        let path = path();
        let public = "AAAAC3NzaC1lZDI1NTE5AAAAILIG2T/B0l0gaqj3puu510tu9N1OkQ4znY3LYuEm5zCF";
        std::fs::write(&path, format!(
            "|1|O33ESRMWPVkMYIwJ1Uw+n877jTo=|nuuC5vEqXlEZ/8BXQR7m619W6Ak= ssh-ed25519 {public}\n"
        )).unwrap();
        let key = russh::keys::parse_public_key_base64(public).unwrap();
        assert!(check("example.com", 22, &key, &path).unwrap());
        assert!(!check("example.com", 2222, &key, &path).unwrap());
    }

    #[test]
    fn invalid_known_hosts_storage_blocks_instead_of_becoming_unknown() {
        let path = path();
        std::fs::create_dir(&path).unwrap();
        let key = super::super::fixture::key(1);
        assert!(matches!(
            check("synthetic.example", 22, key.public_key(), &path),
            Err(Error::KnownHosts)
        ));
    }
}
