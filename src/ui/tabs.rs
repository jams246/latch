pub(super) fn active_after_close(
    active: Option<usize>,
    removed: usize,
    remaining: usize,
) -> Option<usize> {
    active.and_then(|active| {
        if remaining == 0 {
            None
        } else if active > removed {
            Some(active - 1)
        } else if active == removed {
            Some(removed.min(remaining - 1))
        } else {
            Some(active)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::active_after_close;

    #[test]
    fn closing_a_tab_preserves_active_identity_or_focuses_its_neighbor() {
        for (active, removed, remaining, expected) in [
            (None, 0, 0, None),
            (Some(0), 0, 0, None),
            (Some(2), 0, 2, Some(1)),
            (Some(0), 2, 2, Some(0)),
            (Some(1), 1, 2, Some(1)),
            (Some(2), 2, 2, Some(1)),
        ] {
            assert_eq!(active_after_close(active, removed, remaining), expected);
        }
    }
}
