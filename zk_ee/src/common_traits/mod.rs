use core::alloc::Allocator;

pub mod key_like_with_bounds;

/// Custom version of Extend, but fallible
pub trait TryExtend<T> {
    type Error;

    fn try_extend<I>(&mut self, iter: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = T>;

    /// `try_extend` with the items of a slice; contiguous storage copies the slice in one go
    /// instead of item by item
    fn try_extend_from_slice(&mut self, items: &[T]) -> Result<(), Self::Error>
    where
        T: Copy,
    {
        self.try_extend(items.iter().copied())
    }
}

impl<A: Allocator, T> TryExtend<T> for alloc::vec::Vec<T, A> {
    type Error = ();

    fn try_extend<I>(&mut self, iter: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = T>,
    {
        self.extend(iter);
        Ok(())
    }
}
