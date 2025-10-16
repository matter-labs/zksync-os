//! Quasi-vector implementation that uses a chain of fixed-size allocated chunks
//!
//! This module provides a vector-like data structure that uses a chain of fixed-size
//! allocated chunks instead of a single contiguous buffer.

use alloc::collections::LinkedList;
use arrayvec::ArrayVec;
use core::alloc::Allocator;

pub const PAGE_SIZE: usize = 4096;

/// A quasi-vector that stores elements in a linked list of fixed-size chunks.
///
/// # Key Properties
/// - **Predictable allocation**: Memory is allocated in fixed-size chunks
/// - **No reallocation**: Unlike Vec, never needs to reallocate existing data
///
/// # Invariants
/// - The last element in the list is never an empty array
/// - All elements except the last are completely full (contain N elements)
/// - Empty nodes are immediately removed
pub struct ListVec<T: Sized, const N: usize, A: Allocator>(pub LinkedList<ArrayVec<T, N>, A>);

impl<T: Sized, const N: usize, A: Allocator> core::fmt::Debug for ListVec<T, N, A>
where
    T: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("ListVec").field(&self.0).finish()
    }
}

pub const fn num_elements_in_backing_node<
    const PAGE_SIZE: usize,
    T: Sized,
    A: core::alloc::Allocator,
>() -> usize {
    use core::ptr::NonNull;

    // Calculate the overhead from LinkedList node structure:
    // - Two Option<NonNull<()>> for prev/next pointers
    // - ArrayVec<T, 0> metadata (length field, etc.)
    let mut min_consumed = core::mem::size_of::<Option<NonNull<()>>>()
        + core::mem::size_of::<Option<NonNull<()>>>()
        + core::mem::size_of::<ArrayVec<T, 0>>();

    let size = core::mem::size_of::<T>();
    let alignment = core::mem::align_of::<T>();

    // Align the overhead to match element alignment requirements
    if !min_consumed.is_multiple_of(alignment) {
        min_consumed += alignment - (min_consumed % alignment);
    }

    // Calculate effective element size including alignment padding
    let effective_size = size.next_multiple_of(alignment);

    // Determine how many elements fit in the remaining space
    let backing = (PAGE_SIZE - min_consumed) / effective_size;

    // Ensure at least one element fits (sanity check)
    assert!(backing > 0);

    backing
}

impl<T: Sized, const N: usize, A: Allocator + Clone> ListVec<T, N, A> {
    pub const fn new_in(allocator: A) -> Self {
        Self(LinkedList::new_in(allocator))
    }
}

/// Iterator over elements in a ListVec.
pub struct ListVecIter<'a, T: Sized, const N: usize> {
    /// Iterator over the LinkedList nodes (ArrayVec<T, N>)
    outer: alloc::collections::linked_list::Iter<'a, ArrayVec<T, N>>,
    /// Iterator over elements in the current node
    inner: Option<core::slice::Iter<'a, T>>,
    /// Number of elements remaining to be yielded
    remaining: usize,
}

impl<'a, T: Sized, const N: usize> Clone for ListVecIter<'a, T, N> {
    fn clone(&self) -> Self {
        Self {
            outer: self.outer.clone(),
            inner: self.inner.clone(),
            remaining: self.remaining,
        }
    }
}

impl<'a, T: Sized, const N: usize> ListVecIter<'a, T, N> {
    /// Creates a new iterator from its component parts.
    pub fn new_from_parts(
        outer: alloc::collections::linked_list::Iter<'a, ArrayVec<T, N>>,
        inner: Option<core::slice::Iter<'a, T>>,
        remaining: usize,
    ) -> Self {
        Self {
            outer,
            inner,
            remaining,
        }
    }
}

impl<'a, T: Sized, const N: usize> Iterator for ListVecIter<'a, T, N> {
    type Item = &'a T;

    /// Yields the next element from the ListVec.
    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.inner {
            None => {
                // Reached the end
                None
            }
            Some(inner) => match inner.next() {
                None => {
                    // By invariant
                    unreachable!()
                }
                Some(val) => {
                    self.remaining -= 1;

                    // Check if we've exhausted the current node
                    if inner.len() == 0 {
                        // Advance to the next node, maintaining the invariant
                        self.inner = self.outer.next().map(|v| v.iter());
                    }

                    Some(val)
                }
            },
        }
    }

    /// Returns the exact size hint for this iterator.
    ///
    /// Since we track the remaining count precisely, we can provide
    /// exact bounds for both lower and upper size hints.
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<'a, T: Sized, const N: usize> ExactSizeIterator for ListVecIter<'a, T, N> {}
