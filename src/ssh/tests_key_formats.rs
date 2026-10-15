use std::{sync::atomic::Ordering, time::Duration};

use super::{
    fixture::{Fixture, key},
    tests::{config, start, trust},
    *,
};

// Public synthetic RustCrypto/SSH test fixture; the adjacent fixture license is retained.
const PPK: &str = include_str!("fixtures/id_ed25519_encrypted.ppk");

async fn unlock(prompts: &mut tokio::sync::mpsc::Receiver<Prompt>, value: &str, retry: bool) {
    let Prompt::Secret {
        kind: SecretKind::KeyPassphrase { .. },
        error,
        reply,
    } = tokio::time::timeout(Duration::from_secs(60), prompts.recv())
        .await
        .unwrap()
        .unwrap()
    else {
        panic!("Expected a key passphrase, not an account password");
    };
    assert_eq!(error.is_some(), retry);
    assert!(reply.send(Some(Secret::new(value.into()))).is_ok());
}

#[tokio::test]
async fn encrypted_ppk_and_pkcs8_prompt_retry_and_authenticate() {
    for ppk in [true, false] {
        let fixture = Fixture::start(false).await;
        let mut config = config(&fixture, if ppk { "ppk" } else { "pkcs8" });
        let path = config.known_hosts.with_file_name(if ppk {
            "synthetic.ppk"
        } else {
            "synthetic.pem"
        });
        let contents = if ppk {
            PPK.as_bytes().to_vec()
        } else {
            let mut encoded = Vec::new();
            russh::keys::encode_pkcs8_pem_encrypted(&key(2), b"123", 1, &mut encoded).unwrap();
            encoded
        };
        assert!(
            russh::keys::decode_secret_key(std::str::from_utf8(&contents).unwrap(), None).is_err()
        );
        std::fs::write(&path, contents).unwrap();
        config.auth = AuthMethod::KeyFile(path);
        let (_control, mut prompts, mut events) = start(config);
        trust(&mut prompts).await;
        unlock(&mut prompts, "wrong", false).await;
        unlock(&mut prompts, "123", true).await;
        tokio::time::timeout(Duration::from_secs(60), async {
            loop {
                match events.recv().await.unwrap() {
                    Event::Status(State::Connected) => return,
                    Event::Error(error) => panic!("{error}"),
                    _ => (),
                }
            }
        })
        .await
        .unwrap();
        assert!(prompts.try_recv().is_err());
        assert_eq!(fixture.counts.passwords.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn missing_or_unsupported_key_files_fail_without_credential_prompts() {
    for contents in [
        None,
        Some("synthetic-invalid-marker\nEncryption: aes256-cbc\n"),
        Some("PuTTY-User-Key-File-3: ssh-ed25519\nEncryption: unsupported\n"),
    ] {
        let fixture = Fixture::start(false).await;
        let mut config = config(&fixture, "invalid-key-format");
        let path = config.known_hosts.with_file_name("synthetic-invalid-key");
        if let Some(contents) = contents {
            std::fs::write(&path, contents).unwrap();
        }
        config.auth = AuthMethod::KeyFile(path);
        let (_control, mut prompts, mut events) = start(config);
        trust(&mut prompts).await;
        let error = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if let Some(Event::Error(error)) = events.recv().await {
                    return error;
                }
            }
        })
        .await
        .unwrap();
        assert!(error.contains("Cannot read or unlock"));
        assert!(!error.contains("synthetic-invalid-marker"));
        assert!(prompts.try_recv().is_err());
        assert_eq!(fixture.counts.passwords.load(Ordering::SeqCst), 0);
    }
}
