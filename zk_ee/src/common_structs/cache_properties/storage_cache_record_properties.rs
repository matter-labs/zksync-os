//! Wraps values with additional metadata used by IO caches

use core::fmt::Debug;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
/// Encodes state of cache element
pub enum StorageInitialAppearance {
    /// Represent uninitialized element - it doesn't exist in persistent form, so it it would be modified
    /// into non-trivial state, then it would need to be persisted as "insert"
    NonExisting,
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
pub struct StorageCacheRecordProperties {
    initial_appearance: StorageInitialAppearance,
    current_appearance: StorageCurrentAppearance,
}

impl StorageCacheRecordProperties {
    pub fn new(is_new_storage_slot: bool) -> Self {
        let initial_appearance = if is_new_storage_slot {
            // Didn't exist before
            StorageInitialAppearance::NonExisting
        } else {
            StorageInitialAppearance::Existing
        };

        Self {
            initial_appearance,
            current_appearance: StorageCurrentAppearance::Observed,
        }
    }

    /// Returns true if slot didn't exist before
    pub fn is_new_storage_slot(&self) -> bool {
        self.initial_appearance == StorageInitialAppearance::NonExisting
    }

    /// Returns true if the initial value from storage was accessed/used.
    /// This excludes slots that were only touched but never observed, updated, or deleted.
    pub fn is_initial_value_used(&self) -> bool {
        matches!(
            self.current_appearance,
            StorageCurrentAppearance::Observed
                | StorageCurrentAppearance::Updated
                | StorageCurrentAppearance::Deleted
        )
    }

    /// Sets appearance to "observed" to distinguish from elements that were "observed" via explicit read
    /// or update. If it was observed before - does nothing
    pub fn mark_as_observed(&mut self) {
        if self.current_appearance == StorageCurrentAppearance::Touched {
            self.current_appearance = StorageCurrentAppearance::Observed;
        };
    }

    /// Mark element as "update", meaning it was written to, but net difference can be trivial anyway
    pub fn mark_as_updated(&mut self) {
        self.current_appearance = StorageCurrentAppearance::Updated;
    }

    /// Mark element as "update", meaning it was written to, but net difference can be trivial anyway
    pub fn mark_as_deleted(&mut self) {
        self.current_appearance = StorageCurrentAppearance::Deleted;
    }
}
