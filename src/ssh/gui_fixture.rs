use std::path::PathBuf;

use super::fixture::{Fixture, key};
use crate::storage::{AuthMethod, Connection, Store};

fn subsystem(stream: russh::ChannelStream<russh::server::Msg>) {
    tokio::spawn(crate::sftp::tests_server::serve(stream));
}

#[tokio::test]
async fn scrollback_fixture_streams_more_than_one_screen() {
    use super::{Command, Event, tests};
    use std::time::Duration;

    let fixture = Fixture::start(false).await;
    let (control, mut prompts, mut events) = tests::start(tests::config(&fixture, "scrollback"));
    tests::trust(&mut prompts).await;
    tests::password(&mut prompts, "test-only-password", false).await;
    tests::connected(&mut events).await;
    control
        .commands
        .send(Command::Input(b"synthetic-scrollback".to_vec()))
        .await
        .unwrap();
    let mut output = String::new();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !output.contains("Scrollback line 200\r\n") {
            if let Event::Output(bytes) = events.recv().await.expect("Expected scrollback output") {
                output.push_str(&String::from_utf8(bytes).unwrap());
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(
        output
            .lines()
            .filter(|line| line.starts_with("Scrollback line "))
            .count(),
        200
    );
}

#[tokio::test]
#[ignore = "Interactive loopback GUI verification server"]
async fn gui_server() {
    let fixture = Fixture::start_shell_with("/bin/bash", Some(subsystem)).await;
    *fixture.counts.directory.lock().unwrap() = Some("/srv/latch project".into());
    let root = std::env::var_os("LATCH_GUI_FIXTURE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".verification/latch-plan/gui-fixture")
        });
    let directory = root.join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&directory).unwrap();
    let plain_path = directory.join("synthetic_ed25519");
    let encrypted_path = directory.join("synthetic_ed25519_encrypted");
    let private = key(2);
    let encrypted = private
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
    for (path, key) in [(&plain_path, &private), (&encrypted_path, &encrypted)] {
        std::fs::write(
            path,
            key.to_openssh(russh::keys::ssh_key::LineEnding::LF)
                .unwrap(),
        )
        .unwrap();
    }
    let mut store = Store::load(directory.join("latch.db")).unwrap();
    std::fs::write(
        directory.join("known_hosts"),
        format!(
            "[127.0.0.1]:{} {}\n",
            fixture.port,
            key(1).public_key().to_openssh().unwrap()
        ),
    )
    .unwrap();
    for (name, auth) in [
        ("Alpha Password", AuthMethod::Password),
        (
            "Bravo Key",
            AuthMethod::KeyFile {
                path: plain_path.clone(),
            },
        ),
        (
            "Charlie Encrypted",
            AuthMethod::KeyFile {
                path: encrypted_path.clone(),
            },
        ),
        ("Delta Challenge", AuthMethod::KeyboardInteractive),
        ("Echo Agent", AuthMethod::Agent),
    ] {
        let mut connection =
            Connection::new(name.into(), "127.0.0.1".into(), "synthetic".into(), auth);
        connection.port = fixture.port;
        store.upsert(connection).unwrap();
    }
    let ready = serde_json::json!({
        "port": fixture.port,
        "config_directory": directory,
        "plain_key": plain_path,
        "encrypted_key": encrypted_path,
        "host_key_sha256": key(1).fingerprint(russh::keys::HashAlg::Sha256).to_string(),
    });
    std::fs::write(
        root.join("ready.json"),
        serde_json::to_vec_pretty(&ready).unwrap(),
    )
    .unwrap();
    assert_eq!(store.connections().len(), 5);
    println!(
        "Synthetic GUI SSH/SFTP fixture ready on loopback port {}",
        fixture.port
    );
    std::future::pending::<()>().await;
}
