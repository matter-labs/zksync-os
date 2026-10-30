//! This module defines query IDs used to identify different types of oracle requests within
//! the ZKsync OS execution environment.
//!
//! Query IDs are organized using a hierarchical bitmask system for namespace isolation:
//! - Top-level masks define major categories (reserved, basic functionality)
//! - Second-level masks define subcategories within each major category
//! - Individual query IDs are assigned within each subcategory
//!
//! The subspaces themselves, and every ID of the subspaces the platform owns (system and advice),
//! come from `airbender_oracle::query_ids`, so that the queries of the platform and of ZKsync OS
//! never collide. ZKsync OS assigns the IDs inside its own subspaces: those below, and some
//! defined next to their system components (e.g. the storage models' queries).
//!
//! **IMPORTANT**: Query ID uniqueness is not enforced on the caller side. Oracle responses
//! are non-deterministic by nature and MUST be treated as untrusted input. All oracle
//! responses should either be:
//! - Treated as opaque byte arrays, or
//! - Validated against additional constraints during deserialization or subsequent usage

pub use airbender_oracle::query_ids::{
    ACCOUNT_AND_STORAGE_SUBSPACE_MASK, ADVICE_SUBSPACE_MASK, BASIC_SUBSPACE_MASK,
    BLOB_COMMITMENT_AND_PROOF_QUERY_ID, BLOCK_SUBSPACE_MASK, DISCONNECT_ORACLE_QUERY_ID,
    FIELD_OPS_ADVISE_QUERY_ID, GENERIC_SUBSPACE_MASK, MODEXP_ADVICE_QUERY_ID,
    PREIMAGE_SUBSPACE_MASK, RESERVED_SUBSPACE_MASK, STATE_AND_MERKLE_PATHS_SUBSPACE_MASK,
    SYSTEM_SUBSPACE_MASK, TRANSACTION_SUBSPACE_MASK, U256_DIV_REM_ADVICE_QUERY_ID,
    U256_WIDE_DIV_REM_ADVICE_QUERY_ID, UART_QUERY_ID,
};

// ========== Preimage Subspace Queries ==========

/// Query to retrieve preimage data for a given hash
#[allow(clippy::identity_op)]
pub const GENERIC_PREIMAGE_QUERY_ID: u32 = PREIMAGE_SUBSPACE_MASK | 0; // 0x40020000

/// Query to retrieve FRI proof bytes (sidecar) for a given
/// `statement_versioned_hash`.
pub const FRI_PROOF_QUERY_ID: u32 = PREIMAGE_SUBSPACE_MASK | 1; // 0x40020001

// ========== Account and Storage Subspace Queries ==========

/// Query to get the initial value of a storage slot before any modifications in the current block/batch
#[allow(clippy::identity_op)]
pub const INITIAL_STORAGE_SLOT_VALUE_QUERY_ID: u32 = ACCOUNT_AND_STORAGE_SUBSPACE_MASK | 0; // 0x40030000

// ========== State and Merkle Paths Subspace Queries ==========

/// Query to get the initial state commitment (root hash) before block execution
#[allow(clippy::identity_op)]
pub const INITIAL_STATE_COMMITMENT_QUERY_ID: u32 = STATE_AND_MERKLE_PATHS_SUBSPACE_MASK | 0; // 0x40040000

// ========== Transaction Subspace Queries ==========

/// Query to get the size (in bytes) of the next transaction to be processed
#[allow(clippy::identity_op)]
pub const NEXT_TX_SIZE_QUERY_ID: u32 = TRANSACTION_SUBSPACE_MASK | 0; // 0x40060000

/// Query to get transaction data words for the current transaction being processed
pub const TX_DATA_WORDS_QUERY_ID: u32 = TRANSACTION_SUBSPACE_MASK | 1; // 0x40060001

/// Query to get transaction encoding format.
pub const TX_ENCODING_FORMAT_QUERY_ID: u32 = TRANSACTION_SUBSPACE_MASK | 2; // 0x40060002

/// Query to get sender address of the transaction.
pub const TX_FROM_QUERY_ID: u32 = TRANSACTION_SUBSPACE_MASK | 3; // 0x40060003

// ========== Block or batch Subspace Queries ==========

/// Query to retrieve block metadata (timestamp, number, etc.) from the oracle
#[allow(clippy::identity_op)]
pub const BLOCK_METADATA_QUERY_ID: u32 = BLOCK_SUBSPACE_MASK | 0; // 0x40070000

/// Query to get the data required for state correctness proving (e.g. previous state commitment)
pub const ZK_PROOF_DATA_INIT_QUERY_ID: u32 = BLOCK_SUBSPACE_MASK | 1; // 0x40070001
/// Query to get needed DA commitment scheme.
pub const DA_COMMITMENT_SCHEME_QUERY_ID: u32 = BLOCK_SUBSPACE_MASK | 2; // 0x40070002
/// Query to get historical block hashes.
pub const HISTORICAL_BLOCK_HASH_QUERY_ID: u32 = BLOCK_SUBSPACE_MASK | 3; // 0x40070003
/// Query to retrieve static chain-level execution configuration.
pub const CHAIN_CONFIG_QUERY_ID: u32 = BLOCK_SUBSPACE_MASK | 4; // 0x40070004
