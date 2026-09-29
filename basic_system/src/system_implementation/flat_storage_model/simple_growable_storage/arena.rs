use super::*;

/// `vec.push(value)` for a vector that has room for the value: without the growth of `Vec::push`,
/// which the proving allocator does not support, and with the same check on every target.
#[inline(always)]
pub(super) fn push_within_capacity<T, A: Allocator>(vec: &mut Vec<T, A>, value: T) {
    assert!(vec.len() < vec.capacity(), "vector is full");
    vec.spare_capacity_mut()[0].write(value);
    // SAFETY: the element at the length was just initialized
    unsafe { vec.set_len(vec.len() + 1) };
}

/// A vector that never reallocates: a directory of chunks sized for `max_len` elements up front,
/// and chunks of `C` elements, each allocated when the previous one is full.
pub(super) struct ChunkedVec<T, const C: usize, A: Allocator + Clone> {
    pub(super) chunks: Vec<Vec<T, A>, A>,
    pub(super) len: usize,
    pub(super) allocator: A,
}

impl<T, const C: usize, A: Allocator + Clone> ChunkedVec<T, C, A> {
    pub(super) fn with_max_len_in(max_len: usize, allocator: A) -> Self {
        Self {
            chunks: Vec::with_capacity_in(max_len.div_ceil(C), allocator.clone()),
            len: 0,
            allocator,
        }
    }

    #[inline(always)]
    pub(super) fn len(&self) -> usize {
        self.len
    }

    #[inline(always)]
    pub(super) fn get(&self, index: usize) -> &T {
        &self.chunks[index / C][index % C]
    }

    #[inline(always)]
    pub(super) fn get_mut(&mut self, index: usize) -> &mut T {
        &mut self.chunks[index / C][index % C]
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = &T> {
        self.chunks.iter().flatten()
    }

    /// The slot of the next element, in a new chunk if the last one is full.
    pub(super) fn next_slot(&mut self) -> &mut MaybeUninit<T> {
        if self.len / C == self.chunks.len() {
            let chunk = Vec::with_capacity_in(C, self.allocator.clone());
            push_within_capacity(&mut self.chunks, chunk);
        }
        &mut self.chunks[self.len / C].spare_capacity_mut()[0]
    }

    /// Appends the element written into the slot returned by `next_slot`.
    ///
    /// # Safety
    ///
    /// The slot returned by the last `next_slot` must be initialized.
    pub(super) unsafe fn commit_next(&mut self) {
        let chunk = &mut self.chunks[self.len / C];
        // SAFETY: the slot at the length of the chunk is within its capacity, and initialized
        unsafe { chunk.set_len(chunk.len() + 1) };
        self.len += 1;
    }

    pub(super) fn push(&mut self, value: T) {
        self.next_slot().write(value);
        // SAFETY: just initialized
        unsafe { self.commit_next() };
    }
}

/// Elements per chunk: 32 paths are 64 KiB for a tree of height 64.
pub(super) const LEAF_CHUNK_LEN: usize = 32;
/// Keep batch items in bounded chunks without growing a vector in the proving allocator.
pub(super) const ITEM_CHUNK_LEN: usize = 256;
/// Keep key slots in bounded chunks without growing a vector in the proving allocator.
pub(super) const KEY_CHUNK_LEN: usize = 256;
