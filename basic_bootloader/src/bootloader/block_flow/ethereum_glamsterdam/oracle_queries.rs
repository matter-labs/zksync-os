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

// --- the aggregation program (`block_aggregation`) --------------------------------------
//
// Its inputs: the public outputs of the pre-execution check and of the ranges (what the
// proofs of those programs establish), then the preimages that the outputs commit to.
// Numbered after the range queries (`tx_range::oracle_queries`, up to 25).

const fn aggregation_id(n: u32) -> u32 {
    GENERIC_SUBSPACE_MASK | ETHEREUM_QUERIES_SUBSPACE_MASK | n
}

/// The public output of the pre-execution check (`block_aggregation::PublicOutput`)
pub const GLAMSTERDAM_AGGREGATION_PRE_EXECUTION_OUTPUT_QUERY_ID: u32 = aggregation_id(27);
/// The number of ranges (`u32`)
pub const GLAMSTERDAM_AGGREGATION_RANGE_COUNT_QUERY_ID: u32 = aggregation_id(28);
/// The public output of the range at the given index (`u32` input)
pub const GLAMSTERDAM_AGGREGATION_RANGE_OUTPUT_QUERY_ID: u32 = aggregation_id(29);
/// The preimage of a public output (`linkage::ProgramOutput` bytes; a length and data
/// query pair): program `0` is the pre-execution check, `i + 1` the range `i` (`u32` input)
pub const GLAMSTERDAM_AGGREGATION_OUTPUT_PREIMAGE_LEN_QUERY_ID: u32 = aggregation_id(30);
pub const GLAMSTERDAM_AGGREGATION_OUTPUT_PREIMAGE_DATA_QUERY_ID: u32 = aggregation_id(31);
/// The mask of the access list records a range accessed (`u32` range index; a length
/// and data query pair)
pub const GLAMSTERDAM_AGGREGATION_RANGE_MASK_LEN_QUERY_ID: u32 = aggregation_id(32);
pub const GLAMSTERDAM_AGGREGATION_RANGE_MASK_DATA_QUERY_ID: u32 = aggregation_id(33);
