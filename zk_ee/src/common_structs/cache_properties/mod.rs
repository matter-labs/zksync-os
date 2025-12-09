#[derive(Copy, Clone, Eq, PartialEq, Debug)]
/// Encodes state of cache element
pub enum CacheElementPersistenceStatus {
    /// Represent uninitialized element - it doesn't exist in persistent form, so it it would be modified
    /// into non-trivial state, then it would need to be persisted as "insert"
    NonExisting,
    /// Populated with some preexisted value
    Existing,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
/// Encodes state of cache element
pub enum CacheElementValueSatus {
    /// Represent kind-of uninitialized element - it may or may not exist in persistent form, but it was "declared"
    /// to be in cache for some reason, but was not yet read (observed)
    Undefined,
    /// Represent the value that was "observed", but maybe was not modified
    Materialized,
}

#[derive(Clone, Copy, Debug)]
pub struct CacheElementProperties {
    persistent_storage_status: CacheElementPersistenceStatus,
    cache_value_status: CacheElementValueSatus,
}

impl CacheElementProperties {
    pub fn new(is_new_account: bool, observed: bool) -> Self {
        let persistent_storage_status = if is_new_account {
            CacheElementPersistenceStatus::NonExisting
        } else {
            CacheElementPersistenceStatus::Existing
        };

        let cache_value_status = if observed {
            CacheElementValueSatus::Materialized
        } else {
            CacheElementValueSatus::Undefined
        };

        Self {
            persistent_storage_status,
            cache_value_status,
        }
    }

    /// Returns true if didn't exist in persistent storage before
    pub fn is_new_element(&self) -> bool {
        self.persistent_storage_status == CacheElementPersistenceStatus::NonExisting
    }

    /// Returns true if the initial value from storage was accessed/used.
    /// This excludes records that were only touched but never observed, updated, or deleted.
    pub fn is_value_known(&self) -> bool {
        matches!(
            self.cache_value_status,
            CacheElementValueSatus::Materialized
        )
    }

    pub fn mark_value_as_known(&mut self) {
        if self.cache_value_status == CacheElementValueSatus::Undefined {
            self.cache_value_status = CacheElementValueSatus::Materialized;
        };
    }
}
