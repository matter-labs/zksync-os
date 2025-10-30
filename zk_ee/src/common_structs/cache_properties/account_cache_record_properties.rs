//! Wraps values with additional metadata used by IO caches

use core::fmt::Debug;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
/// Encodes state of cache element
pub enum AccountInitialAppearance {
    /// Represent uninitialized element - it doesn't exist in persistent form, so it it would be modified
    /// into non-trivial state, then it would need to be persisted as "insert"
    Unset,
    /// Populated with some preexisted value
    Retrieved,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
/// Encodes state of cache element
pub enum AccountCurrentAppearance {
    /// Represent kind-of uninitialized element - it may or may not exist in persistent form, but it was "declared"
    /// to be in cache for some reason, but was not yet read (observed)
    Touched,
    /// Represent the value that was "observed", either via read or via modification
    Observed,
}

impl AccountCurrentAppearance {
    /// Mark as observed if it was only touched before
    pub fn observe(&mut self) {
        if *self == AccountCurrentAppearance::Touched {
            *self = AccountCurrentAppearance::Observed;
        };
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AccountCacheRecordProperties {
    initial_appearance: AccountInitialAppearance,
    current_appearance: AccountCurrentAppearance,
}

impl AccountCacheRecordProperties {
    pub fn new(is_new_account: bool, observe: bool) -> Self {
        let initial_appearance = if is_new_account {
            AccountInitialAppearance::Unset
        } else {
            AccountInitialAppearance::Retrieved
        };

        let current_appearance = if observe {
            AccountCurrentAppearance::Observed
        } else {
            AccountCurrentAppearance::Touched
        };

        Self {
            initial_appearance,
            current_appearance,
        }
    }

    /// Returns true if account didn't exist before
    pub fn is_new_account(&self) -> bool {
        self.initial_appearance == AccountInitialAppearance::Unset
    }

    /// Sets appearance to "observed" to distinguish from elements that were "observed" via explicit read
    /// or update. If it was observed before - does nothing
    pub fn mark_as_observed(&mut self) {
        self.current_appearance = AccountCurrentAppearance::Observed;
    }

    /// Asserts that the account cache entry has been observed (not just touched).
    /// Used for validation during account deconstruction to ensure proper cache state.
    pub fn assert_observed(&self) {
        assert!(self.current_appearance == AccountCurrentAppearance::Observed);
    }
}
