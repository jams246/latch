use std::{sync::atomic::Ordering, time::Duration};

use tokio::sync::mpsc;

use super::{
    fixture::Fixture,
    session::shell::Shell,
    tests::{config, connected, password, start, trust},
    tests_recovery::status,
    *,
};

async fn login(
    fixture: &Fixture,
    tag: &str,
) -> (
    SessionControl,
    mpsc::Receiver<Prompt>,
    mpsc::Receiver<Event>,
) {
    let (control, mut prompts, mut events) = start(config(fixture, tag));
    trust(&mut prompts).await;
    password(&mut prompts, "test-only-password", false).await;
    connected(&mut events).await;
    output(&mut events, b"Working directory: /home/synthetic").await;
    (control, prompts, events)
}

async fn output(events: &mut mpsc::Receiver<Event>, expected: &[u8]) {
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut bytes = Vec::new();
        loop {
            match events
                .recv()
                .await
                .expect("Expected synthetic shell output")
            {
                Event::Output(data) => {
                    bytes.extend_from_slice(&data);
                    if bytes.windows(expected.len()).any(|part| part == expected) {
                        return;
                    }
                }
                Event::Error(error) => panic!("Unexpected connection error: {error}"),
                _ => (),
            }
        }
    })
    .await
    .unwrap();
}

async fn input(control: &SessionControl, text: impl AsRef<[u8]>) {
    control
        .commands
        .send(Command::Input(text.as_ref().to_vec()))
        .await
        .unwrap();
}

#[tokio::test]
async fn supported_shells_restore_reported_directory_after_transport_interruption() {
    for executable in ["/bin/bash", "/bin/zsh", "/usr/bin/fish"] {
        let fixture = Fixture::start_shell(executable).await;
        let (control, mut prompts, mut events) = login(&fixture, "directory-recovery").await;
        let shell = Shell::from_path(executable).unwrap();
        assert_eq!(
            fixture.counts.execs.lock().unwrap().last(),
            Some(&shell.command(None))
        );
        let path = "/srv/team's reports/$(literal)/東京";
        input(&control, format!("synthetic-cwd:{path}")).await;
        output(&mut events, b"/srv/team%27s%20reports/").await;
        input(&control, b"synthetic-drop").await;
        status(&mut events, State::Reconnecting).await;
        connected(&mut events).await;
        output(&mut events, format!("Working directory: {path}").as_bytes()).await;
        assert_eq!(
            fixture.counts.execs.lock().unwrap().last(),
            Some(&shell.command(Some(path)))
        );
        assert!(prompts.try_recv().is_err());
        assert_eq!(fixture.counts.shells.load(Ordering::SeqCst), 2);
        control.commands.send(Command::Disconnect).await.unwrap();
        status(&mut events, State::Disconnected).await;
    }
}

#[tokio::test]
async fn unsupported_shell_uses_standard_shell_request_without_replaying_input() {
    let fixture = Fixture::start_shell("/bin/unknown").await;
    let (control, mut prompts, mut events) = login(&fixture, "unsupported-directory-shell").await;
    input(&control, b"synthetic-cwd:/srv/project").await;
    output(&mut events, b"file://synthetic/srv/project").await;
    input(&control, b"synthetic-drop").await;
    status(&mut events, State::Reconnecting).await;
    connected(&mut events).await;
    output(&mut events, b"Working directory: /home/synthetic").await;
    assert!(
        fixture
            .counts
            .execs
            .lock()
            .unwrap()
            .iter()
            .all(|command| command == "printf '\\036LATCH_SHELL\\037%s\\036' \"$SHELL\"")
    );
    assert_eq!(fixture.counts.shells.load(Ordering::SeqCst), 2);
    assert!(prompts.try_recv().is_err());
    control.commands.send(Command::Disconnect).await.unwrap();
    status(&mut events, State::Disconnected).await;
}

#[tokio::test]
async fn echoed_commands_cannot_replace_reported_directory() {
    let fixture = Fixture::start_shell("/bin/bash").await;
    let (control, _, mut events) = login(&fixture, "directory-output-only").await;
    input(&control, b"cd /unreported/path\r").await;
    output(&mut events, b"cd /unreported/path\r").await;
    input(&control, b"synthetic-drop").await;
    status(&mut events, State::Reconnecting).await;
    connected(&mut events).await;
    output(&mut events, b"Working directory: /home/synthetic").await;
    assert_eq!(
        fixture.counts.execs.lock().unwrap().last(),
        Some(
            &Shell::from_path("/bin/bash")
                .unwrap()
                .command(Some("/home/synthetic"))
        )
    );
    control.commands.send(Command::Disconnect).await.unwrap();
    status(&mut events, State::Disconnected).await;
}

#[tokio::test]
async fn wrapped_shell_rejection_falls_back_to_standard_interactive_shell() {
    let fixture = Fixture::start_shell("/bin/bash").await;
    fixture
        .counts
        .reject_wrapped_shell
        .store(true, Ordering::SeqCst);
    let (control, _, mut events) = login(&fixture, "rejected-wrapped-shell").await;
    assert_eq!(
        fixture.counts.execs.lock().unwrap().last(),
        Some(&Shell::from_path("/bin/bash").unwrap().command(None))
    );
    assert_eq!(fixture.counts.shells.load(Ordering::SeqCst), 1);
    input(&control, b"fallback shell input").await;
    output(&mut events, b"fallback shell input").await;
    control.commands.send(Command::Disconnect).await.unwrap();
    status(&mut events, State::Disconnected).await;
}
