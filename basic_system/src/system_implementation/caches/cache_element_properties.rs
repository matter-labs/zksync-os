#[derive(Copy, Clone, Eq, PartialEq, Debug)]
/// Tracks whether a cache element exists in persistent storage
enum CacheElementPersistenceStatus {
    /// Element doesn't exist in persistent storage. If modified to a non-trivial state,
    /// it will need to be persisted as an "insert" operation
    NonExisting,
    /// Element was populated with a pre-existing value from storage
    Existing,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
/// Tracks whether a cache element's value has been observed/accessed
enum CacheElementValueStatus {
    /// Element exists in cache but its value hasn't been read or observed yet.
    /// It was declared in cache for some reason but remains unaccessed
    Undefined,
    /// Element's value has been observed/accessed, but may or may not have been modified
    Materialized,
}

#[derive(Clone, Copy, Debug)]
pub struct CacheElementProperties {
    persistent_storage_status: CacheElementPersistenceStatus,
    cache_value_status: CacheElementValueStatus,
}

impl CacheElementProperties {
    pub fn new(is_new_element: bool, is_value_known: bool) -> Self {
        let persistent_storage_status = if is_new_element {
            CacheElementPersistenceStatus::NonExisting
        } else {
            CacheElementPersistenceStatus::Existing
        };

        let cache_value_status = if is_value_known {
            CacheElementValueStatus::Materialized
        } else {
            CacheElementValueStatus::Undefined
        };

        Self {
            persistent_storage_status,
            cache_value_status,
        }
    }

    /// Returns true if the element didn't exist in persistent storage before
    pub fn is_new_element(&self) -> bool {
        self.persistent_storage_status == CacheElementPersistenceStatus::NonExisting
    }

    /// Returns true if the initial value from storage was accessed/used.
    /// This excludes records that were only touched but never observed, updated, or deleted.
    pub fn is_value_known(&self) -> bool {
        matches!(
            self.cache_value_status,
            CacheElementValueStatus::Materialized
        )
    }

    /// Marks the cache element's value as having been observed/accessed
    pub fn mark_value_as_known(&mut self) {
        if self.cache_value_status == CacheElementValueStatus::Undefined {
            self.cache_value_status = CacheElementValueStatus::Materialized;
        };
    }
}
