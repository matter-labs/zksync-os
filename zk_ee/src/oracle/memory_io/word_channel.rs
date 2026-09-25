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
        self.channel.write_word(query_id as usize);
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
        self.channel.write_word(input_word);
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
