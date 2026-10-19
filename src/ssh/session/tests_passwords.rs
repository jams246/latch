use super::*;
use crate::ssh::{Secret, fixture::Fixture, tests};
use std::sync::atomic::Ordering;

#[tokio::test]
async fn rejected_cached_password_requires_a_fresh_prompt_after_host_verification() {
    let fixture = Fixture::start(false).await;
    let config = tests::config(&fixture, "rejected-cached-password");
    let (prompts, mut requests) = mpsc::channel(8);
    let (events, _output) = mpsc::channel(8);
    let mut memory = Memory {
        credential: Some(Secret::new("wrong-synthetic-password".into())),
        ..Memory::default()
    };
    let establish = setup::establish(&config, &prompts, &events, true, &mut memory, false);
    let respond = async {
        let Prompt::HostKey { reply, .. } = tests::next_prompt(&mut requests).await else {
            panic!("Expected host trust before credentials");
        };
        assert_eq!(fixture.counts.passwords.load(Ordering::SeqCst), 0);
        reply.send(true).unwrap();
        tests::password(&mut requests, "test-only-password", true).await;
    };
    let (result, ()) = tokio::join!(establish, respond);
    assert!(result.is_ok());
    assert_eq!(fixture.counts.passwords.load(Ordering::SeqCst), 2);
    assert!(
        memory
            .credential
            .is_some_and(|password| password.as_str() == "test-only-password")
    );
}
