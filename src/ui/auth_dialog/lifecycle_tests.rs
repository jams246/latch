//! Programmatic GPUI lifecycle coverage, separate from native HWND modality tests.

use super::{AuthDialog, Input};
use crate::ui::{Latch, Tab};
use gpui::{AppContext, Entity, TestAppContext, WeakEntity, WindowHandle};
use latch::{
    ssh::{self, Prompt, SecretKind},
    storage::Store,
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
}

fn pending_dialog(cx: &mut TestAppContext, kind: SecretKind) -> Fixture {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".verification/latch-plan/gpui-lifecycle")
        .join(uuid::Uuid::new_v4().to_string());
    let store = Store::load(directory.join("connections.json")).unwrap();
    let runtime = Arc::new(tokio::runtime::Runtime::new().unwrap());
    let owner = cx
        .add_window(|window, cx| Latch::new(store, directory, runtime, window.window_handle(), cx));
    let (commands, _) = mpsc::channel(4);
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
            let terminal =
                cx.new(|cx| TerminalView::new(TerminalState::new(80, 24), Arc::new(|_| {}), cx));
            let (_, events) = std::sync::mpsc::channel();
            let (_, prompts) = mpsc::channel(1);
            app.tabs.push(Tab {
                id: "pending-fixture-tab".into(),
                connection_id: "synthetic-connection".into(),
                name: "Synthetic pending tab".into(),
                destination: "synthetic@127.0.0.1:1".into(),
                state: ssh::State::Authenticating,
                error: None,
                counted: false,
                transport: None,
                terminal,
                dialog: Some(dialog),
                events,
                prompts,
                files: None,
                show_files: false,
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
    let mut fixture = pending_dialog(cx, SecretKind::Password);
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
