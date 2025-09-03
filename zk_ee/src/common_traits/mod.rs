use core::alloc::Allocator;

pub mod key_like_with_bounds;

/// Custom version of Extend, but fallible
pub trait TryExtend<T> {
    type Error;

    fn try_extend<I>(&mut self, iter: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = T>;
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
