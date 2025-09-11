// Quasi-vector implementation that uses a chain of fixed-size allocated chunks

use alloc::collections::LinkedList;
use arrayvec::ArrayVec;
use core::alloc::Allocator;

// Invariants:
// - last element in list is never an empty array
// - all elements in the list except for the last are full
pub struct ListVec<T: Sized, const N: usize, A: Allocator>(LinkedList<ArrayVec<T, N>, A>);

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

    // Size of the two pointers for a linked list node
    // plus the ArrayVec overhead
    let mut min_consumed = core::mem::size_of::<Option<NonNull<()>>>()
        + core::mem::size_of::<Option<NonNull<()>>>()
        + core::mem::size_of::<ArrayVec<T, 0>>();
    let size = core::mem::size_of::<T>();
    let alignment = core::mem::align_of::<T>();
    if !min_consumed.is_multiple_of(alignment) {
        // align up
        min_consumed += alignment - (min_consumed % alignment);
    }

    let effective_size = size.next_multiple_of(alignment);
    let backing = (PAGE_SIZE - min_consumed) / effective_size;
    assert!(backing > 0);

    backing
}

impl<T: Sized, const N: usize, A: Allocator + Clone> ListVec<T, N, A> {
    pub const fn new_in(allocator: A) -> Self {
        Self(LinkedList::new_in(allocator))
    }
}

impl<T: Sized, const N: usize, A: Allocator + Clone> super::stack_trait::Stack<T, A>
    for ListVec<T, N, A>
{
    fn new_in(alloc: A) -> Self {
        ListVec::<T, N, A>::new_in(alloc)
    }

    fn push(&mut self, value: T) {
        match self.0.iter_mut().last() {
            None => {
                // Empty, create a new node and push there
                let mut new_node: ArrayVec<T, N> = ArrayVec::new();
                new_node.push(value);
                self.0.push_back(new_node)
            }
            Some(last_node) => {
                // Check if last node is full
                if last_node.is_full() {
                    // Push to new node
                    let mut new_node: ArrayVec<T, N> = ArrayVec::new();
                    new_node.push(value);
                    self.0.push_back(new_node)
                } else {
                    // Push to current node
                    last_node.push(value)
                }
            }
        }
    }

    fn len(&self) -> usize {
        match self.0.iter().last() {
            None => 0,
            Some(last_node) => last_node.len() + (self.0.len() - 1) * N,
        }
    }

    fn pop(&mut self) -> Option<T> {
        match self.0.iter_mut().last() {
            None => None,
            Some(last_node) => {
                // Safety: nodes are never empty, per invariant
                let x = unsafe { last_node.pop().unwrap_unchecked() };
                if last_node.is_empty() {
                    // Need to pop last node
                    self.0.pop_back();
                }
                Some(x)
            }
        }
    }

    fn top(&self) -> Option<&T> {
        match self.0.iter().last() {
            None => None,
            Some(last_node) => {
                // Safety: nodes are never empty, per invariant
                let x = unsafe { last_node.last().unwrap_unchecked() };
                Some(x)
            }
        }
    }

    fn top_mut(&mut self) -> Option<&mut T> {
        match self.0.iter_mut().last() {
            None => None,
            Some(last_node) => {
                // Safety: nodes are never empty, per invariant
                let x = unsafe { last_node.last_mut().unwrap_unchecked() };
                Some(x)
            }
        }
    }

    fn clear(&mut self) {
        self.0.clear()
    }

    fn iter<'a>(&'a self) -> impl ExactSizeIterator<Item = &'a T> + Clone
    where
        T: 'a,
    {
        let mut outer = self.0.iter();
        let inner = outer.next().map(|first| first.iter());
        ListVecIter {
            outer,
            inner,
            remaining: self.len(),
        }
    }

    // TODO: implement customized iter_skip_n
}

// Invariants:
// - inner is only none if outer is empty
// - If inner is Some, the iterator is never empty
pub struct ListVecIter<'a, T: Sized, const N: usize> {
    outer: alloc::collections::linked_list::Iter<'a, ArrayVec<T, N>>,
    inner: Option<core::slice::Iter<'a, T>>,
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

impl<'a, T: Sized, const N: usize> Iterator for ListVecIter<'a, T, N> {
    type Item = &'a T;

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
                    // Ensure inner is not left empty
                    if inner.len() == 0 {
                        self.inner = self.outer.next().map(|v| v.iter());
                    }
                    Some(val)
                }
            },
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl<'a, T: Sized, const N: usize> ExactSizeIterator for ListVecIter<'a, T, N> {}
