//! This module provides specialized iterator chaining implementations that maintain
//! exact size information throughout the iteration process. This is helpful e.g.
//! for the oracle serialization system.
//!
//! Note that the standard library's `Chain` iterator loses exact size information when chaining
//! iterators.
//!
//! # Usage Patterns
//!
//! ## Simple Chaining (`ExactSizeChain`)
//! ```rust,ignore
//! // Chain two iterators for tuple serialization
//! let iter1 = first_field.iter();
//! let iter2 = second_field.iter();
//! let combined = ExactSizeChain::new(iter1, iter2);
//! assert_eq!(combined.len(), iter1.len() + iter2.len());
//! ```
//!
//! ## Array Chaining (`ExactSizeChainN`)
//! ```rust,ignore
//! // Chain multiple iterators for array serialization
//! let array_iters = array.iter().map(|elem| elem.iter()).collect();
//! let combined = ExactSizeChainN::new(empty_iter, array_iters);
//! ```

/// Chains two `ExactSizeIterator`s while preserving exact size semantics.
///
/// This is an optimized version of iterator chaining that maintains exact size information
/// throughout the iteration process. Unlike standard `Chain`, this implementation uses
/// optional fusing to avoid redundant checks and improve performance.
#[derive(Clone, Debug)]
pub struct ExactSizeChain<A, B> {
    // These are "fused" with `Option` so we don't need separate state to track which part is
    // already exhausted, and we may also get niche layout for `None`. We don't use the real `Fuse`
    // adapter because its specialization for `FusedIterator` unconditionally descends into the
    // iterator, and that could be expensive to keep revisiting stuff like nested chains. It also
    // hurts compiler performance to add more iterator layers to `Chain`.
    //
    // Only the "first" iterator is actually set `None` when exhausted, depending on whether you
    // iterate forward or backward. If you mix directions, then both sides may be `None`.
    a: Option<A>,
    b: Option<B>,
}
impl<A, B> ExactSizeChain<A, B> {
    pub fn new(a: A, b: B) -> ExactSizeChain<A, B> {
        ExactSizeChain {
            a: Some(a),
            b: Some(b),
        }
    }
}

impl<A, B> Iterator for ExactSizeChain<A, B>
where
    A: ExactSizeIterator,
    B: ExactSizeIterator<Item = A::Item>,
{
    type Item = A::Item;

    #[inline]
    fn next(&mut self) -> Option<A::Item> {
        and_then_or_clear(&mut self.a, Iterator::next).or_else(|| self.b.as_mut()?.next())
    }
}

impl<A, B> ExactSizeIterator for ExactSizeChain<A, B>
where
    A: ExactSizeIterator,
    B: ExactSizeIterator<Item = A::Item>,
{
    fn len(&self) -> usize {
        self.a.as_ref().map(|el| el.len()).unwrap_or(0)
            + self.b.as_ref().map(|el| el.len()).unwrap_or(0)
    }
}

#[inline]
fn and_then_or_clear<T, U>(opt: &mut Option<T>, f: impl FnOnce(&mut T) -> Option<U>) -> Option<U> {
    let x = f(opt.as_mut()?);
    if x.is_none() {
        *opt = None;
    }
    x
}

/// Chains one `ExactSizeIterator` iterator with an array of N `ExactSizeIterator` iterators while preserving exact size semantics.
///
/// This is a generalized version of `ExactSizeChain` that can chain a single iterator
/// with multiple iterators in sequence. It's particularly useful for serializing
/// complex nested data structures like arrays where each element may produce
/// multiple iterator values.
#[derive(Clone, Debug)]
pub struct ExactSizeChainN<A, B, const N: usize> {
    // These are "fused" with `Option` so we don't need separate state to track which part is
    // already exhausted, and we may also get niche layout for `None`. We don't use the real `Fuse`
    // adapter because its specialization for `FusedIterator` unconditionally descends into the
    // iterator, and that could be expensive to keep revisiting stuff like nested chains. It also
    // hurts compiler performance to add more iterator layers to `Chain`.
    //
    // Only the "first" iterator is actually set `None` when exhausted, depending on whether you
    // iterate forward or backward. If you mix directions, then both sides may be `None`.
    a: Option<A>,
    b: [Option<B>; N],
    b_idx: usize,
}

