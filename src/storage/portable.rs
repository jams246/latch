use std::{io, path::Path};

use super::{Configuration, Store, files, locking, pair, recovery};

pub(super) const DATABASE: &str = "latch.db";

impl Store {
    /// Loads the current encrypted profile, creating one only when absent.
    ///
    /// # Errors
    /// Reports locked, damaged, or interrupted profiles without resetting them.
    pub fn load_profile(directory: &Path) -> io::Result<Self> {
        let lock = locking::acquire(directory)?;
        if files::exists(&directory.join(recovery::MARKER))? {
            return Err(recovery::interrupted());
        }
        if !pair::present(directory)? {
            if pair::leftovers(directory)? {
                return Err(recovery::interrupted());
            }
            let configuration = Configuration::default();
            let stage = recovery::stage(directory, &configuration)?;
            recovery::publish(&stage, directory, &configuration)?;
        }
        pair::require(directory)?;
        let mut store = Self::load(directory.join(DATABASE))?;
        store._lock = Some(lock);
        Ok(store)
    }
}
