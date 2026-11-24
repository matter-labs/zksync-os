//! Reference implementation of [`MemoryOracle`] for the proving target, where the oracle is a word
//! channel.

use super::MemoryOracle;
use crate::internal_error;
use crate::system::errors::internal::InternalError;
use core::mem::MaybeUninit;
#[cfg(not(target_arch = "riscv32"))]
use core::sync::atomic::{compiler_fence, Ordering};

/// Word-level channel to the oracle: on the proving target, the non-determinism CSR.
pub trait WordChannel {
    fn write_word(&mut self, word: usize);
    fn read_word(&mut self) -> usize;
}

/// [`MemoryOracle`] over a [`WordChannel`]: a query is two channel writes, and every response word is one
/// channel read, stored by the querier itself into the destination.
#[derive(Clone, Copy, Debug, Default)]
pub struct WordChannelOracle<C: WordChannel> {
    channel: C,
}

impl<C: WordChannel> WordChannelOracle<C> {
    pub const fn new(channel: C) -> Self {
        Self { channel }
    }

    pub fn channel(&self) -> &C {
        &self.channel
    }
}

impl<C: WordChannel> MemoryOracle for WordChannelOracle<C> {
    #[inline(always)]
    fn send_query(&mut self, query_id: u32, input_word: usize) -> Result<(), InternalError> {
        send_query_words(&mut self.channel, query_id, input_word);
        Ok(())
    }

    #[inline(always)]
    fn read_word(&mut self) -> Result<u32, InternalError> {
        channel_word_to_u32(self.channel.read_word())
    }

    #[inline(always)]
    unsafe fn write_words(&mut self, dst: *mut u32, num_words: usize) -> Result<(), InternalError> {
        // SAFETY: guaranteed by the caller
        let dst =
            unsafe { core::slice::from_raw_parts_mut(dst.cast::<MaybeUninit<u32>>(), num_words) };
        // unrolled: for long responses the loop overhead is comparable to the channel read itself
        let (chunks, remainder) = dst.as_chunks_mut::<4>();
        for chunk in chunks.iter_mut() {
            chunk[0].write(channel_word_to_u32(self.channel.read_word())?);
            chunk[1].write(channel_word_to_u32(self.channel.read_word())?);
            chunk[2].write(channel_word_to_u32(self.channel.read_word())?);
            chunk[3].write(channel_word_to_u32(self.channel.read_word())?);
        }
        for word in remainder.iter_mut() {
            word.write(channel_word_to_u32(self.channel.read_word())?);
        }
        Ok(())
    }
}

/// A channel word is a `u32` on the proving target, where the conversion compiles to nothing
/// (`TryFrom<usize> for u32` is infallible on 32-bit targets); on wider targets it must fit.
#[inline(always)]
fn channel_word_to_u32(word: usize) -> Result<u32, InternalError> {
    u32::try_from(word).map_err(|_| internal_error!("oracle word does not fit into u32"))
}

/// Sends a query: the query ID, then the input word
#[inline(always)]
fn send_query_words<C: WordChannel>(channel: &mut C, query_id: u32, input_word: usize) {
    channel.write_word(query_id as usize);
    // Not needed on the proving target, which is single-threaded (and built with
    // `-C passes=lower-atomic`, which erases fences anyway).
    #[cfg(not(target_arch = "riscv32"))]
    compiler_fence(Ordering::SeqCst);
    // The input word may be the address of the input, which the oracle reads as soon as it receives
    // the word, while for the compiler the channel write (a `nomem` CSR asm) reads no memory: without
    // a barrier, the stores that fill the input, or the address array of a composite input, are dead
    // and get removed. `black_box(())` is an empty asm that may read any exposed memory, so these
    // stores happen before it, and the channel write, an asm with side effects too, stays after it.
    // Being zero-sized, the argument is not spilled: no instruction is emitted.
    core::hint::black_box(());
    channel.write_word(input_word);
}

/// [`MemoryOracle`] over a [`WordChannel`] whose words carry 64 bits, for a 64-bit guest: the response
/// is still a sequence of `u32` words, delivered two to a channel word wherever the querier takes two
/// at once.
///
/// - [`read_word`](MemoryOracle::read_word) takes one channel word and returns its low half.
/// - [`read_u64`](MemoryOracle::read_u64) takes one channel word, all of it: two response words, the
///   earlier one in the low half.
/// - [`write_words`](MemoryOracle::write_words) takes one channel word for every two response words,
///   the earlier one in the low half, and one more for the last of an odd number, in its low half.
///
/// So what a channel word carries follows from the calls of the querier: the oracle side builds the
/// words as the same computation reads its answers. The half of a channel word that a call does not
/// use is ignored: the prover could have supplied it cleared, so nothing is to be checked.
#[cfg(target_pointer_width = "64")]
#[derive(Clone, Copy, Debug, Default)]
pub struct PackedWordChannelOracle<C: WordChannel> {
    channel: C,
}

#[cfg(target_pointer_width = "64")]
impl<C: WordChannel> PackedWordChannelOracle<C> {
    pub const fn new(channel: C) -> Self {
        Self { channel }
    }

    pub fn channel(&self) -> &C {
        &self.channel
    }
}

#[cfg(target_pointer_width = "64")]
impl<C: WordChannel> MemoryOracle for PackedWordChannelOracle<C> {
    #[inline(always)]
    fn send_query(&mut self, query_id: u32, input_word: usize) -> Result<(), InternalError> {
        send_query_words(&mut self.channel, query_id, input_word);
        Ok(())
    }

    #[inline(always)]
    fn read_word(&mut self) -> Result<u32, InternalError> {
        Ok(self.channel.read_word() as u32)
    }

    #[inline(always)]
    fn read_u64(&mut self) -> Result<u64, InternalError> {
        Ok(self.channel.read_word() as u64)
    }

    #[inline(always)]
    unsafe fn write_words(&mut self, dst: *mut u32, num_words: usize) -> Result<(), InternalError> {
        let pairs = num_words / 2;
        if cfg!(target_endian = "little") && dst.addr().is_multiple_of(align_of::<u64>()) {
            // a pair is the two halves of a 64-bit value in memory, the earlier word first
            // SAFETY: the pairs are within the `num_words` words that the caller guarantees to be
            // valid for writes, and `dst` is aligned for `u64`
            let dst =
                unsafe { core::slice::from_raw_parts_mut(dst.cast::<MaybeUninit<u64>>(), pairs) };
            // unrolled, as in `WordChannelOracle`
            let (chunks, remainder) = dst.as_chunks_mut::<4>();
            for chunk in chunks.iter_mut() {
                chunk[0].write(self.channel.read_word() as u64);
                chunk[1].write(self.channel.read_word() as u64);
                chunk[2].write(self.channel.read_word() as u64);
                chunk[3].write(self.channel.read_word() as u64);
            }
            for pair in remainder.iter_mut() {
                pair.write(self.channel.read_word() as u64);
            }
        } else {
            for i in 0..pairs {
                let pair = self.channel.read_word() as u64;
                // SAFETY: both words of the pair `i` are within the `num_words` words that the caller
                // guarantees to be valid for writes, aligned for `u32`
                unsafe {
                    dst.add(2 * i).write(pair as u32);
                    dst.add(2 * i + 1).write((pair >> 32) as u32);
                }
            }
        }
        if num_words % 2 == 1 {
            // SAFETY: the last word is within the words valid for writes
            unsafe {
                dst.add(num_words - 1)
                    .write(self.channel.read_word() as u32)
            };
        }
        Ok(())
    }
}
