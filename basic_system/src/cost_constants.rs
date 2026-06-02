use evm_interpreter::ERGS_PER_GAS;
use zk_ee::{native_with_delegations, system::Ergs};

#[allow(clippy::identity_op)]
pub const BLAKE2S256_PER_ROUND_COST_ERGS: Ergs = Ergs(1 * ERGS_PER_GAS);

pub const SHA256_STATIC_COST_ERGS: Ergs = Ergs(60 * ERGS_PER_GAS);
pub const SHA256_PER_WORD_COST_ERGS: Ergs = Ergs(12 * ERGS_PER_GAS);

pub const RIPEMD_160_STATIC_COST_ERGS: Ergs = Ergs(600 * ERGS_PER_GAS);
pub const RIPEMD_160_PER_WORD_COST_ERGS: Ergs = Ergs(120 * ERGS_PER_GAS);
#[cfg(not(feature = "modexp-repricing"))]
pub const MODEXP_MINIMAL_COST_ERGS: Ergs = Ergs(200 * ERGS_PER_GAS);
#[cfg(feature = "modexp-repricing")]
pub const MODEXP_MINIMAL_COST_ERGS: Ergs = Ergs(500 * ERGS_PER_GAS);
pub const P256_VERIFY_COST_ERGS: Ergs = Ergs(6900 * ERGS_PER_GAS);
pub const ECRECOVER_COST_ERGS: Ergs = Ergs(3000 * ERGS_PER_GAS);
pub const BN254_ECADD_COST_ERGS: Ergs = Ergs(150 * ERGS_PER_GAS);
pub const BN254_ECMUL_COST_ERGS: Ergs = Ergs(6000 * ERGS_PER_GAS);
pub const BN254_PAIRING_STATIC_COST_ERGS: Ergs = Ergs(45000 * ERGS_PER_GAS);
pub const BN254_PAIRING_COST_PER_PAIR_ERGS: Ergs = Ergs(34000 * ERGS_PER_GAS);
pub const POINT_EVALUATION_COST_ERGS: Ergs = Ergs(50_000 * ERGS_PER_GAS);
pub const EVM_BYTECODE_MAX_ROUNDS_TO_DECOMMIT: Ergs = Ergs(180);

pub const ECRECOVER_NATIVE_COST: u64 = native_with_delegations!(350_000, 43_000, 0);
/// Native costs for keccak256 hashing.
/// Each keccak f1600 permutation produces this many delegations
/// (mirrors NUM_DELEGATION_CALLS_FOR_KECCAK_F1600 from common_constants).
const KECCAK_DELEGATIONS_PER_ROUND: u64 = 649;
/// Per-round RISC-V overhead for absorbing input into the keccak state.
const KECCAK_RISC_V_CYCLES_PER_ROUND: u64 = 1_250;
pub const KECCAK256_BASE_NATIVE_COST: u64 = 400;
pub const KECCAK256_ROUND_NATIVE_COST: u64 = KECCAK_DELEGATIONS_PER_ROUND
    * zk_ee::system::constants::KECCAK_DELEGATION_COEFFICIENT
    + KECCAK_RISC_V_CYCLES_PER_ROUND;
pub const KECCAK256_CHUNK_SIZE: usize = 136;
pub const SHA256_BASE_NATIVE_COST: u64 = 1_600;
pub const SHA256_ROUND_NATIVE_COST: u64 = 4_200;
pub const SHA256_CHUNK_SIZE: usize = 64;
pub const RIPEMD160_BASE_NATIVE_COST: u64 = 1_600;
pub const RIPEMD160_ROUND_NATIVE_COST: u64 = 4_200;
pub const RIPEMD160_CHUNK_SIZE: usize = 64;
/// Native costs for blake2s hashing.
/// NOTE: To recompute if the blake coefficient changes.
pub const BLAKE2S_BASE_NATIVE_COST: u64 = 800;
pub const BLAKE2S_ROUND_NATIVE_COST: u64 = 340;
pub const BLAKE2S_CHUNK_SIZE: usize = 64;

/// Helper to compute blake2s hashing native cost for a given input length.
pub const fn blake2s_native_cost(len: usize) -> u64 {
    let num_rounds = len.div_ceil(BLAKE2S_CHUNK_SIZE) as u64;
    num_rounds
        .saturating_mul(BLAKE2S_ROUND_NATIVE_COST)
        .saturating_add(BLAKE2S_BASE_NATIVE_COST)
}
pub const BN254_ECADD_NATIVE_COST: u64 = native_with_delegations!(46_000, 1650, 0);
pub const BN254_ECMUL_NATIVE_COST: u64 = native_with_delegations!(600_000, 41_000, 0);
pub const BN254_PAIRING_BASE_NATIVE_COST: u64 = native_with_delegations!(13_000_000, 500_000, 0);
pub const BN254_PAIRING_PER_PAIR_NATIVE_COST: u64 = BN254_PAIRING_BASE_NATIVE_COST;
#[cfg(not(feature = "modexp-repricing"))]
pub const MODEXP_WORST_CASE_NATIVE_PER_GAS: u64 = 300;
#[cfg(feature = "modexp-repricing")]
pub const MODEXP_WORST_CASE_NATIVE_PER_GAS: u64 = 500;
pub const P256_NATIVE_COST: u64 = native_with_delegations!(500_000, 71_000, 0);
// TODO(EVM-1178) Add more vectors and benchmark cost better
pub const POINT_EVALUATION_NATIVE_COST: u64 = native_with_delegations!(49_900_000, 3_301_000, 0);
