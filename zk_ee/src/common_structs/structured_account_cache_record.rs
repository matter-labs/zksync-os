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
pub struct AccountCacheAppearance {
    initial_appearance: AccountInitialAppearance,
    current_appearance: AccountCurrentAppearance,
}

impl AccountCacheAppearance {
    pub fn new(
        initial_appearance: AccountInitialAppearance,
        current_appearance: AccountCurrentAppearance,
    ) -> Self {
        Self {
            initial_appearance,
            current_appearance,
        }
    }

    pub fn initial_appearance(&self) -> AccountInitialAppearance {
        self.initial_appearance
    }

    pub fn current_appearance(&self) -> AccountCurrentAppearance {
        self.current_appearance
    }

    /// Sets appearance to "observed" to distinguish from elements that were "observed" via explicit read
    /// or update. If it was observed before - does nothing
    pub fn observe(&mut self) {
        self.current_appearance = AccountCurrentAppearance::Observed;
    }

    /// Sets appearance to "observed" after deconstruction
    pub fn assert_observed(&self) {
        assert!(self.current_appearance == AccountCurrentAppearance::Observed);
    }
}
