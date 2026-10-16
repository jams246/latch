use super::Error;

const MAX_ENTRIES: usize = 100_000;
const MAX_PATH_BYTES: usize = 64 * 1024 * 1024;
const MAX_DEPTH: usize = 64;

#[derive(Default)]
pub(super) struct TreeLimits {
    entries: usize,
    path_bytes: usize,
}

impl TreeLimits {
    pub(super) fn reserve(&mut self, depth: usize, path_bytes: usize) -> Result<(), Error> {
        // Bound retained paths and recursion before a server-controlled child is queued.
        if depth > MAX_DEPTH
            || self.entries == MAX_ENTRIES
            || path_bytes > MAX_PATH_BYTES.saturating_sub(self.path_bytes)
        {
            return Err(Error::TreeLimit);
        }
        self.entries += 1;
        self.path_bytes += path_bytes;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_limits_reject_excess_depth_entries_and_path_storage() {
        for (mut budget, depth, bytes) in [
            (TreeLimits::default(), MAX_DEPTH + 1, 1),
            (
                TreeLimits {
                    entries: MAX_ENTRIES,
                    path_bytes: 0,
                },
                0,
                1,
            ),
            (
                TreeLimits {
                    entries: 0,
                    path_bytes: MAX_PATH_BYTES,
                },
                0,
                1,
            ),
        ] {
            assert!(matches!(
                budget.reserve(depth, bytes),
                Err(Error::TreeLimit)
            ));
        }
    }
}
