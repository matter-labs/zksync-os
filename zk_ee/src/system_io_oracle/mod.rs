pub mod dyn_usize_iterator;
use crate::utils::UsizeAlignedByteBox;
use crate::{system::errors::internal::InternalError, utils::Bytes32};
use core::alloc::Allocator;
///
/// Oracle is an abstraction boundary on how OS (System trait) gets IO information and eventually
/// updates state and/or sends messages to one more layer above
///
/// NOTE: this trait is about pure oracle work,
/// so e.g. if one asks for preimage it gives SOME data, but validity of this data
/// versus image (that depends on which hash is used) it beyond the scope of this trait
///
use core::{mem::MaybeUninit, num::NonZeroU32};

use super::kv_markers::{UsizeDeserializable, UsizeSerializable};

// We will define few aux constants to easier management of query IDs. Note that we do not really
// care if those IDs are unique on the caller side. Oracle input is non-deterministic in any case,
// so any response MUST be either treated as bag of bytes, or checked to satisfy additional constraints either
// during deserialization, or usage later on.

pub const RESERVED_SUBSPACE_MASK: u32 = 0x80_00_00_00;
pub const UART_QUERY_ID: u32 = 0xffffffff;
pub const BASIC_SUBSPACE_MASK: u32 = 0x40_00_00_00;
pub const GENERIC_SUBSPACE_MASK: u32 = BASIC_SUBSPACE_MASK | 0x00_01_00_00;
pub const PREIMAGE_SUBSPACE_MASK: u32 = BASIC_SUBSPACE_MASK | 0x00_02_00_00;
pub const ACCOUNT_AND_STORAGE_SUBSPACE_MASK: u32 = BASIC_SUBSPACE_MASK | 0x00_03_00_00;
pub const STATE_AND_MERKLE_PATHS_SUBSPACE_MASK: u32 = BASIC_SUBSPACE_MASK | 0x00_04_00_00;
pub const ADVISE_SUBSPACE_MASK: u32 = BASIC_SUBSPACE_MASK | 0x00_05_00_00;

#[allow(clippy::identity_op)]
pub const NEXT_TX_SIZE_QUERY_ID: u32 = GENERIC_SUBSPACE_MASK | 0;
pub const DISCONNECT_ORACLE_QUERY_ID: u32 = GENERIC_SUBSPACE_MASK | 1;
pub const BLOCK_METADATA_QUERY_ID: u32 = GENERIC_SUBSPACE_MASK | 2;
pub const TX_DATA_WORDS_QUERY_ID: u32 = GENERIC_SUBSPACE_MASK | 3;
pub const ZK_PROOF_DATA_INIT_QUERY_ID: u32 = GENERIC_SUBSPACE_MASK | 4;

#[allow(clippy::identity_op)]
pub const GENERIC_PREIMAGE_QUERY_ID: u32 = PREIMAGE_SUBSPACE_MASK | 0;

#[allow(clippy::identity_op)]
pub const INITIAL_STORAGE_SLOT_VALUE_QUERY_ID: u32 = ACCOUNT_AND_STORAGE_SUBSPACE_MASK | 0;

#[allow(clippy::identity_op)]
pub const INITIAL_STATE_COMMITMENT_QUERY_ID: u32 = STATE_AND_MERKLE_PATHS_SUBSPACE_MASK | 0;

#[allow(clippy::identity_op)]
pub const HISTORICAL_BLOCK_HASH_QUERY_ID: u32 = ADVISE_SUBSPACE_MASK | 0;

///
/// Convenience trait to define all expected types under one umbrella.
///
pub trait SimpleOracleQuery: Sized {
    const QUERY_ID: u32;
    type Input: UsizeSerializable + UsizeDeserializable;
    type Output: UsizeDeserializable;

    fn get<O: IOOracle>(
        oracle: &mut O,
        input: &Self::Input,
    ) -> Result<Self::Output, InternalError> {
        oracle.query_serializable(Self::QUERY_ID, input)
    }

    /// # Safety
    /// Callee must have apriori way to assume type equality
    unsafe fn transmute_input_ref_unchecked<'a, T: Sized + 'a>(val: &'a T) -> &'a Self::Input
    where
        Self::Input: 'a,
    {
        core::mem::transmute(val)
    }

    /// # Safety
    /// Callee must have apriori way to assume type equality. Will check type IDs inside just in case
    unsafe fn transmute_input_ref<'a, T: 'static + Sized>(val: &'a T) -> &'a Self::Input
    where
        Self::Input: 'static,
    {
        assert_eq!(
            core::any::TypeId::of::<T>(),
            core::any::TypeId::of::<Self::Input>()
        );
        core::mem::transmute(val)
    }

    // Copy == no Drop for now
    /// # Safety
    /// Callee must have apriori way to assume type equality. Will check type IDs inside just in case
    unsafe fn transmute_input<T: 'static + Sized + Copy>(val: T) -> Self::Input
    where
        Self::Input: 'static,
    {
        assert!(core::mem::needs_drop::<T>() == false);
        assert_eq!(
            core::any::TypeId::of::<T>(),
            core::any::TypeId::of::<Self::Input>()
        );
        core::ptr::read((&val as *const T).cast::<Self::Input>())
    }

    /// # Safety
    /// Callee must have apriori way to assume type equality. Will check type IDs inside just in case
    unsafe fn transmute_output<T: 'static + Sized>(val: Self::Output) -> T
    where
        Self::Output: 'static,
    {
        assert!(core::mem::needs_drop::<Self::Output>() == false);
        assert_eq!(
            core::any::TypeId::of::<T>(),
            core::any::TypeId::of::<Self::Output>()
        );
        core::ptr::read((&val as *const Self::Output).cast::<T>())
    }

    /// # Safety
    /// Callee must have apriori way to assume type equality
    unsafe fn transmute_output_unchecked<T: Sized>(val: Self::Output) -> T {
        assert!(core::mem::needs_drop::<Self::Output>() == false);
        core::ptr::read((&val as *const Self::Output).cast::<T>())
    }
}

///
/// Oracle interface
///
pub trait IOOracle: 'static + Sized {
    /// Iterator type that oracle returns.
    type RawIterator<'a>: ExactSizeIterator<Item = usize>;

    ///
    /// Main method to query oracle.
    /// Returns raw iterator.
    ///
    fn raw_query<'a, I: UsizeSerializable + UsizeDeserializable>(
        &'a mut self,
        query_type: u32,
        input: &I,
    ) -> Result<Self::RawIterator<'a>, InternalError>;

