//! A bitmask over the dense indices of a block access list (the address
//! indices, the slot positions), as the pre-execution check and the
//! transaction ranges exchange it: `u32` words with bit `i` at word `i / 32`,
//! bit `i % 32`, the last word zero padded. The words are little-endian, so
//! the mask has the same bytes on every target, and the bit of index `i` is
//! bit `i % 8` of byte `i / 8`.

use super::GlamsterdamError;
use alloc::boxed::Box;
use core::alloc::Allocator;

// The masks and the other tables that get hashed are `u32` words and byte
// streams: the words must have the same byte image on every target.
const _: () = assert!(cfg!(target_endian = "little"));

pub struct IndexMask<A: Allocator> {
    /// The number of indices
    len: u32,
    words: Box<[u32], A>,
}

impl<A: Allocator> IndexMask<A> {
    /// The number of words of a mask of `len` bits
    pub const fn words_for(len: usize) -> usize {
        len.div_ceil(32)
    }

    /// A mask of `len` clear bits
    pub fn zeroed(len: usize, allocator: A) -> Result<Self, GlamsterdamError> {
        if len >= u32::MAX as usize {
            return Err(GlamsterdamError::Capacity("index mask"));
        }
        let zeroed = Box::new_zeroed_slice_in(Self::words_for(len), allocator);
        // SAFETY: zero is a valid `u32`
        let words = unsafe { zeroed.assume_init() };
        Ok(Self {
            len: len as u32,
            words,
        })
    }

    /// A mask with the bits of the `true` flags set
    pub fn from_flags(
        flags: impl ExactSizeIterator<Item = bool>,
        allocator: A,
    ) -> Result<Self, GlamsterdamError> {
        let mut mask = Self::zeroed(flags.len(), allocator)?;
        for (index, flag) in flags.enumerate() {
            if flag {
                mask.set(index as u32);
            }
        }
        Ok(mask)
    }

    /// A mask of `len` bits from its bytes (see the module documentation):
    /// exactly the words of the mask, with the padding bits clear, so that
    /// the bytes of the mask are the bytes it was read from
    pub fn from_bytes(len: usize, bytes: &[u8], allocator: A) -> Result<Self, GlamsterdamError> {
        let mut mask = Self::zeroed(len, allocator)?;
        if bytes.len() != mask.words.len() * 4 {
            return Err(GlamsterdamError::InitialValuesEncoding);
        }
        for (word, chunk) in mask.words.iter_mut().zip(bytes.as_chunks::<4>().0) {
            *word = u32::from_le_bytes(*chunk);
        }
        if mask.padding() != 0 {
            return Err(GlamsterdamError::InitialValuesEncoding);
        }
        Ok(mask)
    }

    /// The number of indices
    pub fn len(&self) -> u32 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Whether the bit of `index` is set (clear beyond the length)
    pub fn get(&self, index: u32) -> bool {
        index < self.len && (self.words[(index / 32) as usize] >> (index % 32)) & 1 != 0
    }

    /// Sets the bit of `index`, which must be below the length
    pub fn set(&mut self, index: u32) {
        debug_assert!(index < self.len);
        self.words[(index / 32) as usize] |= 1 << (index % 32);
    }

    pub fn words(&self) -> &[u32] {
        &self.words
    }

    /// The words as bytes, little-endian: what gets hashed and witnessed
    pub fn bytes(&self) -> &[u8] {
        // SAFETY: `u32` has no padding, so every byte is initialized, and the
        // slice covers the same allocation
        unsafe {
            core::slice::from_raw_parts(
                self.words.as_ptr().cast::<u8>(),
                core::mem::size_of_val::<[u32]>(&self.words),
            )
        }
    }

    /// The bits of the last word beyond the length
    fn padding(&self) -> u32 {
        match self.words.last() {
            Some(last) if !self.len.is_multiple_of(32) => last & !((1u32 << (self.len % 32)) - 1),
            _ => 0,
        }
    }

    /// ORs `other` into this mask; the lengths must agree
    pub fn or_assign(&mut self, other: &Self) -> Result<(), GlamsterdamError> {
        if other.len != self.len {
            return Err(GlamsterdamError::Aggregation("index mask lengths differ"));
        }
        for (word, other) in self.words.iter_mut().zip(other.words.iter()) {
            *word |= *other;
        }
        Ok(())
    }

    /// Whether every bit below the length is set
    pub fn is_full(&self) -> bool {
        self.count_ones() == self.len
    }

    pub fn count_ones(&self) -> u32 {
        self.words.iter().map(|word| word.count_ones()).sum()
    }

    /// The indices whose bit is clear
    pub fn clear_indices(&self) -> impl Iterator<Item = u32> + '_ {
        (0..self.len).filter(move |index| !self.get(*index))
    }
}

impl<A: Allocator + Clone> Clone for IndexMask<A> {
    fn clone(&self) -> Self {
        Self {
            len: self.len,
            words: self.words.clone(),
        }
    }
}

impl<A: Allocator> core::fmt::Debug for IndexMask<A> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("IndexMask")
            .field("len", &self.len)
            .field("set", &self.count_ones())
            .finish()
    }
}

impl<A: Allocator> PartialEq for IndexMask<A> {
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len && self.words == other.words
    }
}

impl<A: Allocator> Eq for IndexMask<A> {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::alloc::Global;

    #[test]
    fn bits_bytes_and_fullness() {
        let mut mask = IndexMask::zeroed(35, Global).unwrap();
        assert!(mask.is_empty() == false && !mask.is_full());
        for index in [0, 31, 32, 34] {
            mask.set(index);
        }
        assert!(mask.get(0) && mask.get(31) && mask.get(32) && mask.get(34));
        assert!(!mask.get(1) && !mask.get(33) && !mask.get(35) && !mask.get(1000));
        assert_eq!(mask.words(), &[0x8000_0001, 0b101]);
        assert_eq!(mask.bytes(), &[1, 0, 0, 0x80, 5, 0, 0, 0]);
        // bit i is bit i % 8 of byte i / 8
        assert_eq!(mask.clear_indices().count(), 31);

        let read = IndexMask::from_bytes(35, mask.bytes(), Global).unwrap();
        assert_eq!(read, mask);
        assert!(IndexMask::from_bytes(34, mask.bytes(), Global).is_err());
        assert!(IndexMask::from_bytes(35, &mask.bytes()[..4], Global).is_err());

        let flags = (0..35).map(|_| true);
        let full = IndexMask::from_flags(flags, Global).unwrap();
        assert!(full.is_full());
        assert_eq!(full.padding(), 0);
        let mut other = IndexMask::zeroed(35, Global).unwrap();
        other.or_assign(&mask).unwrap();
        other.or_assign(&full).unwrap();
        assert!(other.is_full());
        assert!(other
            .or_assign(&IndexMask::zeroed(3, Global).unwrap())
            .is_err());
        assert!(IndexMask::<Global>::zeroed(0, Global).unwrap().is_full());
        assert!(IndexMask::from_flags([true; 32].into_iter(), Global)
            .unwrap()
            .is_full());
    }
}
