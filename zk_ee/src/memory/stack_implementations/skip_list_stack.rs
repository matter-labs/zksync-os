use arrayvec::ArrayVec;
use core::alloc::Allocator;

use crate::{
    common_structs::skip_list_quasi_vec::{ListVec, ListVecIter},
    memory::stack_trait::Stack,
};

/// Stack implementation using a skip list structure.
///
/// This implementation stores elements in fixed-size nodes (ArrayVec<T, N>) organized
/// as a list of nodes.
impl<T: Sized, const N: usize, A: Allocator + Clone> Stack<T, A> for ListVec<T, N, A> {
    fn new_in(alloc: A) -> Self {
        ListVec::<T, N, A>::new_in(alloc)
    }

    /// Pushes an element onto the stack.
    ///
    /// Elements are added to the last node if it has space, otherwise a new node
    /// is created.
    fn push(&mut self, value: T) {
        match self.0.iter_mut().last() {
            None => {
                // Stack is empty - create the first node
                let mut new_node: ArrayVec<T, N> = ArrayVec::new();
                new_node.push(value);
                self.0.push_back(new_node)
            }
            Some(last_node) => {
                if last_node.is_full() {
                    // Last node is at capacity N - allocate a new node
                    let mut new_node: ArrayVec<T, N> = ArrayVec::new();
                    new_node.push(value);
                    self.0.push_back(new_node)
                } else {
                    // Last node has space - add to existing node
                    last_node.push(value)
                }
            }
        }
    }

    fn len(&self) -> usize {
        match self.0.iter().last() {
            None => 0,
            Some(last_node) => {
                // All nodes except the last are full (N elements each)
                // Plus the actual length of the last (potentially partial) node
                last_node.len() + (self.0.len() - 1) * N
            }
        }
    }

    fn pop(&mut self) -> Option<T> {
        match self.0.iter_mut().last() {
            None => None, // Stack is empty
            Some(last_node) => {
                // Safety: By invariant, nodes in the list are never empty
                let x = unsafe { last_node.pop().unwrap_unchecked() };

                if last_node.is_empty() {
                    // Maintain invariant: remove empty nodes immediately
                    self.0.pop_back();
                }
                Some(x)
            }
        }
    }

    /// Returns a reference to the top element without removing it.
    fn top(&self) -> Option<&T> {
        match self.0.iter().last() {
            None => None, // Stack is empty
            Some(last_node) => {
                // Safety: By invariant, nodes in the list are never empty
                let x = unsafe { last_node.last().unwrap_unchecked() };
                Some(x)
            }
        }
    }

    /// Returns a mutable reference to the top element without removing it.
    fn top_mut(&mut self) -> Option<&mut T> {
        match self.0.iter_mut().last() {
            None => None, // Stack is empty
            Some(last_node) => {
                // Safety: By invariant, nodes in the list are never empty
                let x = unsafe { last_node.last_mut().unwrap_unchecked() };
                Some(x)
            }
        }
    }

    /// Removes all elements from the stack, deallocating all nodes.
    fn clear(&mut self) {
        self.0.clear()
    }

