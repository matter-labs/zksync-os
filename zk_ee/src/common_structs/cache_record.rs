//! Wraps values with additional metadata used by IO caches

use crate::system::errors::internal::InternalError;
use crate::system::errors::system::SystemError;

#[derive(Clone, Default)]
/// A cache entry. Wraps actual value with some metadata used by caches.
pub struct CacheRecord<V, M> {
    value: V,
    metadata: M,
}

impl<V, M: Default> CacheRecord<V, M> {
    pub fn new(value: V) -> Self {
        Self {
            value,
            metadata: Default::default(),
        }
    }
}

impl<V, M> CacheRecord<V, M> {
    pub fn value(&self) -> &V {
        &self.value
    }

    pub fn metadata(&self) -> &M {
        &self.metadata
    }

    #[must_use]
    /// Updates value and metadata using callback
    pub fn update<F>(&mut self, f: F) -> Result<(), InternalError>
    where
        F: FnOnce(&mut V, &mut M) -> Result<(), InternalError>,
    {
        f(&mut self.value, &mut self.metadata)
    }

    #[must_use]
    /// Updates the metadata
    pub fn update_metadata<F>(&mut self, f: F) -> Result<(), SystemError>
    where
        F: FnOnce(&mut M) -> Result<(), SystemError>,
    {
        f(&mut self.metadata)
    }
}
