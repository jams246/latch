use std::{sync::atomic::Ordering, time::Duration};

use tokio::sync::mpsc;

use super::{
    fixture::Fixture,
    tests::{config, connected, next_prompt, password, start, trust},
    *,
};

async fn inactive(fixture: &Fixture) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while fixture.counts.active.load(Ordering::SeqCst) != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}

async fn echoed(events: &mut mpsc::Receiver<Event>, expected: &[u8]) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(Event::Output(bytes)) = events.recv().await
                && bytes.windows(expected.len()).any(|part| part == expected)
            {
                return;
            }
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn cancelling_password_prompt_discards_reply_and_releases_transport() {
    let fixture = Fixture::start(false).await;
    let (control, mut prompts, mut events) = start(config(&fixture, "cancel-password"));
    trust(&mut prompts).await;
    let Prompt::Secret { reply, .. } = next_prompt(&mut prompts).await else {
        panic!("Expected password prompt");
    };
    control.commands.send(Command::Disconnect).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !matches!(
            events.recv().await,
            Some(Event::Status(State::Disconnected))
        ) {}
    })
    .await
    .unwrap();
    inactive(&fixture).await;
    assert!(reply.is_closed());
    assert_eq!(fixture.counts.passwords.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn transport_failure_discards_pending_password_reply() {
    let fixture = Fixture::start(false).await;
    let (_control, mut prompts, mut events) = start(config(&fixture, "transport-password"));
    trust(&mut prompts).await;
    let Prompt::Secret { reply, .. } = next_prompt(&mut prompts).await else {
        panic!("Expected password prompt");
    };
    fixture.disconnect_clients();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !matches!(events.recv().await, Some(Event::Status(State::Failed))) {}
    })
    .await
    .unwrap();
    assert!(reply.is_closed());
    inactive(&fixture).await;
}

#[tokio::test]
async fn socket_eof_discards_pending_host_trust_and_releases_transport() {
    use tokio::{net::TcpListener, sync::oneshot};
    let fixture = Fixture::start(false).await;
    let proxy = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let mut config = config(&fixture, "trust-eof");
    config.port = proxy.local_addr().unwrap().port();
    let port = fixture.port;
    let (cut, mut cut_now) = oneshot::channel();
    let proxy = tokio::spawn(async move {
        let (mut downstream, _) = proxy.accept().await.unwrap();
        let mut upstream = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .unwrap();
        tokio::select! {
            _ = tokio::io::copy_bidirectional(&mut downstream, &mut upstream) => (),
            _ = &mut cut_now => (),
        }
    });
    let (_control, mut prompts, mut events) = start(config);
    let Prompt::HostKey { reply, .. } = next_prompt(&mut prompts).await else {
        panic!("Expected host trust");
    };
    cut.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), proxy)
        .await
        .unwrap()
        .unwrap();
    let error = tokio::time::timeout(Duration::from_secs(5), async {
        let mut error = None;
        loop {
            match events.recv().await.expect("Expected connection failure") {
                Event::Error(message) => error = Some(message),
                Event::Status(State::Failed) => break error,
                Event::Status(State::Connected) => panic!("Untrusted session opened a shell"),
                _ => (),
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(error, Some(Error::Transport.to_string()));
    assert!(reply.is_closed());
    assert!(prompts.try_recv().is_err());
    assert_eq!(fixture.counts.passwords.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.counts.ptys.load(Ordering::SeqCst), 0);
    inactive(&fixture).await;
}

#[tokio::test]
async fn refused_shell_never_reports_connected() {
    let fixture = Fixture::start(true).await;
    let (_control, mut prompts, mut events) = start(config(&fixture, "refused-shell"));
    trust(&mut prompts).await;
    password(&mut prompts, "test-only-password", false).await;
    let mut success = false;
    let mut failed = false;
    tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(event) = events.recv().await {
            match event {
                Event::Status(State::Connected) => success = true,
                Event::Status(State::Failed) => {
                    failed = true;
                    break;
                }
                _ => (),
            }
        }
    })
    .await
    .unwrap();
    assert!(failed);
    assert!(!success);
    inactive(&fixture).await;
}

#[tokio::test]
async fn two_tabs_to_same_host_authenticate_independently_and_close_independently() {
    let fixture = Fixture::start(false).await;
    let config = config(&fixture, "independent-tabs");
    let (first, mut prompts, mut events) = start(config.clone());
    trust(&mut prompts).await;
    password(&mut prompts, "test-only-password", false).await;
    connected(&mut events).await;
    let (second, mut prompts, mut events) = start(config);
    password(&mut prompts, "test-only-password", false).await;
    connected(&mut events).await;
    assert_eq!(fixture.counts.passwords.load(Ordering::SeqCst), 2);
    drop(first);
    second
        .commands
        .send(Command::Input(b"second survives".to_vec()))
        .await
        .unwrap();
    echoed(&mut events, b"second survives").await;
    assert_eq!(fixture.counts.active.load(Ordering::SeqCst), 1);
    drop(second);
    inactive(&fixture).await;
}

#[tokio::test]
async fn repeated_connect_disconnect_releases_each_socket_and_channel() {
    let fixture = Fixture::start(false).await;
    let config = config(&fixture, "cycles");
    for attempt in 0..4 {
        let (control, mut prompts, mut events) = start(config.clone());
        if attempt == 0 {
            trust(&mut prompts).await;
        }
        password(&mut prompts, "test-only-password", false).await;
        connected(&mut events).await;
        drop(control);
        inactive(&fixture).await;
    }
    assert_eq!(fixture.counts.passwords.load(Ordering::SeqCst), 4);
}

#[tokio::test]
async fn cancelling_stalled_ssh_handshake_releases_socket() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let fixture = Fixture::start(false).await;
    let mut config = config(&fixture, "stalled");
    config.port = listener.local_addr().unwrap().port();
    let (control, _prompts, _events) = start(config);
    let (mut socket, _) = listener.accept().await.unwrap();
    let mut bytes = [0; 1024];
    let read = socket.read(&mut bytes).await.unwrap();
    assert!(read > 0);
    socket
        .write_all(b"SSH-2.0-SyntheticStall\r\n")
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    drop(control);
    tokio::time::timeout(Duration::from_secs(5), async {
        while socket.read(&mut bytes).await.unwrap_or(0) != 0 {}
    })
    .await
    .unwrap();
}
