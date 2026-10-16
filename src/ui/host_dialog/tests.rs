use crate::ui::{Latch, Tab, TabContent};
use gpui::{
    AppContext, IntoElement, KeyUpEvent, Keystroke, TestAppContext, VisualTestContext,
    WindowHandle, point, px, size,
};
use latch::{
    ssh,
    storage::{AuthMethod, Connection, Store},
    terminal::{TerminalState, TerminalView},
};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::oneshot::error::TryRecvError::{Closed, Empty};
use tokio::sync::{mpsc, oneshot};

fn fixture(cx: &mut TestAppContext) -> (WindowHandle<Latch>, VisualTestContext) {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".verification/host-identity/tests")
        .join(uuid::Uuid::new_v4().to_string());
    let store = Store::load(directory.join("latch.db")).unwrap();
    let runtime = Arc::new(tokio::runtime::Runtime::new().unwrap());
    let handle = cx
        .add_window(|window, cx| Latch::new(store, directory, runtime, window.window_handle(), cx));
    let visual = VisualTestContext::from_window(*handle, cx);
    visual.simulate_resize(size(px(1000.), px(800.)));
    (handle, visual)
}

fn pending_host(
    handle: WindowHandle<Latch>,
    cx: &mut VisualTestContext,
    host: &str,
) -> (oneshot::Receiver<bool>, std::sync::mpsc::Sender<ssh::Event>) {
    let (reply, response) = oneshot::channel();
    let (sender, events) = std::sync::mpsc::channel();
    handle
        .update(cx, |app, window, cx| {
            let (prompts, receiver) = mpsc::channel(1);
            prompts
                .try_send(ssh::Prompt::HostKey {
                    host: host.into(),
                    port: 2222,
                    algorithm: "ssh-ed25519".into(),
                    fingerprint: "SHA256:synthetic-fingerprint".into(),
                    reply,
                })
                .unwrap();
            let content =
                if app.tabs.is_empty() {
                    TabContent::Terminal(cx.new(|cx| {
                        TerminalView::new(TerminalState::new(80, 24), Arc::new(|_| {}), cx)
                    }))
                } else {
                    TabContent::Sftp {
                        files: None,
                        focus: cx.focus_handle(),
                    }
                };
            app.tabs.push(Tab {
                id: host.into(),
                connection: Connection::new(
                    host.into(),
                    host.into(),
                    "synthetic".into(),
                    AuthMethod::Password,
                ),
                name: host.into(),
                destination: format!("synthetic@{host}:2222"),
                state: ssh::State::Connecting,
                error: None,
                counted: false,
                transport: None,
                content,
                dialog: None,
                host_dialog: None,
                reconnect: None,
                events,
                prompts: receiver,
                closing: false,
            });
            app.active = Some(app.tabs.len() - 1);
            window.focus(&app.connection_search.read(cx).focus);
            app.poll(cx);
        })
        .unwrap();
    (response, sender)
}

fn draw(handle: WindowHandle<Latch>, cx: &mut VisualTestContext) {
    let root = handle.root(cx).unwrap();
    cx.draw(point(px(0.), px(0.)), size(px(1000.), px(800.)), |_, _| {
        root.into_any_element()
    });
}

fn enter(cx: &mut VisualTestContext) {
    cx.simulate_keystrokes("enter");
    cx.simulate_event(KeyUpEvent {
        keystroke: Keystroke::parse("enter").unwrap(),
    });
}

#[gpui::test]
fn host_prompt_uses_main_window_and_preserves_identity_details(cx: &mut TestAppContext) {
    let (handle, mut visual) = fixture(cx);
    let (_response, _events) = pending_host(handle, &mut visual, "first.example.invalid");
    draw(handle, &mut visual);
    assert_eq!(cx.windows().len(), 1);
    handle
        .update(&mut visual, |app, window, _| {
            let tab = &app.tabs[0];
            let dialog = tab.host_dialog.as_ref().unwrap();
            for detail in [
                "first.example.invalid:2222",
                "ssh-ed25519",
                "SHA256:synthetic-fingerprint",
            ] {
                assert!(dialog.detail.contains(detail));
            }
            assert!(dialog.cancel.is_focused(window));
            assert!(tab.dialog.is_none());
        })
        .unwrap();
}

#[gpui::test]
fn enter_defaults_to_cancel_and_restores_previous_focus(cx: &mut TestAppContext) {
    let (handle, mut visual) = fixture(cx);
    let (mut response, _events) = pending_host(handle, &mut visual, "cancel.example.invalid");
    draw(handle, &mut visual);
    enter(&mut visual);
    assert_eq!(response.try_recv(), Ok(false));
    handle
        .update(&mut visual, |app, window, cx| {
            assert!(app.tabs[0].host_dialog.is_none());
            assert!(app.connection_search.read(cx).focus.is_focused(window));
        })
        .unwrap();
}

