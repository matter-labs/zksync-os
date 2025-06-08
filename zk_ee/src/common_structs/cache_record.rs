use crate::system::errors::{InternalError, SystemError};

use core::fmt::Debug;

// TODO move to some proper place

#[derive(Default, Copy, Clone, Eq, PartialEq, Debug)]
pub enum Appearance {
    #[default]
    Unset,
    Retrieved,
    Updated,
    Deconstructed,
}

#[derive(Clone, Default)]
/// A cache entry. User facing struct.
pub struct CacheRecord<V, M> {
    appearance: Appearance,
    value: V,
    metadata: M,
}

impl<V, M: Default> CacheRecord<V, M> {
    pub fn new(value: V, appearance: Appearance) -> Self {
        Self {
            appearance,
            value,
            metadata: Default::default(),
        }
    }
}

impl<V, M> CacheRecord<V, M> {
    pub fn appearance(&self) -> Appearance {
        self.appearance
    }

    pub fn value(&self) -> &V {
        &self.value
    }

    pub fn metadata(&self) -> &M {
        &self.metadata
    }

    #[must_use]
    pub fn update<F>(&mut self, f: F) -> Result<(), InternalError>
    where
        F: FnOnce(&mut V, &mut M) -> Result<(), InternalError>,
    {
        if self.appearance != Appearance::Deconstructed {
            self.appearance = Appearance::Updated
        };

        f(&mut self.value, &mut self.metadata)
    }

    #[must_use]
    /// Updates the metadata and retains the appearance.
    pub fn update_metadata<F>(&mut self, f: F) -> Result<(), SystemError>
    where
        F: FnOnce(&mut M) -> Result<(), SystemError>,
    {
        f(&mut self.metadata)
    }

    /// Sets appearance to deconstructed. The value itself remains untouched.
    pub fn deconstruct(&mut self) {
        self.appearance = Appearance::Deconstructed;
    }

    /// Sets appearance to unset. The value itself remains untouched.
    pub fn unset(&mut self) {
        self.appearance = Appearance::Unset;
    }
}
