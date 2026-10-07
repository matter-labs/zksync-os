//! Oracle queries of the Glamsterdam STF, in the Ethereum queries subspace next
//! to the pre-Glamsterdam ones (`block_flow::ethereum::oracle_queries`).

use basic_system::system_implementation::ethereum_storage_model::ETHEREUM_QUERIES_SUBSPACE_MASK;
use zk_ee::oracle::query_ids::GENERIC_SUBSPACE_MASK;

/// Length in bytes of the RLP encoding of the block access list (EIP-7928)
pub const ETHEREUM_BLOCK_ACCESS_LIST_BUFFER_LEN_QUERY_ID: u32 =
    GENERIC_SUBSPACE_MASK | ETHEREUM_QUERIES_SUBSPACE_MASK | 7;
/// The RLP encoding of the block access list
pub const ETHEREUM_BLOCK_ACCESS_LIST_BUFFER_DATA_QUERY_ID: u32 =
    GENERIC_SUBSPACE_MASK | ETHEREUM_QUERIES_SUBSPACE_MASK | 8;
