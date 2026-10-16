use crate::ui::files::{Files, local, pane::Side};
use gpui::{IntoElement, TestAppContext, VisualTestContext, WindowHandle, point, px, size};
use std::{path::PathBuf, sync::Arc};

fn fixture(cx: &mut TestAppContext) -> (WindowHandle<Files>, VisualTestContext) {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(".verification/dual-pane/breadcrumb-scroll")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&directory).unwrap();
    local::remember(&directory, &directory).unwrap();
    let runtime = Arc::new(tokio::runtime::Runtime::new().unwrap());
    let (commands, _) = tokio::sync::mpsc::channel(1);
    let handle = cx.add_window(|_, cx| Files::new(commands, runtime, directory.clone(), cx));
    handle
        .update(cx, |files, _, _| {
            for operation in [files.operation.take(), files.local_operation.take()]
                .into_iter()
                .flatten()
            {
                operation.abort();
            }
            files.closing = true;
            files.local = Some(local::Directory {
                path: directory.join("current_directory"),
                entries: Vec::new(),
                drives: Vec::new(),
            });
            files.directory = Some(latch::sftp::Directory {
                path:
                    "/home/test/a_long_parent_directory/another_parent_directory/current_directory"
                        .into(),
                entries: Vec::new(),
            });
            files.reveal_breadcrumbs(Side::Local);
            files.reveal_breadcrumbs(Side::Remote);
        })
        .unwrap();
    let mut visual = VisualTestContext::from_window(*handle, cx);
    visual.simulate_resize(size(px(800.), px(600.)));
    draw(handle, &mut visual);
    // GPUI's test platform discards next-frame callbacks.
    handle
        .update(&mut visual, |files, _, _| {
            files.scroll_breadcrumbs(Side::Local);
            files.scroll_breadcrumbs(Side::Remote);
            assert_eq!(files.breadcrumb_reveal, [false; 2]);
        })
        .unwrap();
    draw(handle, &mut visual);
    (handle, visual)
}

fn draw(handle: WindowHandle<Files>, cx: &mut VisualTestContext) {
    let root = handle.root(cx).unwrap();
    cx.draw(point(px(0.), px(0.)), size(px(800.), px(600.)), |_, _| {
        root.into_any_element()
    });
}

#[gpui::test]
fn current_directory_is_revealed_and_manual_scroll_survives_renders(cx: &mut TestAppContext) {
    let (handle, mut visual) = fixture(cx);
    for side in [Side::Local, Side::Remote] {
        let last = handle
            .read_with(&visual, |files, _| match side {
                Side::Local => super::local_crumbs(&files.local.as_ref().unwrap().path).len() - 1,
                Side::Remote => {
                    super::remote_crumbs(&files.directory.as_ref().unwrap().path).len() - 1
                }
            })
            .unwrap();
        handle
            .update(&mut visual, |files, _, cx| {
                let scroll = match side {
                    Side::Local => &files.local_breadcrumb_scroll,
                    Side::Remote => &files.remote_breadcrumb_scroll,
                };
                let viewport = scroll.bounds();
                let current = scroll.bounds_for_item(last).unwrap();
                assert!(current.left() + scroll.offset().x >= viewport.left());
                assert!(
                    current.right() + scroll.offset().x <= viewport.right(),
                    "{side:?}: current={current:?}, viewport={viewport:?}, offset={:?}",
                    scroll.offset()
                );
                assert!(scroll.offset().x < px(0.));
                scroll.set_offset(point(px(0.), px(0.)));
                cx.notify();
            })
            .unwrap();
        draw(handle, &mut visual);
        draw(handle, &mut visual);
        handle
            .read_with(&visual, |files, _| {
                let scroll = match side {
                    Side::Local => &files.local_breadcrumb_scroll,
                    Side::Remote => &files.remote_breadcrumb_scroll,
                };
                assert_eq!(scroll.offset().x, px(0.));
                let first = scroll.bounds_for_item(0).unwrap();
                assert!(first.left() >= scroll.bounds().left());
                assert!(first.right() <= scroll.bounds().right());
            })
            .unwrap();
    }
}
