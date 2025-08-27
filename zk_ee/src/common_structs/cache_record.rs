//! Wraps values with additional metadata used by IO caches

use crate::{
    internal_error,
    system::errors::{internal::InternalError, system::SystemError},
};
use core::fmt::Debug;

#[derive(Default, Copy, Clone, Eq, PartialEq, Debug)]
/// Encodes state of cache element
pub enum Appearance {
    #[default]
    /// Represent uninitialized IO element
    Unset,
    /// Populated with some preexisted value
    Retrieved,
    /// Cache value changed compared to initial value
    Updated,
    /// Used for destructed accounts
    Deconstructed,
}

#[derive(Clone, Default)]
/// A cache entry. Wraps actual value with some metadata used by caches.
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

    // TODO: it doesn't make sense to have this method here, will be
    // moved in next release.
    pub fn finish_deconstruction(&mut self) -> Result<(), InternalError> {
        if self.appearance == Appearance::Deconstructed {
            self.appearance = Appearance::Updated;
            Ok(())
        } else {
            Err(internal_error!("Cannot finish deconstruction",))
        }
    }

    #[must_use]
    /// Updates value and metadata using callback. Changes appearance to Updated.
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

#[cfg(test)]
mod tests {
    use super::{Appearance, CacheRecord};

    #[test]
    fn update_works_and_changes_appearance() {
        let mut cache_record: CacheRecord<i32, u32> = CacheRecord::new(5, Appearance::Retrieved);
        cache_record
            .update(|v, _| {
                *v = 4;
                Ok(())
            })
            .expect("Correct update");

        assert_eq!(cache_record.value, 4);
        assert_eq!(cache_record.appearance, Appearance::Updated);
    }

    #[test]
    fn metadata_update_keeps_appearance() {
        let mut cache_record: CacheRecord<i32, u32> = CacheRecord::new(5, Appearance::Retrieved);
        cache_record
            .update_metadata(|m| {
                *m = 4;
                Ok(())
            })
            .expect("Correct update");

        assert_eq!(cache_record.appearance, Appearance::Retrieved);
    }

    #[test]
    fn deconstruct_works() {
        let mut cache_record: CacheRecord<i32, u32> = CacheRecord::new(5, Appearance::Retrieved);
        cache_record.deconstruct();

        assert_eq!(cache_record.appearance, Appearance::Deconstructed);
    }

    #[test]
    fn unset_works() {
        let mut cache_record: CacheRecord<i32, u32> = CacheRecord::new(5, Appearance::Retrieved);
        cache_record.unset();

        assert_eq!(cache_record.appearance, Appearance::Unset);
    }
}
