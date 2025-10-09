//! This module provides the core abstraction for accessing external state and data
//! during ZKsync OS execution. Oracles enable the system to query storage, preimages,
//! transaction data, and other non-deterministic information while maintaining
//! deterministic execution semantics required for zero-knowledge proofs.
//!
//! The oracle system is built around several key components:
//!
//! - **IOOracle trait**: Core interface for querying external data
//! - **Query system**: Type-safe query definitions with unique IDs (uniqueness is not enforced)
//! - **Serialization and deserialization**: `usize`-based data encoding/decoding
//! - **Query processors**: Server- or simulator-side handlers for specific query types
//!
//! # Security Model
//!
//! **Critical**: Oracle responses are treated as **untrusted input**. The oracle system does not validate data authenticity or correctness. All oracle
//! responses MUST be validated by the calling code before use.

pub mod basic_queries;
pub mod query_ids;
pub mod simple_oracle_query;
pub mod usize_serialization;

use core::num::NonZeroU32;

use crate::internal_error;
use crate::oracle::query_ids::NEXT_TX_SIZE_QUERY_ID;
use crate::oracle::usize_serialization::{UsizeDeserializable, UsizeSerializable};
use crate::system::errors::internal::InternalError;

/// Core trait for querying external, non-deterministic data during ZKsync OS execution. This is
/// an abstraction boundary on how ZKsync OS (system) gets IO information and eventually
/// updates state and/or sends messages to one more layer above.
///
/// This trait abstracts access to external state like storage, preimages, and transaction data.
/// Implementations provide the data without validating its correctness - validation occurs
/// at higher system layers. The interface is designed for zero-copy operation using exact-size
/// iterators over `usize` values.
///
/// # Design Notes
/// - All data exchange uses `usize` sequences for cross-architecture compatibility
/// - Query types are identified by `u32` IDs organized in namespaced ranges
///
/// # Security Implications
/// - Oracle responses are treated as untrusted input and MUST be validated
/// - Malformed responses can cause deserialization panics if not handled properly
/// - ZK proof verification (in combination with state and data commitments)
///   should ensure data correctness
pub trait IOOracle: 'static + Sized {
    /// Iterator type that oracle returns for raw usize values
    type RawIterator<'a>: ExactSizeIterator<Item = usize>;

    ///
    /// Main method to query oracle with typed input.
    /// Returns raw iterator over usize values that can be deserialized.
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
        let result: O = UsizeDeserializable::from_iter(&mut it)?;

        // Validate that all data was consumed to detect malformed responses
        if it.next().is_some() {
            return Err(internal_error!("Oracle response contains excess data"));
        }

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
}

/// Extended interface to allow to define supported query types. Only to be used on the other
/// end of the wire, but placed here for consistency
pub trait IOResponder {
    fn supports_query_id(&self, query_type: u32) -> bool;

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
