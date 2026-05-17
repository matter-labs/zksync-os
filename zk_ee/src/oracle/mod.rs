//! This module provides the core abstraction for accessing external state and data
//! during ZKsync OS execution. Oracles enable the system to query storage, preimages,
//! transaction data, and other non-deterministic information while maintaining
//! deterministic execution semantics required for zero-knowledge proofs.
//!
//! The oracle system is built around several key components:
//!
//! - **IOOracle trait**: Core interface for querying external data via serde
//! - **Query system**: Type-safe query definitions with unique IDs (uniqueness is not enforced)
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

use crate::oracle::query_ids::NEXT_TX_SIZE_QUERY_ID;
use crate::system::errors::internal::InternalError;
use core::num::NonZeroU32;
use serde::{de::DeserializeOwned, Serialize};

/// Core trait for querying external, non-deterministic data during ZKsync OS execution. This is
/// an abstraction boundary on how ZKsync OS (system) gets IO information and eventually
/// updates state and/or sends messages to one more layer above.
///
/// This trait abstracts access to external state like storage, preimages, and transaction data.
/// Implementations provide the data without validating its correctness - validation occurs
/// at higher system layers. All data exchange uses serde serialization/deserialization.
///
/// # Design Notes
/// - Query types are identified by `u32` IDs organized in namespaced ranges
///
/// # Security Implications
/// - Oracle responses are treated as untrusted input and MUST be validated
/// - Malformed responses can cause deserialization panics if not handled properly
/// - ZK proof verification (in combination with state and data commitments)
///   should ensure data correctness
pub trait IOOracle: 'static + Sized {
    /// Main method to query oracle with typed input and typed output.
    fn query<I: Serialize, O: DeserializeOwned + Serialize>(
        &mut self,
        query_type: u32,
        input: &I,
    ) -> Result<O, InternalError>;

    /// Convenience method to query oracle with no input.
    fn query_with_empty_input<O: DeserializeOwned + Serialize>(
        &mut self,
        query_type: u32,
    ) -> Result<O, InternalError> {
        self.query::<(), O>(query_type, &())
    }

    /// Returns the byte length of the next transaction.
    ///
    /// If there are no more transactions returns `None`.
    /// Note: length can't be 0, as 0 interpreted as no more transactions.
    fn try_begin_next_tx(&mut self) -> Result<Option<NonZeroU32>, InternalError> {
        let size: u32 = self.query_with_empty_input(NEXT_TX_SIZE_QUERY_ID)?;
        Ok(NonZeroU32::new(size))
    }

    /// Query oracle and return raw bytes.
    fn query_bytes<I: Serialize>(
        &mut self,
        query_type: u32,
        input: &I,
    ) -> Result<alloc::vec::Vec<u8>, InternalError> {
        self.query::<I, alloc::vec::Vec<u8>>(query_type, input)
    }
}
