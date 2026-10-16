use super::{
    fixture::{Fixture, key},
    tests::{config, connected, next_prompt, start, trust},
    *,
};
use std::{sync::atomic::Ordering, time::Duration};

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
            let Prompt::Secret { reply, .. } = next_prompt(&mut prompts).await else {
                panic!("Expected key passphrase");
            };
            assert!(reply.send(Some(Secret::new("wrong".into()))).is_ok());
            let Prompt::Secret {
                kind: SecretKind::KeyPassphrase { .. },
                reply,
                error,
            } = next_prompt(&mut prompts).await
            else {
                panic!("Expected key passphrase");
            };
            assert!(error.is_some());
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
    assert_eq!(fixture.counts.challenge_responses.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn keyboard_interactive_rejects_oversized_challenges_before_prompting() {
    for username in [
        "synthetic-oversized-challenge",
        "synthetic-long-challenge",
        "synthetic-long-name",
        "synthetic-long-instructions",
    ] {
        let fixture = Fixture::start(false).await;
        let mut config = config(&fixture, username);
        config.username = username.into();
        config.auth = AuthMethod::KeyboardInteractive;
        let (_control, mut prompts, mut events) = start(config);
        trust(&mut prompts).await;
        let message = tokio::time::timeout(Duration::from_secs(5), async {
            let mut error = None;
            loop {
                match events.recv().await.expect("Expected terminal failure") {
                    Event::Error(message) => error = Some(message),
                    Event::Status(State::Failed) => break error,
                    Event::Status(State::Connected) => panic!("Unsupported challenge opened shell"),
                    _ => (),
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(message, Some(Error::Challenge.to_string()));
        assert!(prompts.try_recv().is_err());
        assert_eq!(fixture.counts.challenge_responses.load(Ordering::SeqCst), 0);
        assert_eq!(fixture.counts.passwords.load(Ordering::SeqCst), 0);
        assert_eq!(fixture.counts.ptys.load(Ordering::SeqCst), 0);
    }
}
