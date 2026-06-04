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
pub const KECCAK256_BASE_NATIVE_COST: u64 = 2_500;
pub const KECCAK256_ROUND_NATIVE_COST: u64 = 17_500;
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

// BLS12-381 native costs (EIP-2537).
// Measured via RISC-V cycle markers with non-trivial inputs.
pub const BLS12_381_G1ADD_NATIVE_COST: u64 = native_with_delegations!(194_000, 35_800, 0);
pub const BLS12_381_G2ADD_NATIVE_COST: u64 = native_with_delegations!(251_000, 39_100, 0);
// MSM: worst case per point (single-point MSM with all-ones 256-bit scalar).
// Batching amortizes in practice but we charge worst case per point;
// the EVM gas discount table handles batching.
pub const BLS12_381_G1MSM_PER_POINT_NATIVE_COST: u64 =
    native_with_delegations!(3_170_000, 398_300, 0);
pub const BLS12_381_G2MSM_PER_POINT_NATIVE_COST: u64 =
    native_with_delegations!(13_283_000, 1_126_200, 0);
// Pairing: measured with non-trivial G1/G2 generator inputs (1, 2, 4 pairs).
// Linear model fits with <0.01% error on cross-check.
pub const BLS12_381_PAIRING_NATIVE_COST: u64 = native_with_delegations!(12_140_000, 835_600, 0);
pub const BLS12_381_PAIRING_PER_PAIR_NATIVE_COST: u64 =
    native_with_delegations!(10_700_000, 830_500, 0);
// Mapping: allocation-free isogeny with Montgomery's trick for batch inversion.
pub const BLS12_381_MAP_FP_TO_G1_NATIVE_COST: u64 = native_with_delegations!(1_478_000, 246_300, 0);
pub const BLS12_381_MAP_FP2_TO_G2_NATIVE_COST: u64 =
    native_with_delegations!(4_343_000, 541_700, 0);

// Blake2f native costs (EIP-152).
// Measured via RISC-V cycle markers. No delegations — pure RISC-V computation.
pub const BLAKE2F_BASE_NATIVE_COST: u64 = 1_584;
pub const BLAKE2F_PER_ROUND_NATIVE_COST: u64 = 673;