#[gpui::test]
fn tab_traps_focus_and_trust_requires_explicit_selection(cx: &mut TestAppContext) {
    let (handle, mut visual) = fixture(cx);
    let (mut response, _events) = pending_host(handle, &mut visual, "trust.example.invalid");
    draw(handle, &mut visual);
    for (keys, confirm) in [
        ("tab", true),
        ("tab", false),
        ("shift-tab", true),
        ("shift-tab", false),
    ] {
        visual.simulate_keystrokes(keys);
        handle
            .update(&mut visual, |app, window, cx| {
                let dialog = app.tabs[0].host_dialog.as_ref().unwrap();
                assert!(dialog.focus.contains_focused(window, cx));
                assert_eq!(dialog.confirm.is_focused(window), confirm);
                assert_eq!(dialog.cancel.is_focused(window), !confirm);
            })
            .unwrap();
        assert_eq!(response.try_recv(), Err(Empty));
    }
    visual.simulate_keystrokes("tab");
    enter(&mut visual);
    assert_eq!(response.try_recv(), Ok(true));
}

#[gpui::test]
fn delayed_earlier_tab_prompt_cannot_replace_visible_trust_target(cx: &mut TestAppContext) {
    let (handle, mut visual) = fixture(cx);
    let (mut first, _first_events) = pending_host(handle, &mut visual, "first.example.invalid");
    let mut delayed = handle
        .update(&mut visual, |app, _, _| app.tabs[0].host_dialog.take())
        .unwrap();
    delayed.as_mut().unwrap().shown = false;
    let (mut second, _second_events) = pending_host(handle, &mut visual, "second.example.invalid");
    draw(handle, &mut visual);
    visual.simulate_keystrokes("tab");
    handle
        .update(&mut visual, |app, _, cx| {
            app.tabs[0].host_dialog = delayed;
            cx.notify();
        })
        .unwrap();
    draw(handle, &mut visual);
    enter(&mut visual);
    assert_eq!(second.try_recv(), Ok(true));
    assert_eq!(first.try_recv(), Err(Empty));
    draw(handle, &mut visual);
    let header = visual.debug_bounds("host-key-dialog").unwrap().origin + point(px(16.), px(16.));
    visual.simulate_click(header, Default::default());
    visual.simulate_keystrokes("escape");
    assert_eq!(first.try_recv(), Ok(false));
}

#[gpui::test]
fn pending_tabs_keep_independent_trust_replies(cx: &mut TestAppContext) {
    let (handle, mut visual) = fixture(cx);
    let (mut first, _first_events) = pending_host(handle, &mut visual, "first.example.invalid");
    let (mut second, _second_events) = pending_host(handle, &mut visual, "second.example.invalid");
    draw(handle, &mut visual);
    handle
        .update(&mut visual, |app, window, cx| {
            app.respond_host_key("first.example.invalid", true, window, cx)
        })
        .unwrap();
    assert_eq!(first.try_recv(), Ok(true));
    assert_eq!(second.try_recv(), Err(Empty));
    draw(handle, &mut visual);
    handle
        .update(&mut visual, |app, window, cx| app.close_tab(1, window, cx))
        .unwrap();
    assert_eq!(second.try_recv(), Err(Closed));
}

#[gpui::test]
fn shutdown_releases_pending_host_reply(cx: &mut TestAppContext) {
    let (handle, mut visual) = fixture(cx);
    let (mut response, _events) = pending_host(handle, &mut visual, "shutdown.example.invalid");
    draw(handle, &mut visual);
    handle
        .update(&mut visual, |app, _, cx| {
            assert!(!app.request_shutdown(cx));
            assert!(app.tabs.is_empty());
        })
        .unwrap();
    assert_eq!(response.try_recv(), Err(Closed));
}

#[gpui::test]
fn expired_reply_and_failed_session_dismiss_pending_dialogs(cx: &mut TestAppContext) {
    let (handle, mut visual) = fixture(cx);
    let (expired, _first_events) = pending_host(handle, &mut visual, "expired.example.invalid");
    let (mut failed, events) = pending_host(handle, &mut visual, "failed.example.invalid");
    draw(handle, &mut visual);
    drop(expired);
    events.send(ssh::Event::Status(ssh::State::Failed)).unwrap();
    handle
        .root(&mut visual)
        .unwrap()
        .update(&mut visual, |app, cx| app.poll(cx));
    handle
        .update(&mut visual, |app, window, cx| {
            assert!(app.tabs.iter().all(|tab| tab.host_dialog.is_none()));
            assert!(app.connection_search.read(cx).focus.is_focused(window));
        })
        .unwrap();
    assert_eq!(failed.try_recv(), Err(Closed));
    assert_eq!(cx.windows().len(), 1);
}
