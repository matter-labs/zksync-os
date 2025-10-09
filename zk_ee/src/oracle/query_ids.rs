//! This module defines query IDs used to identify different types of oracle requests within
//! the ZKsync OS execution environment.
//!
//! Query IDs are organized using a hierarchical bitmask system for namespace isolation:
//! - Top-level masks define major categories (reserved, basic functionality)
//! - Second-level masks define subcategories within each major category
//! - Individual query IDs are assigned within each subcategory
//!
//! **NOTE**: This list is not exhaustive. Additional implementation-specific query IDs can be
//! defined in the corresponding system components (e.g. `modexp` queries)
//! as needed. The bitmask structure provides namespace isolation to prevent conflicts between
//! different subsystems.
//!
//! **IMPORTANT**: Query ID uniqueness is not enforced on the caller side. Oracle responses
//! are non-deterministic by nature and MUST be treated as untrusted input. All oracle
//! responses should either be:
//! - Treated as opaque byte arrays, or
//! - Validated against additional constraints during deserialization or subsequent usage

// # Query ID Bitmask Structure

/// Top bit (0x80_00_00_00) reserved
pub const RESERVED_SUBSPACE_MASK: u32 = 0x80_00_00_00;
/// Second bit (0x40_00_00_00) for basic oracle functionality
pub const BASIC_SUBSPACE_MASK: u32 = 0x40_00_00_00;

// - Second byte organizes different query subcategories

/// Generic transaction and block-level queries
pub const GENERIC_SUBSPACE_MASK: u32 = BASIC_SUBSPACE_MASK | 0x00_01_00_00; // 0x40010000
/// Preimage and hash-related queries
pub const PREIMAGE_SUBSPACE_MASK: u32 = BASIC_SUBSPACE_MASK | 0x00_02_00_00; // 0x40020000
/// Account state and storage queries
pub const ACCOUNT_AND_STORAGE_SUBSPACE_MASK: u32 = BASIC_SUBSPACE_MASK | 0x00_03_00_00; // 0x40030000
/// State root and Merkle path queries
pub const STATE_AND_MERKLE_PATHS_SUBSPACE_MASK: u32 = BASIC_SUBSPACE_MASK | 0x00_04_00_00; // 0x40040000
/// Computational advice queries (e.g. division/modexp advice)
pub const ADVICE_SUBSPACE_MASK: u32 = BASIC_SUBSPACE_MASK | 0x00_05_00_00; // 0x40050000

/// Special case: UART output query ID (for debugging purposes)
pub const UART_QUERY_ID: u32 = 0xff_ff_ff_ff;

// ========== Generic Subspace Queries ==========

/// Query to get the size (in bytes) of the next transaction to be processed
#[allow(clippy::identity_op)]
pub const NEXT_TX_SIZE_QUERY_ID: u32 = GENERIC_SUBSPACE_MASK | 0; // 0x40010000

/// Signal to disconnect from external oracle and switch to autonomous execution mode
pub const DISCONNECT_ORACLE_QUERY_ID: u32 = GENERIC_SUBSPACE_MASK | 1; // 0x40010001

/// Query to retrieve block metadata (timestamp, number, etc.) from the oracle
pub const BLOCK_METADATA_QUERY_ID: u32 = GENERIC_SUBSPACE_MASK | 2; // 0x40010002

/// Query to get transaction data words for the current transaction being processed
pub const TX_DATA_WORDS_QUERY_ID: u32 = GENERIC_SUBSPACE_MASK | 3; // 0x40010003

/// Query to get the data required for state correctness proving (e.g. previous state commitment)
pub const ZK_PROOF_DATA_INIT_QUERY_ID: u32 = GENERIC_SUBSPACE_MASK | 4; // 0x40010004

// ========== Preimage Subspace Queries ==========

/// Query to retrieve preimage data for a given hash
#[allow(clippy::identity_op)]
pub const GENERIC_PREIMAGE_QUERY_ID: u32 = PREIMAGE_SUBSPACE_MASK | 0; // 0x40020000

// ========== Account and Storage Subspace Queries ==========

/// Query to get the initial value of a storage slot before any modifications in the current block/batch
#[allow(clippy::identity_op)]
pub const INITIAL_STORAGE_SLOT_VALUE_QUERY_ID: u32 = ACCOUNT_AND_STORAGE_SUBSPACE_MASK | 0; // 0x40030000

// ========== State and Merkle Paths Subspace Queries ==========

/// Query to get the initial state commitment (root hash) before block execution
#[allow(clippy::identity_op)]
pub const INITIAL_STATE_COMMITMENT_QUERY_ID: u32 = STATE_AND_MERKLE_PATHS_SUBSPACE_MASK | 0; // 0x40040000
