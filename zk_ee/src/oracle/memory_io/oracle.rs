//! The oracle as seen by the querier.

use super::composite::{CompositeDeserializable, CompositeSerializable};
use super::continuous::{
    continuous_words, exposed_address, ContinuousDeserializable, ContinuousSerializable,
};
use super::dynamic::{read_claimed_len, write_usize_words, DynamicDestination};
use super::short::{ShortDeserializable, ShortSerializable};
use crate::internal_error;
use crate::system::errors::internal::InternalError;
use alloc::boxed::Box;
use core::alloc::Allocator;
use core::mem::MaybeUninit;

/// Querier side of the memory-based oracle protocol (see the [module documentation](super)).
///
/// Implementations provide the transport primitives: [`send_query`](Self::send_query),
/// [`read_word`](Self::read_word), [`write_words`](Self::write_words) and
/// [`finish_query`](Self::finish_query). The typed methods are built on top of them, and are only meant
/// to be overridden with equivalent, faster versions.
///
/// The default primitives reject the protocol: they are for oracles that never serve it, like test
/// doubles of [`IOOracle`](crate::oracle::IOOracle), which requires this trait.
pub trait MemoryOracle: Sized {
    /// Starts a query: sends the query ID and the input word. The oracle reads whatever the input word
    /// refers to during this call; the memory may change as soon as it returns.
    fn send_query(&mut self, _query_id: u32, _input_word: usize) -> Result<(), InternalError> {
        Err(internal_error!(
            "oracle does not serve the memory-based protocol"
        ))
    }

    /// Reads one response word: a short value, or the claimed length of a dynamically sized response.
    fn read_word(&mut self) -> Result<u32, InternalError> {
        Err(internal_error!(
            "oracle does not serve the memory-based protocol"
        ))
    }

    /// Writes the next `num_words` response words to `dst`, in order. This is the memcpy-like primitive:
    /// "write the next `u32` to the next aligned address", repeated.
    ///
    /// # Safety
    ///
    /// `dst` must be aligned for `u32` and valid for writes of `num_words` `u32`s.
    unsafe fn write_words(
        &mut self,
        _dst: *mut u32,
        _num_words: usize,
    ) -> Result<(), InternalError> {
        Err(internal_error!(
            "oracle does not serve the memory-based protocol"
        ))
    }

    /// Ends the query. Implementations that can (in forward mode) should check here that the response
    /// was consumed in full.
    fn finish_query(&mut self) -> Result<(), InternalError> {
        Ok(())
    }

    /// Starts a query with a short input, passed by value.
    #[inline(always)]
    fn send_short<T: ShortSerializable>(
        &mut self,
        query_id: u32,
        input: T,
    ) -> Result<(), InternalError> {
        self.send_query(query_id, input.to_short_word() as usize)
    }

    /// Starts a query with an input continuous in memory, passed by address.
    #[inline(always)]
    fn send_continuous<T: ContinuousSerializable>(
        &mut self,
        query_id: u32,
        input: &T,
    ) -> Result<(), InternalError> {
        self.send_query(query_id, exposed_address(input))
    }

    /// Starts a query with a composite input, passed by the address of the array of element addresses.
    #[inline(always)]
    fn send_composite<C: CompositeSerializable>(
        &mut self,
        query_id: u32,
        input: C,
    ) -> Result<(), InternalError> {
        input.with_address(|address| self.send_query(query_id, address))
    }

    /// Reads a short value.
    #[inline(always)]
    fn read_short<T: ShortDeserializable>(&mut self) -> Result<T, InternalError> {
        T::from_short_word(self.read_word()?)
    }

    /// Writes the next `size_of::<T>()` bytes of the response to `dst`, memcpy-like, without validating
    /// them: [`ContinuousDeserializable::validate`] must succeed before the value is used.
    ///
    /// # Safety
    ///
    /// `dst` must be aligned and valid for writes of `T`.
    #[inline(always)]
    unsafe fn write<T: ContinuousDeserializable>(
        &mut self,
        dst: *mut T,
    ) -> Result<(), InternalError> {
        let num_words = const { continuous_words::<T>() };
        // SAFETY: `T` is aligned for `u32` and consists of `num_words` words, and `dst` is valid for
        // writes of `T`
        unsafe { self.write_words(dst.cast::<u32>(), num_words) }
    }

    /// Writes the next value of the response to `dst`, and validates it in place.
    #[inline(always)]
    fn init<'a, T: ContinuousDeserializable>(
        &mut self,
        dst: &'a mut MaybeUninit<T>,
    ) -> Result<&'a mut T, InternalError> {
        let this = dst.as_mut_ptr();
        // SAFETY: `dst` is aligned and valid for writes of `T`
        unsafe { self.write(this)? };
        // SAFETY: all bytes of `dst` were just written, and `dst` is exclusively borrowed for `'a`
        unsafe { T::validate(this) }
    }

    /// Writes and validates each element of a composite output, in turn.
    #[inline(always)]
    fn init_composite<'a, C: CompositeDeserializable<'a>>(
        &mut self,
        dst: C,
    ) -> Result<C::Initialized, InternalError> {
        dst.init_from(self)
    }

    /// Receives a dynamically sized response into `dst`, and returns the number of `usize` words written.
    #[inline(always)]
    fn write_dynamic<D: DynamicDestination>(&mut self, dst: D) -> Result<usize, InternalError> {
        dst.receive(self)
    }

    /// Receives a dynamically sized response into a new allocation of exactly the claimed length, which
    /// must not exceed `max_len` `usize` words.
    fn write_dynamic_boxed<A: Allocator>(
        &mut self,
        max_len: usize,
        allocator: A,
    ) -> Result<Box<[usize], A>, InternalError> {
        let len = read_claimed_len(self, max_len)?;
        let mut buffer = Box::try_new_uninit_slice_in(len, allocator)
            .map_err(|_| internal_error!("failed to allocate for oracle response"))?;
        // SAFETY: the buffer holds `len` words
        unsafe { write_usize_words(self, buffer.as_mut_ptr().cast::<usize>(), len)? };
        // SAFETY: all `len` words were just written
        Ok(unsafe { buffer.assume_init() })
    }
}
