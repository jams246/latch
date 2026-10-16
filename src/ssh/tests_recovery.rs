use std::{sync::atomic::Ordering, time::Duration};

use tokio::sync::mpsc;

use super::{
    fixture::{Fixture, key},
    tests::{config, connected, password, start, trust},
    *,
};

pub(super) async fn status(events: &mut mpsc::Receiver<Event>, expected: State) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match events.recv().await.expect("Expected lifecycle event") {
                Event::Status(state) if state == expected => return,
                Event::Error(error) => panic!("Unexpected connection error: {error}"),
                _ => (),
            }
        }
    })
    .await
    .unwrap();
}

async fn input(control: &SessionControl, bytes: &[u8]) {
    control
        .commands
        .send(Command::Input(bytes.to_vec()))
        .await
        .unwrap();
}

#[tokio::test]
async fn interrupted_terminal_reuses_password_and_latest_dimensions() {
    for interruption in [b"synthetic-drop".as_slice(), b"synthetic-shell-close"] {
        let fixture = Fixture::start(false).await;
        let (control, mut prompts, mut events) = start(config(&fixture, "recover-terminal"));
        trust(&mut prompts).await;
        password(&mut prompts, "test-only-password", false).await;
        connected(&mut events).await;
        control
            .commands
            .send(Command::Resize {
                cols: 103,
                rows: 37,
            })
            .await
            .unwrap();
        input(&control, interruption).await;
        status(&mut events, State::Reconnecting).await;
        connected(&mut events).await;
        assert!(prompts.try_recv().is_err());
        assert_eq!(fixture.counts.passwords.load(Ordering::SeqCst), 2);
        assert_eq!(fixture.counts.ptys.load(Ordering::SeqCst), 2);
        assert_eq!(fixture.counts.cols.load(Ordering::SeqCst), 103);
        assert_eq!(fixture.counts.rows.load(Ordering::SeqCst), 37);
        input(&control, b"recovered shell").await;
        status_after_disconnect(&control, &mut events).await;
    }
}

async fn status_after_disconnect(control: &SessionControl, events: &mut mpsc::Receiver<Event>) {
    control.commands.send(Command::Disconnect).await.unwrap();
    status(events, State::Disconnected).await;
    assert!(events.try_recv().is_err());
}

#[tokio::test]
async fn clean_remote_shell_exit_does_not_reconnect() {
    for command in [b"synthetic-exit".as_slice(), b"synthetic-eof-exit"] {
        let fixture = Fixture::start(false).await;
        let (control, mut prompts, mut events) = start(config(&fixture, "clean-exit"));
        trust(&mut prompts).await;
        password(&mut prompts, "test-only-password", false).await;
        connected(&mut events).await;
        input(&control, command).await;
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                match events.recv().await.unwrap() {
                    Event::Status(State::Disconnected) => break,
                    Event::Status(State::Reconnecting) => panic!("Clean exit retried"),
                    _ => (),
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(fixture.counts.passwords.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test]
async fn encrypted_key_recovery_does_not_prompt_for_passphrase_again() {
    let fixture = Fixture::start(false).await;
    let mut config = config(&fixture, "recover-key");
    let path = config.known_hosts.with_file_name("synthetic-encrypted-key");
    let encrypted = key(2)
        .encrypt_with(
            russh::keys::ssh_key::Cipher::Aes256Ctr,
            russh::keys::ssh_key::Kdf::Bcrypt {
                salt: vec![7; 16],
                rounds: 1,
            },
            7,
            "test-only-passphrase",
        )
        .unwrap();
    std::fs::write(
        &path,
        encrypted
            .to_openssh(russh::keys::ssh_key::LineEnding::LF)
            .unwrap(),
    )
    .unwrap();
    config.auth = AuthMethod::KeyFile(path);
    let (control, mut prompts, mut events) = start(config);
    trust(&mut prompts).await;
    let Prompt::Secret {
        kind: SecretKind::KeyPassphrase { .. },
        reply,
        ..
    } = super::tests::next_prompt(&mut prompts).await
    else {
        panic!("Expected passphrase")
    };
    assert!(
        reply
            .send(Some(Secret::new("test-only-passphrase".into())))
            .is_ok()
    );
    connected(&mut events).await;
    input(&control, b"synthetic-drop").await;
    status(&mut events, State::Reconnecting).await;
    connected(&mut events).await;
    assert!(prompts.try_recv().is_err());
    assert_eq!(fixture.counts.passwords.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.counts.shells.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn changed_host_identity_never_receives_cached_password_on_recovery() {
    let fixture = Fixture::start(false).await;
    let config = config(&fixture, "recover-changed-key");
    let known_hosts = config.known_hosts.clone();
    let (control, mut prompts, mut events) = start(config);
    trust(&mut prompts).await;
    password(&mut prompts, "test-only-password", false).await;
    connected(&mut events).await;
    std::fs::write(
        &known_hosts,
        format!(
            "[127.0.0.1]:{} {}\n",
            fixture.port,
            key(3).public_key().to_openssh().unwrap(),
        ),
    )
    .unwrap();
    input(&control, b"synthetic-drop").await;
    status(&mut events, State::Reconnecting).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(prompts.try_recv().is_err());
    assert_eq!(fixture.counts.passwords.load(Ordering::SeqCst), 1);
    while let Ok(event) = events.try_recv() {
        assert!(!matches!(
            event,
            Event::Status(State::Connected | State::Failed) | Event::Error(_)
        ));
    }
    status_after_disconnect(&control, &mut events).await;
}

#[tokio::test]
async fn recovery_can_be_cancelled_or_dropped_without_more_attempts() {
    for disconnect in [true, false] {
        let fixture = Fixture::start(false).await;
        let (control, mut prompts, mut events) = start(config(&fixture, "cancel-recovery"));
        trust(&mut prompts).await;
        password(&mut prompts, "test-only-password", false).await;
        connected(&mut events).await;
        fixture.disconnect_clients();
        status(&mut events, State::Reconnecting).await;
        if disconnect {
            status_after_disconnect(&control, &mut events).await;
        }
        drop(control);
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(fixture.counts.active.load(Ordering::SeqCst), 0);
        assert_eq!(fixture.counts.passwords.load(Ordering::SeqCst), 1);
    }
}
