//! Oracle queries of the transaction-range validation, in the Ethereum queries
//! subspace after the pre-execution ones.

use basic_system::system_implementation::ethereum_storage_model::ETHEREUM_QUERIES_SUBSPACE_MASK;
use zk_ee::oracle::query_ids::GENERIC_SUBSPACE_MASK;

const fn id(n: u32) -> u32 {
    GENERIC_SUBSPACE_MASK | ETHEREUM_QUERIES_SUBSPACE_MASK | n
}

/// The block access index range to validate (`BlockAccessIndexRange`)
pub const GLAMSTERDAM_ACCESS_INDEX_RANGE_QUERY_ID: u32 = id(16);
/// The number of transactions of the block (`u32`), proven against the transactions trie
pub const GLAMSTERDAM_TRANSACTION_COUNT_QUERY_ID: u32 = id(17);

/// The initial value tables of the pre-execution check, as byte buffers
/// (length and data query pairs): the account leaf stream by address index,
/// the fresh account mask, the slot leaf stream by slot position (see
/// `witness::InitialValues`)
pub const GLAMSTERDAM_INITIAL_ACCOUNTS_LEN_QUERY_ID: u32 = id(18);
pub const GLAMSTERDAM_INITIAL_ACCOUNTS_DATA_QUERY_ID: u32 = id(19);
pub const GLAMSTERDAM_FRESH_ACCOUNTS_LEN_QUERY_ID: u32 = id(20);
pub const GLAMSTERDAM_FRESH_ACCOUNTS_DATA_QUERY_ID: u32 = id(21);
pub const GLAMSTERDAM_INITIAL_SLOTS_LEN_QUERY_ID: u32 = id(22);
pub const GLAMSTERDAM_INITIAL_SLOTS_DATA_QUERY_ID: u32 = id(23);

/// The block's running gas sums at the start of the range (`RangeStartGas`), for a
/// range that does not start at the first transaction: unverified here, linked across
/// ranges by the aggregation
pub const GLAMSTERDAM_RANGE_START_GAS_QUERY_ID: u32 = id(24);
