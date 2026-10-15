use super::{
    fixture::{Fixture, key},
    tests::{config, connected, next_prompt, start, trust},
    *,
};
use std::sync::atomic::Ordering;

#[tokio::test]
async fn key_files_authenticate_with_only_required_passphrase_prompts() {
    for encrypted in [false, true] {
        let fixture = Fixture::start(false).await;
        let mut config = config(&fixture, if encrypted { "encrypted" } else { "key" });
        let path = config.known_hosts.with_file_name("synthetic_ed25519");
        let private = if encrypted {
            key(2)
                .encrypt_with(
                    russh::keys::ssh_key::Cipher::Aes256Ctr,
                    russh::keys::ssh_key::Kdf::Bcrypt {
                        salt: vec![7; 16],
                        rounds: 1,
                    },
                    7,
                    "test-only-passphrase",
                )
                .unwrap()
        } else {
            key(2)
        };
        std::fs::write(
            &path,
            private
                .to_openssh(russh::keys::ssh_key::LineEnding::LF)
                .unwrap(),
        )
        .unwrap();
        config.auth = AuthMethod::KeyFile(path);
        let (_control, mut prompts, mut events) = start(config);
        trust(&mut prompts).await;
        if encrypted {
            let Prompt::Secret {
                kind: SecretKind::KeyPassphrase { .. },
                reply,
                ..
            } = next_prompt(&mut prompts).await
            else {
                panic!("Expected key passphrase");
            };
            assert!(
                reply
                    .send(Some(Secret::new("test-only-passphrase".into())))
                    .is_ok()
            );
        }
        connected(&mut events).await;
        assert!(prompts.try_recv().is_err());
        assert_eq!(fixture.counts.passwords.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn keyboard_interactive_identifies_and_answers_challenge() {
    let fixture = Fixture::start(false).await;
    let mut config = config(&fixture, "keyboard");
    config.auth = AuthMethod::KeyboardInteractive;
    let (_control, mut prompts, mut events) = start(config);
    trust(&mut prompts).await;
    let Prompt::KeyboardInteractive {
        name,
        fields,
        reply,
        ..
    } = next_prompt(&mut prompts).await
    else {
        panic!("Expected explicit challenge");
    };
    assert_eq!(name, "Synthetic verification");
    assert_eq!(fields[0].label, "Code");
    assert!(!fields[0].echo);
    assert!(reply.send(Some(vec![Secret::new("123456".into())])).is_ok());
    connected(&mut events).await;
}
