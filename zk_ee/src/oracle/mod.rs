//! This module provides the core abstraction for accessing external state and data
//! during ZKsync OS execution. Oracles enable the system to query storage, preimages,
//! transaction data, and other non-deterministic information while maintaining
//! deterministic execution semantics required for zero-knowledge proofs.
//!
//! The oracle system uses a u32 word-aligned format ([`word_layout::WordLayout`]) for
//! all data exchange, providing architecture-independent serialization with optimal
//! performance on RISC-V proving targets.
//!
//! # Security Model
//!
//! **Critical**: Oracle responses are treated as **untrusted input**. All oracle
//! responses MUST be validated by the calling code before use.

pub mod basic_queries;
pub mod query_ids;
pub mod simple_oracle_query;
pub mod word_layout;

use crate::oracle::query_ids::NEXT_TX_SIZE_QUERY_ID;
use crate::system::errors::internal::InternalError;
use core::num::NonZeroU32;
use word_layout::WordLayout;

/// Core trait for querying external, non-deterministic data during ZKsync OS execution.
///
/// All data exchange uses the [`WordLayout`] u32 word-aligned format.
/// Query types are identified by `u32` IDs organized in namespaced ranges.
///
/// # Security
/// Oracle responses are untrusted input and MUST be validated by callers.
pub trait IOOracle: 'static + Sized {
    fn query<I: WordLayout, O: WordLayout>(
        &mut self,
        query_type: u32,
        input: &I,
    ) -> Result<O, InternalError>;

    fn query_with_empty_input<O: WordLayout>(
        &mut self,
        query_type: u32,
    ) -> Result<O, InternalError> {
        self.query::<(), O>(query_type, &())
    }

    fn try_begin_next_tx(&mut self) -> Result<Option<NonZeroU32>, InternalError> {
        let size: u32 = self.query_with_empty_input(NEXT_TX_SIZE_QUERY_ID)?;
        Ok(NonZeroU32::new(size))
    }

    fn query_bytes<I: WordLayout>(
        &mut self,
        query_type: u32,
        input: &I,
    ) -> Result<alloc::vec::Vec<u8>, InternalError> {
        self.query::<I, alloc::vec::Vec<u8>>(query_type, input)
    }

    /// Query oracle, returning bytes directly in a UsizeAlignedByteBox.
    /// Default impl goes through query_bytes + copy. ProvingOracle overrides
    /// to read the Vec<u8> wire format directly into the box — one allocation,
    /// zero copies.
    fn query_byte_box<I: WordLayout, A: core::alloc::Allocator>(
        &mut self,
        query_type: u32,
        input: &I,
        allocator: A,
    ) -> Result<crate::utils::UsizeAlignedByteBox<A>, InternalError> {
        let bytes = self.query_bytes(query_type, input)?;
        Ok(crate::utils::UsizeAlignedByteBox::from_slice_in(
            &bytes, allocator,
        ))
    }

    /// Query oracle, reading the response into an existing value.
    /// Reuses heap allocations in the output where possible (e.g. Vec fields).
    fn query_into<I: WordLayout, O: WordLayout>(
        &mut self,
        query_type: u32,
        input: &I,
        output: &mut O,
    ) -> Result<(), InternalError> {
        *output = self.query(query_type, input)?;
        Ok(())
    }
}
