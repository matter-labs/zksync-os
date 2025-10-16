use core::alloc::Allocator;

/// A factory trait for creating stack data structures with compile-time configuration.
///
/// This trait allows the system to abstract over different stack implementations
/// (Vec, skip lists, etc.) while providing compile-time parameters for optimization.
/// The const parameter N configures implementation-specific behavior like node sizes
/// or capacity constraints.
///
/// # Why This Exists
/// Different execution environments need different stack implementations:
/// - Forward execution: Uses Vec for dynamic allocation
/// - Proof generation: Uses skip lists for deterministic memory access patterns
pub trait StackFactory<const N: usize> {
    type Stack<T: Sized, const M: usize, A: Allocator + Clone>: Stack<T, A>;

    fn new_in<T, A: Allocator + Clone>(alloc: A) -> Self::Stack<T, N, A>;
}

/// This trait defines the common interface for stack data structures.
/// Implementations can range from simple Vec-based stacks to
/// more complex structures like skip lists for proof environments.
pub trait Stack<T: Sized, A: Allocator> {
    /// Creates a new empty stack using the provided allocator.
    fn new_in(alloc: A) -> Self;

    /// Returns the number of elements in the stack.
    fn len(&self) -> usize;

    /// Returns true if the stack contains no elements.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Pushes an element onto the top of the stack.
    fn push(&mut self, value: T);

    /// Removes and returns the top element from the stack, or None if empty.
    fn pop(&mut self) -> Option<T>;

    /// Returns a reference to the top element without removing it.
    fn top(&self) -> Option<&T>;

    /// Returns a mutable reference to the top element without removing it.
    fn top_mut(&mut self) -> Option<&mut T>;

    /// Removes all elements from the stack.
    fn clear(&mut self);

    /// Shortens the stack, keeping only the first `new_len` elements.
    ///
    /// This provides efficient rollback functionality by removing elements
    /// from the top of the stack until the desired length is reached.
    fn truncate(&mut self, new_len: usize) {
        if new_len < self.len() {
            let num_iterations = self.len() - new_len;
            for _ in 0..num_iterations {
                let _ = unsafe { self.pop().unwrap_unchecked() };
            }
        }
    }

    /// Returns an iterator over the stack elements from bottom to top.
    fn iter<'a>(&'a self) -> impl ExactSizeIterator<Item = &'a T> + Clone
    where
        T: 'a;

    /// Returns an iterator that skips the first n elements.
    ///
    /// Useful for iterating over a subset of the stack without creating
    /// intermediate collections.
    fn iter_skip_n<'a>(&'a self, n: usize) -> impl ExactSizeIterator<Item = &'a T> + Clone
    where
        T: 'a,
    {
        self.iter().skip(n)
    }
}
