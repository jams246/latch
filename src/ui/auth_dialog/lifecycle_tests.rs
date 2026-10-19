//! Programmatic GPUI lifecycle coverage, separate from native HWND modality tests.

mod password_tests;

use super::{AuthDialog, Input};
use crate::ui::{Latch, SessionKind, Tab, TabContent};
use gpui::{AppContext, Entity, TestAppContext, WeakEntity, WindowHandle};
use latch::{
    ssh::{self, Prompt, SecretKind},
    storage::{AuthMethod, Connection, Store},
    terminal::TerminalState,
    terminal::TerminalView,
};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::{mpsc, oneshot};

struct Fixture {
    owner: WindowHandle<Latch>,
    dialog: WindowHandle<AuthDialog>,
    entity: Entity<AuthDialog>,
    input: WeakEntity<Input>,
    reply: oneshot::Receiver<Option<ssh::Secret>>,
    commands: mpsc::Receiver<ssh::Command>,
}

fn pending_dialog(cx: &mut TestAppContext, kind: SecretKind, session_kind: SessionKind) -> Fixture {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".verification/latch-plan/gpui-lifecycle")
        .join(uuid::Uuid::new_v4().to_string());
    let store = Store::load(directory.join("latch.db")).unwrap();
    let runtime = Arc::new(tokio::runtime::Runtime::new().unwrap());
    let owner = cx
        .add_window(|window, cx| Latch::new(store, directory, runtime, window.window_handle(), cx));
    let (commands, command_receiver) = mpsc::channel(4);
    let (reply, response) = oneshot::channel();
    let dialog = cx.add_window(|window, cx| {
        // TestPlatform has no HWND; native owner disable/restore is tested separately.
        let mut dialog = AuthDialog {
            prompt: None,
            fields: Vec::new(),
            title: String::new(),
            authentication_title: "Authenticate to synthetic fixture".into(),
            destination: "synthetic@127.0.0.1:1".into(),
            detail: String::new(),
            error: None,
            busy: false,
            password: None,
            commands,
            focus: cx.focus_handle(),
            modal: None,
        };
        dialog.set_prompt(
            Prompt::Secret {
                kind,
                error: None,
                reply,
            },
            window,
            cx,
        );
        dialog
    });
    let entity = dialog.root(cx).unwrap();
    let input = entity.update(cx, |dialog, cx| {
        let input = &dialog.fields[0].1;
        input.update(cx, |input, cx| {
            assert!(input.masked);
            assert!(input.replace(0..0, "synthetic pending secret", cx));
        });
        input.downgrade()
    });
    owner
        .update(cx, |app, _, cx| {
            let content =
                match session_kind {
                    SessionKind::Terminal => TabContent::Terminal(cx.new(|cx| {
                        TerminalView::new(TerminalState::new(80, 24), Arc::new(|_| {}), cx)
                    })),
                    SessionKind::Sftp => TabContent::Sftp {
                        files: None,
                        focus: cx.focus_handle(),
                    },
                };
            let (_, events) = std::sync::mpsc::channel();
            let (_, prompts) = mpsc::channel(1);
            app.tabs.push(Tab {
                id: "pending-fixture-tab".into(),
                connection: Connection::new(
                    "Synthetic".into(),
                    "127.0.0.1".into(),
                    "synthetic".into(),
                    AuthMethod::Password,
                ),
                name: "Synthetic pending tab".into(),
                destination: "synthetic@127.0.0.1:1".into(),
                state: ssh::State::Authenticating,
                error: None,
                counted: false,
                save_password: true,
                password_attempt: None,
                transport: None,
                content,
                dialog: Some(dialog),
                host_dialog: None,
                reconnect: None,
                events,
                prompts,
                closing: false,
            });
            app.active = Some(0);
            cx.notify();
        })
        .unwrap();
    assert_eq!(cx.windows().len(), 2);
    Fixture {
        owner,
        dialog,
        entity,
        input,
        reply: response,
        commands: command_receiver,
    }
}

