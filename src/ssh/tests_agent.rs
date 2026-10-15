use std::{
    sync::{Arc, Mutex, atomic::Ordering},
    time::Duration,
};

use russh::{
    client,
    keys::agent::{
        client::AgentClient,
        server::{Agent, MessageType},
    },
};
use tokio::sync::{mpsc, watch};

use super::{
    fixture::{Fixture, key},
    host::Client,
    tests::config,
    *,
};

#[derive(Clone, Default)]
struct Policy(Arc<Mutex<Vec<&'static str>>>);

impl Agent for Policy {
    async fn confirm_request(&self, request: MessageType) -> bool {
        let name = match request {
            MessageType::RequestKeys => "list-public-identities",
            MessageType::Sign => "sign",
            MessageType::AddKeys => "add-synthetic-key",
            _ => "unexpected",
        };
        self.0.lock().unwrap().push(name);
        true
    }
}

async fn handle(fixture: &Fixture) -> (client::Handle<Client>, mpsc::Receiver<Prompt>) {
    let config = config(fixture, "agent");
    russh::keys::known_hosts::learn_known_hosts_path(
        &config.host,
        config.port,
        key(1).public_key(),
        &config.known_hosts,
    )
    .unwrap();
    let (prompts, incoming) = mpsc::channel(1);
    let (_cancel, cancel) = watch::channel(false);
    let (closed, disconnected) = watch::channel(false);
    let handler = Client {
        host: config.host,
        port: config.port,
        path: config.known_hosts,
        prompts,
        cancel,
        closed,
        disconnected,
    };
    let handle = tokio::time::timeout(
        Duration::from_secs(5),
        client::connect(
            Arc::new(client::Config::default()),
            ("127.0.0.1", fixture.port),
            handler,
        ),
    )
    .await
    .unwrap()
    .unwrap();
    (handle, incoming)
}

#[tokio::test]
async fn agent_signs_without_exporting_private_key_or_prompting_or_forwarding() {
    let fixture = Fixture::start(false).await;
    let (mut handle, mut prompts) = handle(&fixture).await;
    let (seed_client, seed_server) = tokio::io::duplex(65536);
    let (auth_client, auth_server) = tokio::io::duplex(65536);
    let policy = Policy::default();
    let server_policy = policy.clone();
    let server = tokio::spawn(async move {
        russh::keys::agent::server::serve(
            futures::stream::iter([Ok(seed_server), Ok(auth_server)]),
            server_policy,
        )
        .await
        .unwrap();
    });
    let mut seed = AgentClient::connect(seed_client);
    seed.add_identity(&key(2), &[]).await.unwrap();
    policy.0.lock().unwrap().clear();
    tokio::time::timeout(
        Duration::from_secs(5),
        super::agent::authenticate_with(
            &mut handle,
            "synthetic",
            AgentClient::connect(auth_client),
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        *policy.0.lock().unwrap(),
        ["list-public-identities", "sign"]
    );
    assert!(prompts.try_recv().is_err());
    assert_eq!(fixture.counts.passwords.load(Ordering::SeqCst), 0);
    assert_eq!(fixture.counts.agent_forwarding.load(Ordering::SeqCst), 0);
    handle
        .disconnect(
            russh::Disconnect::ByApplication,
            "Synthetic test complete",
            "en",
        )
        .await
        .unwrap();
    drop(seed);
    server.await.unwrap();
}

#[tokio::test]
async fn empty_or_disconnected_agent_fails_without_a_password_prompt() {
    for disconnected in [false, true] {
        let fixture = Fixture::start(false).await;
        let (mut handle, mut prompts) = handle(&fixture).await;
        let (client, stream) = tokio::io::duplex(65536);
        if disconnected {
            drop(stream);
        } else {
            tokio::spawn(async move {
                russh::keys::agent::server::serve(futures::stream::iter([Ok(stream)]), ())
                    .await
                    .unwrap();
            });
        }
        let result = tokio::time::timeout(
            Duration::from_secs(5),
            super::agent::authenticate_with(&mut handle, "synthetic", AgentClient::connect(client)),
        )
        .await
        .unwrap();
        assert!(matches!(result, Err(Error::Agent)));
        assert!(prompts.try_recv().is_err());
        assert_eq!(fixture.counts.passwords.load(Ordering::SeqCst), 0);
    }
}
