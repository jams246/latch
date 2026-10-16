use gpui::{prelude::*, *};

pub(super) fn cell(value: String, width: f32) -> Div {
    div()
        .w(px(width))
        .flex_shrink_0()
        .px_2()
        .text_xs()
        .truncate()
        .child(value)
}

pub(super) fn modified(seconds: Option<u64>) -> String {
    seconds
        .and_then(|seconds| i64::try_from(seconds).ok())
        .and_then(|seconds| chrono::DateTime::from_timestamp(seconds, 0))
        .map_or_else(
            || "--".into(),
            |date| date.format("%Y-%m-%d %H:%M").to_string(),
        )
}

pub(in crate::ui::files) fn display_size(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64;
    for unit in ["KB", "MB", "GB", "TB", "PB", "EB"] {
        value /= 1024.;
        if value < 1024. || unit == "EB" {
            return if value < 10. {
                format!("{value:.1} {unit}")
            } else {
                format!("{value:.0} {unit}")
            };
        }
    }
    format!("{bytes} B")
}

#[cfg(test)]
mod tests {
    use super::{display_size, modified};

    #[test]
    fn sizes_stay_readable_in_compact_columns() {
        assert_eq!(display_size(1023), "1023 B");
        assert_eq!(display_size(1024), "1.0 KB");
        assert_eq!(display_size(1_048_576), "1.0 MB");
        assert_eq!(display_size(u64::MAX), "16 EB");
    }

    #[test]
    fn dates_use_utc_and_unavailable_dates_have_placeholder() {
        assert_eq!(modified(Some(0)), "1970-01-01 00:00");
        assert_eq!(modified(None), "--");
        assert_eq!(modified(Some(u64::MAX)), "--");
    }
}
