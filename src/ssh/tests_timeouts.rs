use std::time::Duration;

use tokio::sync::{mpsc, oneshot};

use super::{
    fixture::Fixture,
    tests::{config, next_prompt, start, trust},
    *,
};

#[tokio::test(start_paused = true)]
async fn unanswered_host_trust_times_out_and_discards_reply() {
    let (prompts, mut incoming) = mpsc::channel(1);
    let (reply, response) = oneshot::channel();
    let task = tokio::spawn(async move {
        prompt(
            &prompts,
            Prompt::HostKey {
                host: "synthetic.example".into(),
                port: 22,
                algorithm: "ssh-ed25519".into(),
                fingerprint: "SHA256:synthetic".into(),
                reply,
            },
            response,
        )
        .await
    });
    let Some(Prompt::HostKey { reply, .. }) = incoming.recv().await else {
        panic!("Expected host trust");
    };
    tokio::time::advance(Duration::from_secs(121)).await;
    assert!(matches!(task.await.unwrap(), Err(Error::Timeout)));
    assert!(reply.is_closed());
}

#[tokio::test]
async fn pending_password_timeout_fails_and_discards_secret_reply() {
    let fixture = Fixture::start(false).await;
    let (_control, mut prompts, mut events) = start(config(&fixture, "password-timeout"));
    trust(&mut prompts).await;
    let Prompt::Secret { reply, .. } = next_prompt(&mut prompts).await else {
        panic!("Expected password prompt");
    };
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(121)).await;
    let mut error = String::new();
    tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(event) = events.recv().await {
            match event {
                Event::Error(message) => error = message,
                Event::Status(State::Failed) => break,
                _ => (),
            }
        }
    })
    .await
    .unwrap();
    tokio::time::resume();
    assert!(error.contains("timed out"), "{error}");
    assert!(reply.is_closed());
}
