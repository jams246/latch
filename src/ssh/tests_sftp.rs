use std::{sync::atomic::Ordering, time::Duration};

use tokio::sync::{mpsc, oneshot};

use super::{
    fixture::Fixture,
    tests::{config, connected, next_prompt, password, trust},
    *,
};
use crate::sftp::Browser;

fn subsystem(stream: russh::ChannelStream<russh::server::Msg>) {
    tokio::spawn(crate::sftp::tests_server::serve(stream));
}

fn start(
    config: ConnectionConfig,
) -> (
    SessionControl,
    mpsc::Receiver<Prompt>,
    mpsc::Receiver<Event>,
) {
    let (prompts, receiver) = mpsc::channel(8);
    let (events, output) = mpsc::channel(64);
    (spawn_sftp(config, prompts, events), receiver, output)
}

async fn stopped(events: &mut mpsc::Receiver<Event>, fixture: &Fixture, state: State) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !matches!(events.recv().await.expect("Expected final state"), Event::Status(value) if value == state) {}
        while fixture.counts.active.load(Ordering::SeqCst) != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}

async fn rejected_open(control: &SessionControl) {
    let (reply, receiver) = oneshot::channel();
    control
        .commands
        .send(Command::OpenSftp(reply))
        .await
        .unwrap();
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(5), receiver)
            .await
            .unwrap()
            .unwrap(),
        Err(Error::Authentication)
    ));
}

#[tokio::test]
async fn sftp_only_connects_and_lists_when_server_refuses_all_terminal_requests() {
    let fixture = Fixture::start_sftp(subsystem).await;
    let mut config = config(&fixture, "direct-sftp");
    config.cols = 0;
    config.rows = 0;
    let (control, mut prompts, mut events) = start(config);
    trust(&mut prompts).await;
    control
        .commands
        .send(Command::Resize {
            cols: 100,
            rows: 30,
        })
        .await
        .unwrap();
    assert_eq!(fixture.counts.passwords.load(Ordering::SeqCst), 0);
    password(&mut prompts, "test-only-password", false).await;
    connected(&mut events).await;
    control
        .commands
        .send(Command::Input(b"ignored terminal input".to_vec()))
        .await
        .unwrap();
    control
        .commands
        .send(Command::Resize { cols: 80, rows: 24 })
        .await
        .unwrap();

    let browser = Browser::connect(&control.commands).await.unwrap();
    let directory = browser.list("/").await.unwrap();

    assert!(
        directory
            .entries
            .iter()
            .any(|entry| entry.name == "hello.txt")
    );
    assert_eq!(fixture.counts.passwords.load(Ordering::SeqCst), 1);
    assert_eq!(fixture.counts.ptys.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.counts.shells.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.counts.resizes.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.counts.subsystems.load(Ordering::SeqCst), 1);
    control.commands.send(Command::Disconnect).await.unwrap();
    stopped(&mut events, &fixture, State::Disconnected).await;
}

#[tokio::test]
async fn sftp_open_is_rejected_until_host_trust_and_authentication_complete() {
    let fixture = Fixture::start_sftp(subsystem).await;
    let (control, mut prompts, mut events) = start(config(&fixture, "sftp-before-auth"));
    let pending = next_prompt(&mut prompts).await;
    let Prompt::HostKey { reply, .. } = pending else {
        panic!("Expected host trust before credentials");
    };
    rejected_open(&control).await;
    reply.send(true).unwrap();
    let Prompt::Secret { reply, .. } = next_prompt(&mut prompts).await else {
        panic!("Expected password prompt");
    };
    rejected_open(&control).await;
    assert_eq!(fixture.counts.subsystems.load(Ordering::SeqCst), 0);
    control.commands.send(Command::Disconnect).await.unwrap();
    stopped(&mut events, &fixture, State::Disconnected).await;
    assert!(reply.is_closed());
    assert_eq!(fixture.counts.passwords.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn sftp_only_peer_disconnect_is_detected_without_a_shell_channel() {
    let fixture = Fixture::start_sftp(subsystem).await;
    let (control, mut prompts, mut events) = start(config(&fixture, "sftp-peer-loss"));
    trust(&mut prompts).await;
    password(&mut prompts, "test-only-password", false).await;
    connected(&mut events).await;
    fixture.disconnect_clients();
    super::tests_recovery::status(&mut events, State::Reconnecting).await;
    control.commands.send(Command::Disconnect).await.unwrap();
    stopped(&mut events, &fixture, State::Disconnected).await;
    assert_eq!(fixture.counts.ptys.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.counts.shells.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn sftp_subsystem_loss_recovers_verified_transport_without_another_password() {
    let fixture = Fixture::start_sftp(subsystem).await;
    let (control, mut prompts, mut events) = start(config(&fixture, "sftp-recovery"));
    trust(&mut prompts).await;
    password(&mut prompts, "test-only-password", false).await;
    connected(&mut events).await;
    let browser = Browser::connect(&control.commands).await.unwrap();
    control.commands.send(Command::Reconnect).await.unwrap();
    super::tests_recovery::status(&mut events, State::Reconnecting).await;
    connected(&mut events).await;
    let reopened = Browser::connect(&control.commands).await.unwrap();
    assert!(
        reopened
            .list("/")
            .await
            .unwrap()
            .entries
            .iter()
            .any(|entry| entry.name == "hello.txt")
    );
    assert!(prompts.try_recv().is_err());
    assert_eq!(fixture.counts.passwords.load(Ordering::SeqCst), 2);
    assert_eq!(fixture.counts.ptys.load(Ordering::SeqCst), 0);
    drop(browser);
}
