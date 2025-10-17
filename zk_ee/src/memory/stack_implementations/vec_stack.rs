use alloc::vec::Vec;
use core::alloc::Allocator;

use crate::memory::stack_trait::{Stack, StackFactory};

/// Vec based implementation of the Stack trait.
///
/// This is the standard implementation used in forward execution environments
/// where memory allocation and resizing are allowed.
impl<T: Sized, A: Allocator> Stack<T, A> for Vec<T, A> {
    fn new_in(alloc: A) -> Self {
        Vec::new_in(alloc)
    }

    fn len(&self) -> usize {
        Vec::len(self)
    }

    fn push(&mut self, value: T) {
        // We allow resize here for forward execution environments
        Vec::push(self, value);
    }

    fn pop(&mut self) -> Option<T> {
        Vec::pop(self)
    }

    fn top(&self) -> Option<&T> {
        self.last()
    }

    fn top_mut(&mut self) -> Option<&mut T> {
        self.last_mut()
    }

    fn clear(&mut self) {
        Vec::clear(self)
    }

    fn truncate(&mut self, new_len: usize) {
        Vec::truncate(self, new_len);
    }

    fn iter<'a>(&'a self) -> impl ExactSizeIterator<Item = &'a T> + Clone
    where
        T: 'a,
    {
        self[..].iter()
    }

    fn iter_skip_n<'a>(&'a self, n: usize) -> impl ExactSizeIterator<Item = &'a T> + Clone
    where
        T: 'a,
    {
        self[n..].iter()
    }
}

/// This factory is used in forward execution where memory can be allocated
/// and resized freely.
pub struct VecStackFactory {}

impl<const M: usize> StackFactory<M> for VecStackFactory {
    /// Vec ignores const parameters N and M.
    type Stack<T: Sized, const N: usize, A: Allocator + Clone> = Vec<T, A>;

    /// Creates a new Vec-based stack with the given allocator.
    fn new_in<T, A: Allocator + Clone>(alloc: A) -> Self::Stack<T, M, A> {
        Self::Stack::<T, M, A>::new_in(alloc)
    }
}
