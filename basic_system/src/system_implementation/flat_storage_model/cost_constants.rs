use evm_interpreter::gas_constants::SELFBALANCE;
use evm_interpreter::gas_constants::{ADDRESS_ACCESS_COST_COLD, ADDRESS_ACCESS_COST_WARM};
use evm_interpreter::ERGS_PER_GAS;
use zk_ee::native_with_delegations;
use zk_ee::system::Ergs;

/// Native cost for querying the preimage cache
pub const PREIMAGE_CACHE_GET_NATIVE_COST: u64 = 500;
pub const PREIMAGE_CACHE_SET_NATIVE_COST: u64 = 500;

// Storage costs
// Avg is ~10x smaller, maybe we can reduce it, but it depends on cache state.
pub const WARM_STORAGE_READ_NATIVE_COST: u64 = 4000;
// Avg is ~10x smaller, maybe we can reduce it, but it depends on cache state.
pub const WARM_STORAGE_WRITE_EXTRA_NATIVE_COST: u64 = 1000;
/// Native cost of a single Merkle path traversal (DEPTH=64 blake2s hashes).
const SINGLE_MERKLE_PATH_NATIVE_COST: u64 = native_with_delegations!(100_000, 0, 1320);

// Cold storage costs, derived from Merkle path counts (see docs/system/io/tree.md).
// Read and write-extra are charged separately; write-extra covers the additional
// paths beyond those already paid by the read.
pub const COLD_EXISTING_STORAGE_READ_NATIVE_COST: u64 = SINGLE_MERKLE_PATH_NATIVE_COST;
pub const COLD_NEW_STORAGE_READ_NATIVE_COST: u64 = 2 * SINGLE_MERKLE_PATH_NATIVE_COST;
pub const COLD_EXISTING_STORAGE_WRITE_EXTRA_NATIVE_COST: u64 = SINGLE_MERKLE_PATH_NATIVE_COST;
pub const COLD_NEW_STORAGE_WRITE_EXTRA_NATIVE_COST: u64 = 3 * SINGLE_MERKLE_PATH_NATIVE_COST;

// Account persist costs: proactive charge for the deferred 0x8003 write + preimage hash.
// Existing account: 1 merkle path write extra.
pub const ACCOUNT_PERSIST_EXISTING_WRITE_NATIVE_COST: u64 =
    COLD_EXISTING_STORAGE_WRITE_EXTRA_NATIVE_COST;
// New account: 3 merkle paths write extra.
pub const ACCOUNT_PERSIST_NEW_WRITE_NATIVE_COST: u64 = COLD_NEW_STORAGE_WRITE_EXTRA_NATIVE_COST;

pub const COLD_PROPERTIES_ACCESS_EXTRA_COST_ERGS: Ergs =
    Ergs((ADDRESS_ACCESS_COST_COLD - ADDRESS_ACCESS_COST_WARM) * ERGS_PER_GAS);
pub const WARM_PROPERTIES_ACCESS_COST_ERGS: Ergs = Ergs(ADDRESS_ACCESS_COST_WARM * ERGS_PER_GAS);
// Taken from EVM's SELFBALANCE
pub const KNOWN_TO_BE_WARM_PROPERTIES_ACCESS_COST_ERGS: Ergs = Ergs(SELFBALANCE * ERGS_PER_GAS);

// Avg is ~10x smaller, maybe we can reduce it, but it depends on cache state.
pub const WARM_ACCOUNT_CACHE_ACCESS_NATIVE_COST: u64 = 4000;
// Avg is ~10x smaller, maybe we can reduce it, but it depends on cache state.
pub const WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST: u64 = 1000;

// Avg is ~10x smaller, maybe we can reduce it, but it depends on cache state.
pub const WARM_TSTORAGE_READ_NATIVE_COST: u64 = 4000;
// Avg is ~10x smaller, maybe we can reduce it, but it depends on cache state.
pub const WARM_TSTORAGE_WRITE_NATIVE_COST: u64 = 4000;

// Avg is ~6x smaller, maybe we can reduce it, but it depends on the
// quasi vec.
pub const EVENT_STORAGE_BASE_NATIVE_COST: u64 = 6000;
pub const EVENT_TOPIC_NATIVE_COST: u64 = 200;
pub const EVENT_DATA_PER_BYTE_COST: u64 = 2;
/// Upper bound on the RLP framing of one receipt log, excluding its topic
/// entries (33 bytes each) and data payload: 21 bytes for the encoded address,
/// 2 for the topics-list header (at most four topics), and up to 9 each for the
/// data and outer log-list headers.
pub const RECEIPT_LOG_RLP_OVERHEAD_BYTES: u64 = 21 + 2 + 9 + 9;

const INTEROP_ROOT_BYTE_LENGTH: u64 = 32 * 4;
// Same costs as for events, as the same structure is used.
pub const INTEROP_ROOT_STORAGE_NATIVE_COST: u64 =
    EVENT_STORAGE_BASE_NATIVE_COST + INTEROP_ROOT_BYTE_LENGTH * EVENT_DATA_PER_BYTE_COST;

// Same costs as for events, as the same structure is used.
pub const SL_CHAIN_ID_BYTE_LENGTH: u64 = 32;
pub const NEW_SL_CHAIN_ID_STORAGE_NATIVE_COST: u64 =
    EVENT_STORAGE_BASE_NATIVE_COST + SL_CHAIN_ID_BYTE_LENGTH * EVENT_DATA_PER_BYTE_COST;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interop_root_storage_charges_four_words() {
        assert_eq!(INTEROP_ROOT_BYTE_LENGTH, 4 * 32);
        assert_eq!(
            INTEROP_ROOT_STORAGE_NATIVE_COST,
            EVENT_STORAGE_BASE_NATIVE_COST + 4 * 32 * EVENT_DATA_PER_BYTE_COST
        );
    }
}
