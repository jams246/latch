use super::{Metrics, remaining_label};
use crate::ui::files::tests_interaction::{draw, fixture};
use gpui::TestAppContext;
use std::time::{Duration, Instant};

#[test]
fn current_rate_and_eta_follow_transferred_bytes() {
    let start = Instant::now();
    let now = start + Duration::from_secs(1);
    let mut metrics = Metrics::new(start);
    metrics.update("report.csv".into(), 1024, Some(4096), now);
    assert_eq!(metrics.speed(now), 1024.);
    assert_eq!(metrics.remaining(now), Some(3));
    assert_eq!(metrics.fraction(), 0.25);
    assert_eq!(metrics.size_label(), "1.0 KB / 4.0 KB");
}

#[test]
fn switching_files_preserves_folder_totals_and_progress() {
    let start = Instant::now();
    let mut metrics = Metrics::new(start);
    metrics.update(
        "first.txt".into(),
        1024,
        Some(4096),
        start + Duration::from_secs(1),
    );
    metrics.update(
        "second.txt".into(),
        3072,
        Some(4096),
        start + Duration::from_secs(2),
    );
    assert_eq!(metrics.name, "second.txt");
    assert_eq!(metrics.fraction(), 0.75);
    assert_eq!(metrics.remaining(start + Duration::from_secs(2)), Some(1));
}

#[test]
fn stalled_transfers_stop_reporting_old_speed_or_eta() {
    let start = Instant::now();
    let mut metrics = Metrics::new(start);
    metrics.update(
        "report.csv".into(),
        1024,
        Some(4096),
        start + Duration::from_secs(1),
    );
    let stalled = start + Duration::from_secs(5);
    assert_eq!(metrics.speed(stalled), 0.);
    assert_eq!(metrics.remaining(stalled), None);
}

#[test]
fn speed_window_ignores_older_samples_and_bounds_sample_count() {
    let start = Instant::now();
    let mut metrics = Metrics::new(start);
    for milliseconds in 1..=10_000 {
        metrics.update(
            "file".into(),
            milliseconds,
            None,
            start + Duration::from_millis(milliseconds),
        );
    }
    assert_eq!(metrics.speed(start + Duration::from_secs(10)), 1000.);
    assert!(metrics.samples.len() <= 13);
}

#[test]
fn unknown_empty_and_oversized_totals_are_safe() {
    let now = Instant::now();
    let mut metrics = Metrics::new(now);
    assert_eq!(metrics.speed(now), 0.);
    assert_eq!(metrics.fraction(), 0.);
    assert_eq!(metrics.remaining(now), None);
    assert_eq!(metrics.size_label(), "0 B / --");
    metrics.update("empty".into(), 0, Some(0), now);
    assert_eq!(metrics.fraction(), 1.);
    assert_eq!(metrics.remaining(now), Some(0));
    metrics.update("changed".into(), 1024, Some(512), now);
    assert_eq!(metrics.fraction(), 1.);
    assert_eq!(metrics.remaining(now), Some(0));
}

#[test]
fn remaining_time_stays_readable() {
    assert_eq!(remaining_label(None), "-- remaining");
    assert_eq!(remaining_label(Some(0)), "0s remaining");
    assert_eq!(remaining_label(Some(59)), "59s remaining");
    assert_eq!(remaining_label(Some(61)), "1m 1s remaining");
    assert_eq!(remaining_label(Some(3661)), "1h 1m remaining");
}

#[gpui::test]
fn pane_headers_stay_aligned_with_a_status_in_either_pane(cx: &mut TestAppContext) {
    for (local, remote, local_action, remote_action, busy) in [
        ("Local warning", "", None, None, false),
        ("", "Remote notice", None, None, false),
        ("", "", Some("Local action"), None, false),
        ("", "", None, Some("Remote action"), false),
        ("", "", None, None, true),
        ("", "", None, None, false),
    ] {
        let (handle, mut visual) = fixture(cx);
        handle
            .update(&mut visual, |files, _, _| {
                files.local_status = local.into();
                files.status = remote.into();
                files.action_status = [
                    local_action.map(str::to_string),
                    remote_action.map(str::to_string),
                ];
                files.local_busy = false;
                files.busy = busy;
            })
            .unwrap();
        draw(handle, &mut visual);
        let visible = !local.is_empty()
            || !remote.is_empty()
            || local_action.is_some()
            || remote_action.is_some()
            || busy;
        assert_eq!(visual.debug_bounds("local-status").is_some(), visible);
        assert_eq!(visual.debug_bounds("remote-status").is_some(), visible);
        assert_eq!(
            visual.debug_bounds("local-column-0").unwrap().origin.y,
            visual.debug_bounds("remote-column-0").unwrap().origin.y
        );
    }
}
