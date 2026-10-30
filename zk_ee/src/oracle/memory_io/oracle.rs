//! The oracle as seen by the querier.

use super::composite::{CompositeDeserializable, CompositeSerializable};
use super::continuous::{
    continuous_words, exposed_address, ContinuousDeserializable, ContinuousSerializable,
};
use super::dynamic::{read_claimed_len, write_usize_words, DynamicDestination};
use super::short::{ShortDeserializable, ShortSerializable};
use super::MemoryOracle;
use crate::internal_error;
use crate::system::errors::internal::InternalError;
use alloc::boxed::Box;
use core::alloc::Allocator;
use core::mem::MaybeUninit;

/// The typed encodings of the memory-based protocol (see the [module documentation](super)), on the
/// querier side: values passed by value or by address, received memcpy-like and validated in place, on
/// top of the transport primitives of [`MemoryOracle`] (`airbender_oracle`'s), with ZKsync OS's errors.
/// Every such oracle has them.
///
/// The `send_*` methods are `unsafe`, as `send_query` is: each sends its input the way its type is passed,
/// and the caller guarantees that the query ID takes it that way. [`OracleQuery`](super::OracleQuery) and
/// [`DynamicOracleQuery`](super::DynamicOracleQuery), `unsafe` traits, bind each query ID of ZKsync OS to its
/// input type once, and their methods are safe.
pub trait MemoryOracleExt: MemoryOracle<Error = InternalError> {
    /// Starts a query with a short input, passed by value.
    ///
    /// # Safety
    ///
    /// `query_id` must take a short input of this type, the value itself.
    #[inline(always)]
    unsafe fn send_short<T: ShortSerializable>(
        &mut self,
        query_id: u32,
        input: T,
    ) -> Result<(), InternalError> {
        // SAFETY: the input word is the value itself, which `query_id` takes (the caller's contract)
        unsafe { self.send_query(query_id, input.to_short_word() as usize) }
    }

    /// Starts a query with an input continuous in memory, passed by address.
    ///
    /// # Safety
    ///
    /// `query_id` must take a `T`, passed by address: an oracle in this process reads one through it.
    #[inline(always)]
    unsafe fn send_continuous<T: ContinuousSerializable>(
        &mut self,
        query_id: u32,
        input: &T,
    ) -> Result<(), InternalError> {
        // SAFETY: the input word is the address of `input`, live for the call, and `query_id` takes a
        // `T` (the caller's contract)
        unsafe { self.send_query(query_id, exposed_address(input)) }
    }

    /// Starts a query with a composite input, passed by the address of the array of element addresses.
    ///
    /// # Safety
    ///
    /// `query_id` must take these elements, passed by the address of the array of their addresses.
    #[inline(always)]
    unsafe fn send_composite<C: CompositeSerializable>(
        &mut self,
        query_id: u32,
        input: C,
    ) -> Result<(), InternalError> {
        // SAFETY: the input word is the address of the array of the element addresses, both live
        // until `with_address` returns, and `query_id` takes these elements (the caller's contract)
        input.with_address(|address| unsafe { self.send_query(query_id, address) })
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

impl<O: MemoryOracle<Error = InternalError>> MemoryOracleExt for O {}
