use super::{
    Files, local,
    pane::{Column, Side},
};
use gpui::{
    IntoElement, MouseButton, NavigationDirection, TestAppContext, VisualTestContext, WindowHandle,
    point, px, size,
};
use std::{
    path::PathBuf,
    sync::{Arc, mpsc},
};

pub(super) fn fixture(cx: &mut TestAppContext) -> (WindowHandle<Files>, VisualTestContext) {
    let config = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".verification/dual-pane/ui-interaction")
        .join(uuid::Uuid::new_v4().to_string());
    let initial = config.join("initial");
    std::fs::create_dir_all(&initial).unwrap();
    std::fs::create_dir_all(config.join("first")).unwrap();
    std::fs::create_dir_all(config.join("second")).unwrap();
    local::remember(&config, &initial).unwrap();
    let runtime = Arc::new(
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap(),
    );
    let (commands, _) = tokio::sync::mpsc::channel(1);
    let handle = cx.add_window(|_, cx| Files::new(commands, runtime, config, cx));
    handle
        .update(cx, |files, _, _| {
            for operation in [files.operation.take(), files.local_operation.take()]
                .into_iter()
                .flatten()
            {
                operation.abort();
            }
            (files.sender, files.events) = mpsc::channel();
            (files.local_sender, files.local_events) = mpsc::channel();
            // Keep worker results out of polling while checking event dispatch.
            files.local_sender = mpsc::channel().0;
            files.local_busy = false;
            files.busy = false;
            files.local_status.clear();
            files.status.clear();
        })
        .unwrap();
    let mut visual = VisualTestContext::from_window(*handle, cx);
    visual.simulate_resize(size(px(1200.), px(700.)));
    draw(handle, &mut visual);
    (handle, visual)
}

pub(super) fn draw(handle: WindowHandle<Files>, cx: &mut VisualTestContext) {
    let root = handle.root(cx).unwrap();
    cx.draw(point(px(0.), px(0.)), size(px(1200.), px(700.)), |_, _| {
        root.into_any_element()
    });
}

#[gpui::test]
fn clicking_each_panes_headers_changes_only_its_own_sort(cx: &mut TestAppContext) {
    let (handle, mut visual) = fixture(cx);
    for (side, selectors) in [
        (
            Side::Local,
            [
                "local-column-0",
                "local-column-1",
                "local-column-2",
                "local-column-3",
            ],
        ),
        (
            Side::Remote,
            [
                "remote-column-0",
                "remote-column-1",
                "remote-column-2",
                "remote-column-3",
            ],
        ),
    ] {
        for (column, selector) in [Column::Name, Column::Modified, Column::Size, Column::Type]
            .into_iter()
            .zip(selectors)
        {
            let other_before = handle
                .read_with(&visual, |files, _| match side {
                    Side::Local => (files.remote_sort.column, files.remote_sort.descending),
                    Side::Remote => (files.local_sort.column, files.local_sort.descending),
                })
                .unwrap();
            for click in 0..2 {
                let position = visual.debug_bounds(selector).unwrap().center();
                visual.simulate_click(position, Default::default());
                handle
                    .read_with(&visual, |files, _| {
                        let (sort, other) = match side {
                            Side::Local => (files.local_sort, files.remote_sort),
                            Side::Remote => (files.remote_sort, files.local_sort),
                        };
                        assert_eq!(sort.column, column);
                        assert_eq!(sort.descending, (column == Column::Name) ^ (click == 1));
                        assert_eq!((other.column, other.descending), other_before);
                    })
                    .unwrap();
                draw(handle, &mut visual);
            }
        }
    }
}

#[gpui::test]
fn mouse_back_and_forward_use_the_pane_under_the_pointer(cx: &mut TestAppContext) {
    let (handle, mut visual) = fixture(cx);
    handle
        .update(&mut visual, |files, _, _| {
            for path in ["first", "second"] {
                files
                    .local_history
                    .commit(files.config_directory.join(path));
                files.remote_history.commit(format!("/{path}"));
            }
        })
        .unwrap();
    draw(handle, &mut visual);

    for (side, selector) in [(Side::Local, "local-pane"), (Side::Remote, "remote-pane")] {
        for (direction, target) in [
            (NavigationDirection::Back, 0),
            (NavigationDirection::Forward, 1),
        ] {
            let position = visual.debug_bounds(selector).unwrap().center();
            visual.simulate_mouse_down(
                position,
                MouseButton::Navigate(direction),
                Default::default(),
            );
            visual.simulate_mouse_up(
                position,
                MouseButton::Navigate(direction),
                Default::default(),
            );
            handle
                .update(&mut visual, |files, _, _| match side {
                    Side::Local => {
                        assert_eq!(files.pending_local_history, Some(target));
                        assert_eq!(files.pending_remote_history, None);
                        if let Some(task) = files.local_operation.take() {
                            task.abort();
                        }
                        files.local_busy = false;
                        files.pending_local_history = None;
                        files.local_history.commit_at(
                            target,
                            files.config_directory.join(if target == 0 {
                                "first"
                            } else {
                                "second"
                            }),
                        );
                    }
                    Side::Remote => {
                        assert_eq!(files.pending_remote_history, Some(target));
                        assert_eq!(files.pending_local_history, None);
                        if let Some(task) = files.operation.take() {
                            task.abort();
                        }
                        files.busy = false;
                        files.pending_remote_history = None;
                        files.remote_history.commit_at(
                            target,
                            if target == 0 { "/first" } else { "/second" }.into(),
                        );
                    }
                })
                .unwrap();
            draw(handle, &mut visual);
        }
    }
}
