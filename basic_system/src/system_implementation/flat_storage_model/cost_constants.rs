use evm_interpreter::gas_constants::SELFBALANCE;
use evm_interpreter::gas_constants::{ADDRESS_ACCESS_COST_COLD, ADDRESS_ACCESS_COST_WARM};
use evm_interpreter::ERGS_PER_GAS;
use zk_ee::native_with_delegations;
use zk_ee::system::Ergs;

/// Native cost for querying the preimage cache
pub const PREIMAGE_CACHE_GET_NATIVE_COST: u64 = 500;
pub const PREIMAGE_CACHE_SET_NATIVE_COST: u64 = 500;

/// Native costs for blake2s hashing
/// NOTE: To recompute if the blake coefficient changes
pub const BLAKE2S_BASE_NATIVE_COST: u64 = 800;
pub const BLAKE2S_ROUND_NATIVE_COST: u64 = 340;
pub const BLAKE2S_CHUNK_SIZE: u64 = 64;

// Storage costs
// Avg is ~10x smaller, maybe we can reduce it, but it depends on cache state.
pub const WARM_STORAGE_READ_NATIVE_COST: u64 = 4000;
// Avg is ~10x smaller, maybe we can reduce it, but it depends on cache state.
pub const WARM_STORAGE_WRITE_EXTRA_NATIVE_COST: u64 = 1000;
// Estimation based on worst-case
pub const COLD_EXISTING_STORAGE_READ_NATIVE_COST: u64 = native_with_delegations!(100_000, 0, 1320);
pub const COLD_NEW_STORAGE_READ_NATIVE_COST: u64 = 2 * COLD_EXISTING_STORAGE_READ_NATIVE_COST;
pub const COLD_EXISTING_STORAGE_WRITE_EXTRA_NATIVE_COST: u64 =
    native_with_delegations!(40_000, 0, 660);
pub const COLD_NEW_STORAGE_WRITE_EXTRA_NATIVE_COST: u64 =
    native_with_delegations!(100_000, 0, 1300);

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

// Helper to compute hashing native cost
pub fn blake2s_native_cost(len: usize) -> u64 {
    let num_rounds = (len as u64).div_ceil(BLAKE2S_CHUNK_SIZE);
    num_rounds
        .saturating_mul(BLAKE2S_ROUND_NATIVE_COST)
        .saturating_add(BLAKE2S_BASE_NATIVE_COST)
}
