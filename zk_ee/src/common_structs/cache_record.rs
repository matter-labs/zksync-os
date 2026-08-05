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

    #[test]
    fn finish_deconstruction_works() {
        let mut cache_record: CacheRecord<i32, u32> = CacheRecord::new(5, Appearance::Retrieved);

        // First deconstruct the record
        cache_record.deconstruct();
        assert_eq!(cache_record.appearance, Appearance::Deconstructed);

        // finish_deconstruction should change appearance from Deconstructed to Updated
        let result = cache_record.finish_deconstruction();
        assert!(
            result.is_ok(),
            "finish_deconstruction should succeed on deconstructed record"
        );
        assert_eq!(
            cache_record.appearance,
            Appearance::Updated,
            "finish_deconstruction should change appearance to Updated"
        );

        // Value should remain unchanged
        assert_eq!(
            cache_record.value, 5,
            "finish_deconstruction should not modify the value"
        );
    }

    #[test]
    fn finish_deconstruction_fails_on_non_deconstructed() {
        let mut cache_record: CacheRecord<i32, u32> = CacheRecord::new(5, Appearance::Retrieved);

        // Should fail on non-deconstructed record
        let result = cache_record.finish_deconstruction();
        assert!(
            result.is_err(),
            "finish_deconstruction should fail on non-deconstructed record"
        );
        assert_eq!(
            cache_record.appearance,
            Appearance::Retrieved,
            "appearance should remain unchanged on failure"
        );

        // Test with other appearances
        cache_record.unset();
        let result = cache_record.finish_deconstruction();
        assert!(
            result.is_err(),
            "finish_deconstruction should fail on unset record"
        );

        // Test after update
        let mut cache_record: CacheRecord<i32, u32> = CacheRecord::new(5, Appearance::Updated);
        let result = cache_record.finish_deconstruction();
        assert!(
            result.is_err(),
            "finish_deconstruction should fail on updated record"
        );
    }

    #[test]
    fn deconstructed_account_lifecycle() {
        let mut cache_record: CacheRecord<i32, u32> = CacheRecord::new(5, Appearance::Retrieved);

        // 1. Account gets deconstructed
        cache_record.deconstruct();
        assert_eq!(cache_record.appearance, Appearance::Deconstructed);

        // 2. In finish_tx, we call finish_deconstruction before update
        cache_record
            .finish_deconstruction()
            .expect("Should finish deconstruction");
        assert_eq!(cache_record.appearance, Appearance::Updated);

        // 3. Now we can update the account normally
        cache_record
            .update(|v, _| {
                *v = 0; // Set to some value
                Ok(())
            })
            .expect("Should update successfully");

        assert_eq!(cache_record.appearance, Appearance::Updated);
        assert_eq!(cache_record.value, 0);
    }
}
