use super::{local, tests_navigation::fixture};
use gpui::TestAppContext;
use std::{io, path::PathBuf, sync::mpsc};

fn directory(path: &str) -> local::Directory {
    local::Directory {
        path: path.into(),
        entries: Vec::new(),
        drives: Vec::new(),
    }
}

#[gpui::test]
fn accepted_local_results_request_persistence_with_the_source_identity(cx: &mut TestAppContext) {
    let files = fixture(cx);
    files.update(cx, |files, cx| {
        let (sender, events) = mpsc::channel();
        files.preference_sender = sender;
        files.local_sender.send(Ok(directory("selected"))).unwrap();
        assert!(events.try_recv().is_err());

        files.poll(cx);

        let preference = events.try_recv().unwrap();
        assert_eq!(preference.tab_id, files.tab_id);
        assert_eq!(preference.connection_id, files.connection_id);
        assert_eq!(preference.path, PathBuf::from("selected"));
        assert!(events.try_recv().is_err());
    });
}

#[gpui::test]
fn failed_or_closing_local_results_never_request_persistence(cx: &mut TestAppContext) {
    let files = fixture(cx);
    files.update(cx, |files, cx| {
        let (sender, events) = mpsc::channel();
        files.preference_sender = sender;
        files
            .local_sender
            .send(Err(io::Error::from(io::ErrorKind::NotFound)))
            .unwrap();
        files.poll(cx);
        assert!(events.try_recv().is_err());

        files.prepare_close(cx);
        files.local_sender.send(Ok(directory("late"))).unwrap();
        files.poll(cx);
        assert!(events.try_recv().is_err());
    });
}
