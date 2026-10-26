use super::*;

fn submitted(cx: &mut TestAppContext, kind: SecretKind) -> Fixture {
    let mut fixture = pending_dialog(cx, kind, SessionKind::Terminal);
    fixture
        .owner
        .update(cx, |app, _, cx| {
            app.update_store(cx, |store| store.upsert(app.tabs[0].connection.clone()))
                .unwrap();
        })
        .unwrap();
    fixture
        .dialog
        .update(cx, |dialog, window, cx| dialog.submit(window, cx))
        .unwrap();
    assert!(fixture.reply.try_recv().unwrap().is_some());
    fixture
}

fn status(fixture: &Fixture, state: ssh::State, cx: &mut TestAppContext) {
    fixture
        .owner
        .update(cx, |app, _, cx| {
            let (events, receiver) = std::sync::mpsc::channel();
            app.tabs[0].events = receiver;
            events.send(ssh::Event::Status(state)).unwrap();
            app.poll(cx);
        })
        .unwrap();
    crate::ui::persistence::tests::settle(fixture.owner, cx);
}

#[gpui::test]
fn password_is_saved_only_after_connected_and_failure_discards_candidate(cx: &mut TestAppContext) {
    for state in [ssh::State::Connected, ssh::State::Failed] {
        let fixture = submitted(cx, SecretKind::Password);
        fixture
            .owner
            .read_with(cx, |app, cx| {
                assert!(
                    app.store(cx)
                        .saved_password(&app.tabs[0].connection)
                        .is_none()
                );
            })
            .unwrap();
        status(&fixture, state, cx);
        fixture
            .owner
            .read_with(cx, |app, cx| {
                assert_eq!(
                    app.store(cx)
                        .saved_password(&app.tabs[0].connection)
                        .is_some(),
                    state == ssh::State::Connected
                );
            })
            .unwrap();
        fixture
            .entity
            .read_with(cx, |dialog, _| assert!(dialog.password.is_none()));
        fixture
            .owner
            .update(cx, |_, window, _| window.remove_window())
            .unwrap();
    }
}

#[gpui::test]
fn key_passphrases_never_become_saved_passwords(cx: &mut TestAppContext) {
    let fixture = submitted(
        cx,
        SecretKind::KeyPassphrase {
            path: "synthetic-key".into(),
        },
    );
    status(&fixture, ssh::State::Connected, cx);
    fixture
        .owner
        .read_with(cx, |app, cx| {
            assert!(
                app.store(cx)
                    .saved_password(&app.tabs[0].connection)
                    .is_none()
            );
        })
        .unwrap();
}

#[gpui::test]
fn cleared_password_cannot_be_restored_by_in_flight_authentication(cx: &mut TestAppContext) {
    let fixture = submitted(cx, SecretKind::Password);
    fixture
        .owner
        .update(cx, |app, window, cx| {
            let connection = app.tabs[0].connection.clone();
            app.update_store(cx, |store| {
                store.save_password(
                    &connection,
                    ssh::Secret::new("old-synthetic-password".into()),
                )
            })
            .unwrap();
            app.edit(Some(connection), window, cx);
            app.clear_saved_password(cx);
            assert!(!app.tabs[0].save_password);
        })
        .unwrap();
    status(&fixture, ssh::State::Connected, cx);
    fixture
        .owner
        .read_with(cx, |app, cx| {
            assert!(
                app.store(cx)
                    .saved_password(&app.tabs[0].connection)
                    .is_none()
            );
            assert!(
                Store::load(app.directory.join("latch.db"))
                    .unwrap()
                    .saved_password(&app.tabs[0].connection)
                    .is_none()
            );
        })
        .unwrap();
}

#[gpui::test]
fn stored_password_answers_initial_prompt_but_retry_requires_fresh_input(cx: &mut TestAppContext) {
    let fixture = submitted(cx, SecretKind::Password);
    let (prompts, receiver) = mpsc::channel(2);
    fixture
        .owner
        .update(cx, |app, _, cx| {
            let connection = &app.tabs[0].connection;
            app.update_store(cx, |store| {
                store.save_password(
                    connection,
                    ssh::Secret::new("stored-synthetic-password".into()),
                )
            })
            .unwrap();
            app.tabs[0].prompts = receiver;
        })
        .unwrap();
    let (reply, mut response) = oneshot::channel();
    prompts
        .try_send(Prompt::Secret {
            kind: SecretKind::Password,
            error: None,
            reply,
        })
        .ok()
        .unwrap();
    fixture.owner.update(cx, |app, _, cx| app.poll(cx)).unwrap();
    assert!(
        response
            .try_recv()
            .unwrap()
            .is_some_and(|value| value.as_str() == "stored-synthetic-password")
    );
    let (reply, mut response) = oneshot::channel();
    prompts
        .try_send(Prompt::Secret {
            kind: SecretKind::Password,
            error: Some("Password rejected".into()),
            reply,
        })
        .ok()
        .unwrap();
    fixture.owner.update(cx, |app, _, cx| app.poll(cx)).unwrap();
    crate::ui::persistence::tests::settle(fixture.owner, cx);
    assert!(matches!(
        response.try_recv(),
        Err(oneshot::error::TryRecvError::Empty)
    ));
    fixture.entity.read_with(cx, |dialog, cx| {
        assert!(dialog.password.is_none());
        assert!(dialog.fields[0].1.read(cx).value.is_empty());
    });
    fixture
        .owner
        .read_with(cx, |app, cx| {
            assert!(
                app.store(cx)
                    .saved_password(&app.tabs[0].connection)
                    .is_none()
            );
        })
        .unwrap();
}

#[gpui::test]
fn clearing_password_cancels_attempts_in_other_windows(cx: &mut TestAppContext) {
    let fixture = submitted(cx, SecretKind::Password);
    let other = crate::ui::windows_tests::second(fixture.owner, cx);
    let connection = fixture
        .owner
        .read_with(cx, |app, _| app.tabs[0].connection.clone())
        .unwrap();
    other
        .update(cx, |app, _, cx| {
            let (mut tab, _) =
                crate::ui::sessions::tests::synthetic_tab(connection, SessionKind::Terminal, cx);
            tab.password_attempt = Some(ssh::Secret::new("synthetic-attempt".into()));
            app.tabs.push(tab);
        })
        .unwrap();
    fixture
        .owner
        .update(cx, |app, window, cx| {
            app.edit(Some(app.tabs[0].connection.clone()), window, cx);
            app.clear_saved_password(cx);
        })
        .unwrap();
    other
        .read_with(cx, |app, _| {
            assert!(!app.tabs[0].save_password);
            assert!(app.tabs[0].password_attempt.is_none());
        })
        .unwrap();
    crate::ui::persistence::tests::settle(fixture.owner, cx);
}