fn assert_dialog_released(fixture: &mut Fixture, cx: &mut TestAppContext) {
    assert!(matches!(
        fixture.reply.try_recv(),
        Err(oneshot::error::TryRecvError::Closed)
    ));
    fixture.entity.read_with(cx, |dialog, _| {
        assert!(dialog.prompt.is_none());
        assert!(dialog.fields.is_empty());
        assert!(dialog.modal.is_none());
    });
    assert!(fixture.input.upgrade().is_none());
    assert!(fixture.dialog.root(cx).is_err());
    assert_eq!(cx.windows().len(), 1);
    fixture
        .owner
        .read_with(cx, |app, _| {
            assert!(app.tabs.is_empty());
            assert!(app.active.is_none());
            assert_eq!(app.pending_cleanup, 0);
        })
        .unwrap();
}

#[gpui::test]
fn closing_pending_tab_releases_dialog_secret_and_reply(cx: &mut TestAppContext) {
    let mut fixture = pending_dialog(cx, SecretKind::Password, SessionKind::Terminal);
    fixture
        .owner
        .update(cx, |app, window, cx| app.close_tab(0, window, cx))
        .unwrap();
    assert_dialog_released(&mut fixture, cx);
}

#[gpui::test]
fn shutdown_releases_pending_passphrase_before_main_window_can_close(cx: &mut TestAppContext) {
    let mut fixture = pending_dialog(
        cx,
        SecretKind::KeyPassphrase {
            path: ".verification/latch-plan/synthetic-encrypted-key".into(),
        },
        SessionKind::Terminal,
    );
    assert!(
        !fixture
            .owner
            .update(cx, |app, _, cx| app.request_shutdown(cx))
            .unwrap()
    );
    assert_dialog_released(&mut fixture, cx);
    fixture
        .owner
        .update(cx, |app, window, cx| {
            assert!(app.request_shutdown(cx));
            window.remove_window();
        })
        .unwrap();
    assert!(cx.windows().is_empty());
}

#[gpui::test]
fn closing_pending_sftp_tab_releases_dialog_secret_and_reply(cx: &mut TestAppContext) {
    let mut fixture = pending_dialog(cx, SecretKind::Password, SessionKind::Sftp);
    fixture
        .owner
        .update(cx, |app, window, cx| app.close_tab(0, window, cx))
        .unwrap();
    assert_dialog_released(&mut fixture, cx);
}

#[gpui::test]
fn misrouted_host_prompt_cancels_without_trust_or_old_credentials(cx: &mut TestAppContext) {
    let mut fixture = pending_dialog(cx, SecretKind::Password, SessionKind::Terminal);
    let (reply, mut trust) = oneshot::channel();
    fixture
        .dialog
        .update(cx, |dialog, window, cx| {
            dialog.set_prompt(
                Prompt::HostKey {
                    host: "127.0.0.1".into(),
                    port: 22,
                    algorithm: "ssh-ed25519".into(),
                    fingerprint: "synthetic".into(),
                    reply,
                },
                window,
                cx,
            );
            dialog.submit(window, cx);
        })
        .unwrap();
    assert!(matches!(
        trust.try_recv(),
        Err(oneshot::error::TryRecvError::Closed)
    ));
    assert!(matches!(
        fixture.reply.try_recv(),
        Err(oneshot::error::TryRecvError::Closed)
    ));
    assert!(matches!(
        fixture.commands.try_recv(),
        Ok(ssh::Command::Disconnect)
    ));
    fixture.entity.read_with(cx, |dialog, _| {
        assert!(dialog.prompt.is_none());
        assert!(dialog.fields.is_empty());
        assert!(dialog.modal.is_none());
    });
    assert!(fixture.input.upgrade().is_none());
    assert!(fixture.dialog.root(cx).is_err());
    assert_eq!(cx.windows().len(), 1);
}
