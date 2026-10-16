use std::{path::PathBuf, sync::atomic::Ordering, time::Duration};

use tokio::sync::mpsc;

use super::{
    fixture::{Fixture, key},
    *,
};

pub(super) fn config(fixture: &Fixture, tag: &str) -> ConnectionConfig {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".verification/latch-plan/ssh-tests")
        .join(format!("{tag}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&path).unwrap();
    ConnectionConfig {
        name: "Synthetic fixture".into(),
        host: "127.0.0.1".into(),
        port: fixture.port,
        username: "synthetic".into(),
        auth: AuthMethod::Password,
        known_hosts: path.join("known_hosts"),
        cols: 80,
        rows: 24,
    }
}

pub(super) async fn next_prompt(prompts: &mut mpsc::Receiver<Prompt>) -> Prompt {
    tokio::time::timeout(Duration::from_secs(5), prompts.recv())
        .await
        .unwrap()
        .unwrap()
}

pub(super) async fn connected(events: &mut mpsc::Receiver<Event>) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match events.recv().await.unwrap() {
                Event::Status(State::Connected) => break,
                Event::Error(error) => panic!("{error}"),
                _ => (),
            }
        }
    })
    .await
    .unwrap();
}

pub(super) async fn trust(prompts: &mut mpsc::Receiver<Prompt>) {
    let Prompt::HostKey {
        algorithm,
        fingerprint,
        reply,
        ..
    } = next_prompt(prompts).await
    else {
        panic!("Expected host trust before credentials");
    };
    assert_eq!(algorithm, "ssh-ed25519");
    assert!(fingerprint.starts_with("SHA256:"));
    reply.send(true).unwrap();
}

pub(super) async fn password(prompts: &mut mpsc::Receiver<Prompt>, value: &str, retry: bool) {
    let Prompt::Secret {
        kind: SecretKind::Password,
        error,
        reply,
    } = next_prompt(prompts).await
    else {
        panic!("Expected password prompt");
    };
    assert_eq!(error.is_some(), retry);
    assert!(reply.send(Some(Secret::new(value.into()))).is_ok());
}

pub(super) fn start(
    config: ConnectionConfig,
) -> (
    SessionControl,
    mpsc::Receiver<Prompt>,
    mpsc::Receiver<Event>,
) {
    let (prompts, receiver) = mpsc::channel(8);
    let (events, output) = mpsc::channel(64);
    (spawn(config, prompts, events), receiver, output)
}

#[tokio::test]
async fn unknown_host_requires_trust_then_password_and_shell_acknowledgement() {
    let fixture = Fixture::start(false).await;
    let config = config(&fixture, "trust");
    let known = config.known_hosts.clone();
    let (control, mut prompts, mut events) = start(config.clone());
    trust(&mut prompts).await;
    control
        .commands
        .send(Command::Resize {
            cols: 110,
            rows: 40,
        })
        .await
        .unwrap();
    assert_eq!(fixture.counts.passwords.load(Ordering::SeqCst), 0);
    password(&mut prompts, "test-only-password", false).await;
    connected(&mut events).await;
    assert_eq!(fixture.counts.ptys.load(Ordering::SeqCst), 1);
    assert!(known.exists());
    control
        .commands
        .send(Command::Resize {
            cols: 100,
            rows: 30,
        })
        .await
        .unwrap();
    control
        .commands
        .send(Command::Input(b"echo test\r".to_vec()))
        .await
        .unwrap();
    let mut bytes = Vec::new();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !bytes.windows(10).any(|part| part == b"echo test\r") {
            if let Some(Event::Output(data)) = events.recv().await {
                bytes.extend(data);
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(fixture.counts.resizes.load(Ordering::SeqCst), 2);
    control.commands.send(Command::Disconnect).await.unwrap();
    let (_second, mut second_prompts, mut second_events) = start(config);
    password(&mut second_prompts, "test-only-password", false).await;
    connected(&mut second_events).await;
}

#[tokio::test]
async fn changed_key_blocks_before_any_credential_prompt() {
    let fixture = Fixture::start(false).await;
    let config = config(&fixture, "changed");
    russh::keys::known_hosts::learn_known_hosts_path(
        &config.host,
        config.port,
        key(3).public_key(),
        &config.known_hosts,
    )
    .unwrap();
    let (_control, mut prompts, mut events) = start(config);
    let mut message = String::new();
    tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(event) = events.recv().await {
            if let Event::Error(error) = event {
                message = error;
                break;
            }
        }
    })
    .await
    .unwrap();
    assert!(message.contains("host key changed"), "{message}");
    assert!(prompts.try_recv().is_err());
    assert_eq!(fixture.counts.passwords.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn password_failure_prompts_again_without_reusing_secret() {
    let fixture = Fixture::start(false).await;
    let (_control, mut prompts, mut events) = start(config(&fixture, "retry"));
    trust(&mut prompts).await;
    password(&mut prompts, "wrong", false).await;
    password(&mut prompts, "test-only-password", true).await;
    connected(&mut events).await;
    assert_eq!(fixture.counts.passwords.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn cancelling_host_confirmation_releases_network_session() {
    let fixture = Fixture::start(false).await;
    let (control, mut prompts, _events) = start(config(&fixture, "cancel"));
    let pending = next_prompt(&mut prompts).await;
    drop(control);
    tokio::time::timeout(Duration::from_secs(5), async {
        while fixture.counts.active.load(Ordering::SeqCst) != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    drop(pending);
}

#[tokio::test]
async fn excessive_output_before_shell_acknowledgement_fails_without_connecting() {
    let fixture = Fixture::start(false).await;
    let mut config = config(&fixture, "startup-output-limit");
    config.username = "synthetic-output-flood".into();
    let (control, mut prompts, mut events) = start(config);
    trust(&mut prompts).await;
    password(&mut prompts, "test-only-password", false).await;
    let error = tokio::time::timeout(Duration::from_secs(5), async {
        let mut error = None;
        loop {
            match events.recv().await.expect("Expected terminal failure") {
                Event::Error(message) => error = Some(message),
                Event::Status(State::Failed) => break error,
                Event::Status(State::Connected) => panic!("Flooding server opened a shell"),
                _ => (),
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(error, Some(Error::ChannelOutputLimit.to_string()));
    assert!(control.commands.is_closed());
}
