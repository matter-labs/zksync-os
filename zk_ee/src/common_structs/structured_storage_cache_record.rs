//! Wraps values with additional metadata used by IO caches

use core::fmt::Debug;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
/// Encodes state of cache element
pub enum StorageInitialAppearance {
    /// Represent uninitialized element - it doesn't exist in persistent form, so it it would be modified
    /// into non-trivial state, then it would need to be persisted as "insert"
    Empty,
    /// Populated with some preexisted value
    Existing,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
/// Encodes state of cache element
pub enum StorageCurrentAppearance {
    /// Represent kind-of uninitialized element - it may or may not exist in persistent form, but it was "declared"
    /// to be in cache for some reason, but was not yet read (observed)
    Touched,
    /// Represent the value that was "observed", but maybe was not modified
    Observed,
    /// Cache value was potentially changed compared to initial value
    Updated,
    /// Element was deleted (not just set to zero, but explicitly)
    Deleted,
}

#[derive(Clone, Copy, Debug)]
pub struct StorageCacheAppearance {
    initial_appearance: StorageInitialAppearance,
    current_appearance: StorageCurrentAppearance,
}

impl StorageCacheAppearance {
    pub fn new(
        initial_appearance: StorageInitialAppearance,
        current_appearance: StorageCurrentAppearance,
    ) -> Self {
        Self {
            initial_appearance,
            current_appearance,
        }
    }

    pub fn initial_appearance(&self) -> StorageInitialAppearance {
        self.initial_appearance
    }

    pub fn current_appearance(&self) -> StorageCurrentAppearance {
        self.current_appearance
    }

    /// Sets appearance to "observed" to distinguish from elements that were "observed" via explicit read
    /// or update. If it was observed before - does nothing
    pub fn observe(&mut self) {
        if self.current_appearance == StorageCurrentAppearance::Touched {
            self.current_appearance = StorageCurrentAppearance::Observed;
        };
    }

    /// Mark element as "update", meaning it was written to, but net difference can be trivial anyway
    pub fn update(&mut self) {
        self.current_appearance = StorageCurrentAppearance::Updated;
    }

    /// Mark element as "update", meaning it was written to, but net difference can be trivial anyway
    pub fn delete(&mut self) {
        self.current_appearance = StorageCurrentAppearance::Deleted;
    }
}
