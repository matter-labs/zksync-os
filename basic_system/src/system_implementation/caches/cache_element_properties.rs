use zk_ee::internal_error;
use zk_ee::system::errors::internal::InternalError;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
/// Tracks whether a cache element exists in persistent storage
enum CacheElementPersistenceStatus {
    /// Not known yet: no value was declared for the element
    /// (see `CacheElementValueStatus::Undefined`)
    Unknown,
    /// Element doesn't exist in persistent storage. If modified to a non-trivial state,
    /// it will need to be persisted as an "insert" operation
    NonExisting,
    /// Element was populated with a pre-existing value from storage
    Existing,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
/// Tracks what is known about a cache element's value. The status only moves
/// forward (`Undefined` -> `WitnessDeclared` -> `Observed`) and, unlike the
/// element's records, is never rolled back: knowing a value is a fact about the
/// whole history of the element.
enum CacheElementValueStatus {
    /// The element was warmed up (access list, precompile warm-up) but no
    /// value was ever declared for it: the records hold a placeholder, nothing
    /// was asked from the witness and no proof obligation exists
    Undefined,
    /// The witness declared a value for the element (it was loaded from the
    /// oracle), but nothing read, updated or otherwise observed it yet, so it
    /// still carries no proof obligation. It may be cold or warm
    WitnessDeclared,
    /// The declared value was read, updated or otherwise observed: it is part of
    /// the state transition and must be verified against the state commitment
    Observed,
}

#[derive(Clone, Copy, Debug)]
pub struct CacheElementProperties {
    persistent_storage_status: CacheElementPersistenceStatus,
    cache_value_status: CacheElementValueStatus,
}

impl CacheElementProperties {
    /// An element whose value was declared by the witness, observed or not
    pub fn new(is_new_element: bool, is_value_observed: bool) -> Self {
        let persistent_storage_status = if is_new_element {
            CacheElementPersistenceStatus::NonExisting
        } else {
            CacheElementPersistenceStatus::Existing
        };

        let cache_value_status = if is_value_observed {
            CacheElementValueStatus::Observed
        } else {
            CacheElementValueStatus::WitnessDeclared
        };

        Self {
            persistent_storage_status,
            cache_value_status,
        }
    }

    /// An element that was only warmed up: no value was declared for it yet
    pub fn undefined() -> Self {
        Self {
            persistent_storage_status: CacheElementPersistenceStatus::Unknown,
            cache_value_status: CacheElementValueStatus::Undefined,
        }
    }

    /// Whether the witness declared a value for the element (observed or not)
    pub fn is_value_declared(&self) -> bool {
        self.cache_value_status != CacheElementValueStatus::Undefined
    }

    /// Records that the witness declared the value of an undefined element.
    pub fn mark_value_as_declared(&mut self, is_new_element: bool) {
        debug_assert!(!self.is_value_declared());
        self.persistent_storage_status = if is_new_element {
            CacheElementPersistenceStatus::NonExisting
        } else {
            CacheElementPersistenceStatus::Existing
        };
        self.cache_value_status = CacheElementValueStatus::WitnessDeclared;
    }

    /// Returns true if the element didn't exist in persistent storage before.
    /// Only meaningful once a value is declared.
    pub fn is_new_element(&self) -> bool {
        debug_assert!(self.is_value_declared());
        self.persistent_storage_status == CacheElementPersistenceStatus::NonExisting
    }

    /// Returns true if the initial value from storage was accessed/used.
    /// This excludes records that were only touched or declared but never observed,
    /// updated, or deleted.
    pub fn is_value_observed(&self) -> bool {
        matches!(self.cache_value_status, CacheElementValueStatus::Observed)
    }

    /// Marks the declared value as observed/accessed. Only `WitnessDeclared`
    /// moves to `Observed`; observing an already observed value is a no-op, and
    /// an undefined element has no value to observe, so that is an error: the
    /// value must be declared first.
    #[must_use]
    pub fn mark_value_as_observed(&mut self) -> Result<(), InternalError> {
        match self.cache_value_status {
            CacheElementValueStatus::WitnessDeclared => {
                self.cache_value_status = CacheElementValueStatus::Observed;
                Ok(())
            }
            CacheElementValueStatus::Observed => Ok(()),
            CacheElementValueStatus::Undefined => Err(internal_error!(
                "can not observe a cache element with an undefined value"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_status_only_moves_forward() {
        let mut undefined = CacheElementProperties::undefined();
        assert!(!undefined.is_value_declared());
        assert!(!undefined.is_value_observed());
        assert!(
            undefined.mark_value_as_observed().is_err(),
            "nothing to observe before the witness declares a value"
        );
        assert!(!undefined.is_value_observed());

        undefined.mark_value_as_declared(true);
        assert!(undefined.is_value_declared());
        assert!(undefined.is_new_element());
        assert!(!undefined.is_value_observed());
        undefined.mark_value_as_observed().unwrap();
        assert!(undefined.is_value_observed());
        // idempotent
        undefined.mark_value_as_observed().unwrap();
        assert!(undefined.is_value_observed());

        let mut declared = CacheElementProperties::new(false, false);
        assert!(declared.is_value_declared() && !declared.is_value_observed());
        declared.mark_value_as_observed().unwrap();
        assert!(declared.is_value_observed());
    }
}
