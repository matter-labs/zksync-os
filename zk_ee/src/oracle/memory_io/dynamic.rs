//! Destinations of dynamically sized responses.

use super::MemoryOracle;
use crate::internal_error;
use crate::system::errors::internal::InternalError;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::alloc::Allocator;
use core::mem::{size_of, MaybeUninit};

/// Number of oracle (`u32`) words in a `usize` word.
pub(crate) const U32_WORDS_PER_USIZE: usize = size_of::<usize>() / size_of::<u32>();

/// A destination for a dynamically sized response of `usize` words.
///
/// The oracle first claims the length of the response in `u32` words, the unit of the wire, so that a
/// response is the same sequence of words on every target (a native run records exactly the words the
/// proving target reads). The claim must be a whole number of `usize` words, and fit into the destination,
/// which may be longer than the response; the oracle then writes that many words to the beginning of the
/// destination.
pub trait DynamicDestination {
    /// Receives the response, and returns the number of `usize` words written.
    fn receive<O: MemoryOracle>(self, oracle: &mut O) -> Result<usize, InternalError>;
}

/// Reads the claimed length of a dynamically sized response, and returns it in `usize` words after checking
/// that it fits into `capacity` of them.
#[inline(always)]
pub(crate) fn read_claimed_len<O: MemoryOracle>(
    oracle: &mut O,
    capacity: usize,
) -> Result<usize, InternalError> {
    let claimed_u32_words = oracle.read_word()? as usize;
    // always true on the proving target, where a `usize` is one `u32` word
    if !claimed_u32_words.is_multiple_of(U32_WORDS_PER_USIZE) {
        return Err(internal_error!(
            "oracle response is not a whole number of usize words"
        ));
    }
    let len = claimed_u32_words / U32_WORDS_PER_USIZE;
    if len > capacity {
        return Err(internal_error!(
            "oracle response is longer than the destination"
        ));
    }
    Ok(len)
}

/// Writes the next `len` `usize` words of the response to `dst`.
///
/// # Safety
///
/// `dst` must be aligned and valid for writes of `len` `usize`s.
#[inline(always)]
pub(crate) unsafe fn write_usize_words<O: MemoryOracle>(
    oracle: &mut O,
    dst: *mut usize,
    len: usize,
) -> Result<(), InternalError> {
    // SAFETY: `usize` is aligned for `u32`, and `len` `usize`s are `len * U32_WORDS_PER_USIZE` `u32`s
    unsafe { oracle.write_words(dst.cast::<u32>(), len * U32_WORDS_PER_USIZE) }
}

impl DynamicDestination for &mut [MaybeUninit<usize>] {
    #[inline(always)]
    fn receive<O: MemoryOracle>(self, oracle: &mut O) -> Result<usize, InternalError> {
        let len = read_claimed_len(oracle, self.len())?;
        // SAFETY: `len` does not exceed the length of the slice
        unsafe { write_usize_words(oracle, self.as_mut_ptr().cast::<usize>(), len)? };
        Ok(len)
    }
}

impl DynamicDestination for &mut [usize] {
    #[inline(always)]
    fn receive<O: MemoryOracle>(self, oracle: &mut O) -> Result<usize, InternalError> {
        let len = read_claimed_len(oracle, self.len())?;
        // SAFETY: `len` does not exceed the length of the slice, and the oracle writes whole words, so
        // the slice stays initialized even if the transfer fails midway
        unsafe { write_usize_words(oracle, self.as_mut_ptr(), len)? };
        Ok(len)
    }
}

/// Appends the response within the spare capacity of the vector. The vector is never reallocated: the
/// querier bounds the response by the capacity it reserves beforehand.
impl<A: Allocator> DynamicDestination for &mut Vec<usize, A> {
    #[inline(always)]
    fn receive<O: MemoryOracle>(self, oracle: &mut O) -> Result<usize, InternalError> {
        let len = self.spare_capacity_mut().receive(oracle)?;
        // SAFETY: the first `len` words of the spare capacity were just written
        unsafe { self.set_len(self.len() + len) };
        Ok(len)
    }
}

impl<A: Allocator> DynamicDestination for &mut Box<[usize], A> {
    #[inline(always)]
    fn receive<O: MemoryOracle>(self, oracle: &mut O) -> Result<usize, InternalError> {
        (&mut **self).receive(oracle)
    }
}

impl<A: Allocator> DynamicDestination for &mut Box<[MaybeUninit<usize>], A> {
    #[inline(always)]
    fn receive<O: MemoryOracle>(self, oracle: &mut O) -> Result<usize, InternalError> {
        (&mut **self).receive(oracle)
    }
}