    /// Returns an iterator over all elements from bottom to top of the stack.
    fn iter<'a>(&'a self) -> impl ExactSizeIterator<Item = &'a T> + Clone
    where
        T: 'a,
    {
        let mut outer = self.0.iter();
        let inner = outer.next().map(|first| first.iter());
        ListVecIter::new_from_parts(outer, inner, self.len())
    }

    /// Returns an iterator that skips the first `n` elements.
    ///
    /// Jumps straight to the node holding element `n` (O(n / N) node hops) instead of
    /// stepping over `n` elements, and clones of the result do not re-skip.
    fn iter_skip_n<'a>(&'a self, n: usize) -> impl ExactSizeIterator<Item = &'a T> + Clone
    where
        T: 'a,
    {
        let len = self.len();
        let mut outer = self.0.iter();
        if n >= len {
            // Nothing left to yield (same as `iter().skip(n)` for `n >= len`).
            return ListVecIter::new_from_parts(outer, None, 0);
        }
        // All nodes but the last are full, so element `n` is at offset `n % N` of node `n / N`.
        // As `n < len`, that node exists and holds more than `n % N` elements.
        let inner = outer.nth(n / N).map(|node| node[n % N..].iter());
        ListVecIter::new_from_parts(outer, inner, len - n)
    }

    // TODO: optimized truncate?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common_structs::skip_list_quasi_vec::ListVec;
    use alloc::alloc::Global;
    use alloc::vec::Vec;

    type TestStack = ListVec<i32, 3, Global>; // Node capacity of 3 for easy testing

    #[test]
    fn test_within_single_node() {
        let mut stack = TestStack::new_in(Global);

        // Fill up one node (capacity 3)
        stack.push(1);
        stack.push(2);
        stack.push(3);

        assert_eq!(stack.len(), 3);
        assert_eq!(stack.top(), Some(&3));

        // Pop elements in LIFO order
        assert_eq!(stack.pop(), Some(3));
        assert_eq!(stack.len(), 2);
        assert_eq!(stack.top(), Some(&2));

        assert_eq!(stack.pop(), Some(2));
        assert_eq!(stack.len(), 1);
        assert_eq!(stack.top(), Some(&1));

        assert_eq!(stack.pop(), Some(1));
        assert_eq!(stack.len(), 0);
        assert_eq!(stack.top(), None);
    }

    #[test]
    fn test_multiple_nodes() {
        let mut stack = TestStack::new_in(Global);

        // Push elements across multiple nodes
        for i in 1..=10 {
            stack.push(i);
        }

        assert_eq!(stack.len(), 10);
        assert_eq!(stack.top(), Some(&10));

        // Should have created 4 nodes: [1,2,3], [4,5,6], [7,8,9], [10]
        assert_eq!(stack.0.len(), 4);

        // Pop all elements and verify LIFO order
        for expected in (1..=10).rev() {
            assert_eq!(stack.pop(), Some(expected));
        }

        assert_eq!(stack.len(), 0);
        assert!(stack.is_empty());
        // All nodes should be cleaned up
        assert_eq!(stack.0.len(), 0);
    }

    #[test]
    fn test_node_cleanup_invariant() {
        let mut stack = TestStack::new_in(Global);

        // Fill exactly one node
        stack.push(1);
        stack.push(2);
        stack.push(3);
        assert_eq!(stack.0.len(), 1);

        // Add one more to create second node
        stack.push(4);
        assert_eq!(stack.0.len(), 2);

        // Pop the last element - second node should be removed
        assert_eq!(stack.pop(), Some(4));
        assert_eq!(stack.0.len(), 1);

        // First node should still have 3 elements
        assert_eq!(stack.len(), 3);
        assert_eq!(stack.top(), Some(&3));
    }

    #[test]
    fn test_top_mut() {
        let mut stack = TestStack::new_in(Global);
        assert_eq!(stack.top_mut(), None);

        stack.push(10);
        assert_eq!(stack.top_mut(), Some(&mut 10));

        // Modify through mutable reference
        if let Some(top) = stack.top_mut() {
            *top = 99;
        }

        assert_eq!(stack.top(), Some(&99));
        assert_eq!(stack.pop(), Some(99));
    }

    #[test]
    fn test_clear() {
        let mut stack = TestStack::new_in(Global);

        // Add elements across multiple nodes
        for i in 1..=10 {
            stack.push(i);
        }

        assert_eq!(stack.len(), 10);
        assert_eq!(stack.0.len(), 4);

        stack.clear();

        assert_eq!(stack.len(), 0);
        assert!(stack.is_empty());
        assert_eq!(stack.0.len(), 0);
        assert_eq!(stack.top(), None);
    }

    #[test]
    fn test_truncate() {
        let mut stack = TestStack::new_in(Global);

        // Add 10 elements
        for i in 1..=10 {
            stack.push(i);
        }

        assert_eq!(stack.len(), 10);

        // Truncate to 7 elements
        stack.truncate(7);
        assert_eq!(stack.len(), 7);
        assert_eq!(stack.top(), Some(&7));

        // Should have 3 nodes now: [1,2,3], [4,5,6], [7]
        assert_eq!(stack.0.len(), 3);

        // Truncate to 3 elements (exactly one node)
        stack.truncate(3);
        assert_eq!(stack.len(), 3);
        assert_eq!(stack.top(), Some(&3));
        assert_eq!(stack.0.len(), 1);

        // Truncate to 0
        stack.truncate(0);
        assert_eq!(stack.len(), 0);
        assert!(stack.is_empty());
        assert_eq!(stack.0.len(), 0);
    }

    #[test]
    fn test_truncate_no_op() {
        let mut stack = TestStack::new_in(Global);

        for i in 1..=5 {
            stack.push(i);
        }

        let original_len = stack.len();

        // Truncate to same length should be no-op
        stack.truncate(5);
        assert_eq!(stack.len(), original_len);
        assert_eq!(stack.top(), Some(&5));

        // Truncate to larger length should be no-op
        stack.truncate(10);
        assert_eq!(stack.len(), original_len);
        assert_eq!(stack.top(), Some(&5));
    }

    #[test]
    fn test_iterator_empty() {
        let stack = TestStack::new_in(Global);
        let mut iter = stack.iter();

        assert_eq!(iter.len(), 0);
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_iterator_single_node() {
        let mut stack = TestStack::new_in(Global);
        stack.push(1);
        stack.push(2);
        stack.push(3);

        let collected: Vec<&i32> = stack.iter().collect();
        assert_eq!(collected, vec![&1, &2, &3]);

        // Test exact size iterator
        let iter = stack.iter();
        assert_eq!(iter.len(), 3);
    }

    #[test]
    fn test_iterator_multiple_nodes() {
        let mut stack = TestStack::new_in(Global);

        // Add elements across multiple nodes
        for i in 1..=7 {
            stack.push(i);
        }

        let collected: Vec<&i32> = stack.iter().collect();
        let expected_values: Vec<i32> = (1..=7).collect();
        let expected: Vec<&i32> = expected_values.iter().collect();
        assert_eq!(collected, expected);

        // Test exact size iterator
        let iter = stack.iter();
        assert_eq!(iter.len(), 7);
    }

    #[test]
    fn test_iterator_skip_n() {
        let mut stack = TestStack::new_in(Global);

        for i in 1..=10 {
            stack.push(i);
        }

        let collected: Vec<&i32> = stack.iter_skip_n(3).collect();
        let expected_values: Vec<i32> = (4..=10).collect();
        let expected: Vec<&i32> = expected_values.iter().collect();
        assert_eq!(collected, expected);

        // Test exact size after skip
        let iter = stack.iter_skip_n(3);
        assert_eq!(iter.len(), 7);
    }

    #[test]
    fn test_iterator_clone() {
        let mut stack = TestStack::new_in(Global);
        stack.push(1);
        stack.push(2);
        stack.push(3);

        let iter1 = stack.iter();
        let iter2 = iter1.clone();

        let collected1: Vec<&i32> = iter1.collect();
        let collected2: Vec<&i32> = iter2.collect();

        assert_eq!(collected1, collected2);
        assert_eq!(collected1, vec![&1, &2, &3]);
    }

    /// Checks `iter_skip_n(n)` against `iter().skip(n)` for every `n` in `0..=len + 3`:
    /// same elements, exact `len()` at every step, and clones taken mid-iteration
    /// yield the same remaining sequence as the original.
    fn check_iter_skip_n_matches_skip<const N: usize>(stack: &ListVec<usize, N, Global>) {
        let len = stack.len();
        let all: Vec<usize> = stack.iter().copied().collect();
        assert_eq!(all, (0..len).collect::<Vec<usize>>());

        for n in 0..=len + 3 {
            let expected: Vec<&usize> = stack.iter().skip(n).collect();
            let expected_len = len.saturating_sub(n);
            assert_eq!(expected.len(), expected_len);
            assert_eq!(stack.iter_skip_n(n).collect::<Vec<&usize>>(), expected);

            let mut iter = stack.iter_skip_n(n);
            for k in 0..=expected_len {
                assert_eq!(iter.len(), expected_len - k, "N={N} len={len} n={n} k={k}");
                let clone = iter.clone();
                assert_eq!(clone.len(), iter.len());
                assert_eq!(clone.collect::<Vec<&usize>>(), expected[k..]);
                assert_eq!(iter.next(), expected.get(k).copied());
            }
            assert_eq!(iter.len(), 0);
            assert_eq!(iter.next(), None);
        }
    }

    /// Runs the differential check on stacks of every length in `0..=100`, built by
    /// pushing only, and by overshooting then shrinking via `pop` and via `truncate`
    /// (so partial last nodes also arise from shrinking).
    fn check_iter_skip_n_all_lengths<const N: usize>() {
        for len in 0..=100 {
            let mut pushed = ListVec::<usize, N, Global>::new_in(Global);
            for i in 0..len {
                pushed.push(i);
            }
            check_iter_skip_n_matches_skip(&pushed);

            let mut popped = ListVec::<usize, N, Global>::new_in(Global);
            for i in 0..len + N + 1 {
                popped.push(i);
            }
            for _ in 0..N + 1 {
                popped.pop();
            }
            assert_eq!(popped.len(), len);
            check_iter_skip_n_matches_skip(&popped);

            let mut truncated = ListVec::<usize, N, Global>::new_in(Global);
            for i in 0..len + 2 * N + 3 {
                truncated.push(i);
            }
            truncated.truncate(len);
            assert_eq!(truncated.len(), len);
            check_iter_skip_n_matches_skip(&truncated);
        }
    }

    #[test]
    fn test_iter_skip_n_matches_skip_node_size_1() {
        check_iter_skip_n_all_lengths::<1>();
    }

    #[test]
    fn test_iter_skip_n_matches_skip_node_size_2() {
        check_iter_skip_n_all_lengths::<2>();
    }

    #[test]
    fn test_iter_skip_n_matches_skip_node_size_3() {
        check_iter_skip_n_all_lengths::<3>();
    }

    #[test]
    fn test_iter_skip_n_matches_skip_node_size_32() {
        check_iter_skip_n_all_lengths::<32>();
    }
}
