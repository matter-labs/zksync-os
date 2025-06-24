use evm_interpreter::ERGS_PER_GAS;
use zk_ee::{native_with_delegations, system::Ergs};

pub const KECCAK256_STATIC_COST_ERGS: Ergs = Ergs(30 * ERGS_PER_GAS);
pub const KECCAK256_PER_WORD_COST_ERGS: Ergs = Ergs(6 * ERGS_PER_GAS);
#[allow(clippy::identity_op)]
pub const BLAKE2S256_PER_ROUND_COST_ERGS: Ergs = Ergs(1 * ERGS_PER_GAS);

pub const SHA256_STATIC_COST_ERGS: Ergs = Ergs(60 * ERGS_PER_GAS);
pub const SHA256_PER_WORD_COST_ERGS: Ergs = Ergs(12 * ERGS_PER_GAS);

pub const RIPEMD_160_STATIC_COST_ERGS: Ergs = Ergs(600 * ERGS_PER_GAS);
pub const RIPEMD_160_PER_WORD_COST_ERGS: Ergs = Ergs(120 * ERGS_PER_GAS);
pub const MODEXP_MINIMAL_COST_ERGS: Ergs = Ergs(200 * ERGS_PER_GAS);
pub const P256_VERIFY_COST_ERGS: Ergs = Ergs(5000 * ERGS_PER_GAS);
pub const ECRECOVER_COST_ERGS: Ergs = Ergs(3000 * ERGS_PER_GAS);
pub const BN254_ECADD_COST_ERGS: Ergs = Ergs(150 * ERGS_PER_GAS);
pub const BN254_ECMUL_COST_ERGS: Ergs = Ergs(6000 * ERGS_PER_GAS);
pub const BN254_PAIRING_STATIC_COST_ERGS: Ergs = Ergs(45000 * ERGS_PER_GAS);
pub const BN254_PAIRING_COST_PER_PAIR_ERGS: Ergs = Ergs(34000 * ERGS_PER_GAS);
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
// TODO (EVM-1071): make sure what the worst-case is
pub const BN254_ECADD_NATIVE_COST: u64 = native_with_delegations!(46_000, 1650, 0);
// TODO (EVM-1071): make sure what the worst-case is
pub const BN254_ECMUL_NATIVE_COST: u64 = native_with_delegations!(825_000, 62_000, 0);
// TODO (EVM-1071): make sure what the worst-case is
pub const BN254_PAIRING_BASE_NATIVE_COST: u64 = native_with_delegations!(13_000_000, 500_000, 0);
// TODO (EVM-1071): make sure what the worst-case is
pub const BN254_PAIRING_PER_PAIR_NATIVE_COST: u64 = BN254_PAIRING_BASE_NATIVE_COST;
// TODO (EVM-1071): add more precise calculation
// Note: after the modexp optimization, it should be ~200.
#[cfg(feature = "evm-compatibility")]
pub const MODEXP_WORST_CASE_NATIVE_PER_GAS: u64 = 200;
#[cfg(not(feature = "evm-compatibility"))]
pub const MODEXP_WORST_CASE_NATIVE_PER_GAS: u64 = 5_600;