    ///
    /// Main method to query oracle.
    /// Returns raw iterator.
    ///
    fn raw_query_with_empty_input<'a>(
        &'a mut self,
        query_type: u32,
    ) -> Result<Self::RawIterator<'a>, InternalError> {
        self.raw_query(query_type, &())
    }

    ///
    /// Convenience method to query oracle.
    /// Returns deserialized output.
    ///
    fn query_serializable<I: UsizeSerializable + UsizeDeserializable, O: UsizeDeserializable>(
        &mut self,
        query_type: u32,
        input: &I,
    ) -> Result<O, InternalError> {
        let mut it = self.raw_query(query_type, input)?;
        let result: O = UsizeDeserializable::from_iter(&mut it).expect("must initialize");
        assert!(it.next().is_none());

        Ok(result)
    }

    // Few wrappers that return output in convenient types

    ///
    /// Returns the requested type. Expects that such query type has trivial input parameters.
    ///
    fn query_with_empty_input<T: UsizeDeserializable>(
        &mut self,
        query_type: u32,
    ) -> Result<T, InternalError> {
        self.query_serializable::<_, T>(query_type, &())
    }

    ///
    /// Returns the byte length of the next transaction.
    ///
    /// If there are no more transactions returns `None`.
    /// Note: length can't be 0, as 0 interpreted as no more transactions.
    ///
    fn try_begin_next_tx(&mut self) -> Result<Option<NonZeroU32>, InternalError> {
        let size = self.query_with_empty_input::<u32>(NEXT_TX_SIZE_QUERY_ID)?;

        Ok(NonZeroU32::new(size))
    }

    ///
    /// Convenience to expose preimage into the preallocated buffer of bounded size
    ///
    fn expose_preimage(
        &mut self,
        query_type: u32,
        hash: &Bytes32,
        destination: &mut [MaybeUninit<usize>],
    ) -> Result<usize, InternalError> {
        let mut it = self
            .raw_query(query_type, hash)
            .expect("must make an iterator for preimage");
        assert!(it.len() <= destination.len());
        let words_written = it.len();
        for dst in destination.iter_mut().take(words_written) {
            unsafe {
                // Contract of ExactSizeIterator
                dst.write(it.next().unwrap_unchecked());
            }
        }

        Ok(words_written)
    }

    fn get_bytes_from_query<A: Allocator, I: UsizeSerializable + UsizeDeserializable>(
        &mut self,
        length_query_id: u32, // must return number of bytes
        body_query_id: u32,   // must return
        input: &I,
        allocator: A,
    ) -> Result<Option<UsizeAlignedByteBox<A>>, InternalError> {
        use crate::internal_error;
        use crate::utils::USIZE_SIZE;

        let size = self.query_serializable::<I, u32>(length_query_id, input)?;
        if size == 0 {
            return Ok(None);
        }
        let num_bytes = size as usize;
        let num_words = num_bytes.next_multiple_of(USIZE_SIZE) / USIZE_SIZE;
        // NOTE: we leave some slack for 64/32 bit arch mismatches
        let num_words = num_words.next_multiple_of(2);
        let body_query_it = self.raw_query(body_query_id, input)?;
        let body_it_len = body_query_it.len();
        if body_it_len > num_words {
            return Err(internal_error!("iterator len is inconsistent"));
        }
        // create buffer
        let mut buffer = UsizeAlignedByteBox::from_usize_iterator_in(body_query_it, allocator);
        buffer.truncated_to_byte_length(num_bytes);

        Ok(Some(buffer))
    }
}

/// Extended interface to allow to define supported query types. Only to be used on the other
/// end of the wire, but placed here for consistency
pub trait IOResponder {
    fn supports_query_id(&self, query_type: u32) -> bool;

    // type QueryIDsIterator<'a>: ExactSizeIterator<Item = u32> where Self: 'a;
    fn all_supported_query_ids<'a>(&'a self) -> impl ExactSizeIterator<Item = u32> + 'a;

    fn query_serializable_static<
        I: 'static + UsizeSerializable + UsizeDeserializable,
        O: 'static + UsizeDeserializable,
    >(
        &mut self,
        query_type: u32,
        input: &I,
    ) -> Result<O, InternalError>;
}