impl<A, B, const N: usize> ExactSizeChainN<A, B, N> {
    pub fn new(a: A, b: [Option<B>; N]) -> Self {
        assert!(N > 0);
        Self {
            a: Some(a),
            b,
            b_idx: 0,
        }
    }
}

impl<A, B, const N: usize> Iterator for ExactSizeChainN<A, B, N>
where
    A: ExactSizeIterator,
    B: ExactSizeIterator<Item = A::Item>,
{
    type Item = A::Item;

    #[inline]
    fn next(&mut self) -> Option<A::Item> {
        if N == 0 {
            and_then_or_clear(&mut self.a, Iterator::next)
        } else {
            and_then_or_clear(&mut self.a, Iterator::next).or_else(|| {
                while self.b_idx < N {
                    if let Some(next) = self.b[self.b_idx].as_mut().unwrap().next() {
                        return Some(next);
                    } else {
                        self.b[self.b_idx] = None;
                        self.b_idx += 1
                    }
                }

                None
            })
        }
    }
}

impl<A, B, const N: usize> ExactSizeIterator for ExactSizeChainN<A, B, N>
where
    A: ExactSizeIterator,
    B: ExactSizeIterator<Item = A::Item>,
{
    fn len(&self) -> usize {
        let mut result = self.a.as_ref().map(|el| el.len()).unwrap_or(0);
        for el in self.b.iter().skip(self.b_idx) {
            result += el.as_ref().map(|el| el.len()).unwrap_or(0)
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_size_chain_basic() {
        let iter1 = [1, 2, 3].into_iter();
        let iter2 = [4, 5].into_iter();

        let mut chain = ExactSizeChain::new(iter1, iter2);

        // Test exact size
        assert_eq!(chain.len(), 5);

        // Test iteration
        assert_eq!(chain.next(), Some(1));
        assert_eq!(chain.len(), 4);
        assert_eq!(chain.next(), Some(2));
        assert_eq!(chain.next(), Some(3));
        assert_eq!(chain.len(), 2);
        assert_eq!(chain.next(), Some(4));
        assert_eq!(chain.next(), Some(5));
        assert_eq!(chain.len(), 0);
        assert_eq!(chain.next(), None);
    }

    #[test]
    fn test_exact_size_chain_empty_first() {
        let iter1 = [].into_iter();
        let iter2 = [1, 2, 3].into_iter();

        let mut chain = ExactSizeChain::new(iter1, iter2);

        assert_eq!(chain.len(), 3);
        assert_eq!(chain.next(), Some(1));
        assert_eq!(chain.next(), Some(2));
        assert_eq!(chain.next(), Some(3));
        assert_eq!(chain.next(), None);
    }

    #[test]
    fn test_exact_size_chain_empty_second() {
        let iter1 = [1, 2, 3].into_iter();
        let iter2 = [].into_iter();

        let mut chain = ExactSizeChain::new(iter1, iter2);

        assert_eq!(chain.len(), 3);
        assert_eq!(chain.next(), Some(1));
        assert_eq!(chain.next(), Some(2));
        assert_eq!(chain.next(), Some(3));
        assert_eq!(chain.next(), None);
    }

    #[test]
    fn test_exact_size_chain_both_empty() {
        let iter1: core::array::IntoIter<usize, 0> = [].into_iter();
        let iter2 = [].into_iter();

        let mut chain = ExactSizeChain::new(iter1, iter2);

        assert_eq!(chain.len(), 0);
        assert_eq!(chain.next(), None);
    }

    #[test]
    fn test_exact_size_chain_n_basic() {
        let iter_a = [1].into_iter();
        let iter_b1 = [2, 3, 4].into_iter();
        let iter_b2 = [5, 6, 7].into_iter();
        let iter_b3 = [8, 9, 10].into_iter();

        let chain = ExactSizeChainN::new(iter_a, [Some(iter_b1), Some(iter_b2), Some(iter_b3)]);

        // Test exact size
        assert_eq!(chain.len(), 10);

        // Test iteration
        let collected: Vec<_> = chain.collect();
        assert_eq!(collected, vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    }
}
