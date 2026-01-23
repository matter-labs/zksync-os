pub const ECRECOVER_HOOK_ADDRESS_LOW: u16 = 0x0001;
pub const SHA256_HOOK_ADDRESS_LOW: u16 = 0x0002;
pub const RIPEMD160_HOOK_ADDRESS_LOW: u16 = 0x0003;
pub const ID_HOOK_ADDRESS_LOW: u16 = 0x0004;
pub const MODEXP_HOOK_ADDRESS_LOW: u16 = 0x0005;
pub const ECADD_HOOK_ADDRESS_LOW: u16 = 0x0006;
pub const ECMUL_HOOK_ADDRESS_LOW: u16 = 0x0007;
pub const ECPAIRING_HOOK_ADDRESS_LOW: u16 = 0x0008;
#[cfg(any(feature = "eip-152", feature = "mock-unsupported-precompiles"))]
pub const BLAKE2F_HOOK_ADDRESS_LOW: u16 = 0x0009;
#[cfg(any(
    feature = "point_eval_precompile",
    feature = "mock-unsupported-precompiles"
))]
pub const POINT_EVAL_HOOK_ADDRESS_LOW: u16 = 0x000a;
#[cfg(feature = "p256_precompile")]
pub const P256_VERIFY_PREHASH_HOOK_ADDRESS_LOW: u16 = 0x0100;

pub const BLS12_G1ADD_ADDRESS_LOW: u16 = 0x0b;
pub const BLS12_G1MSM_ADDRESS_LOW: u16 = 0x0c;
pub const BLS12_G2ADD_ADDRESS_LOW: u16 = 0x0d;
pub const BLS12_G2MSM_ADDRESS_LOW: u16 = 0x0e;
pub const BLS12_PAIRING_CHECK_ADDRESS_LOW: u16 = 0x0f;
pub const BLS12_MAP_FP_TO_G1_ADDRESS_LOW: u16 = 0x10;
pub const BLS12_MAP_FP2_TO_G2_ADDRESS_LOW: u16 = 0x11;

/// Source of truth for supported EVM precompile addresses
pub const PRECOMPILE_ADDRESSES_LOWS: &[u16] = &[
    ECRECOVER_HOOK_ADDRESS_LOW,
    SHA256_HOOK_ADDRESS_LOW,
    RIPEMD160_HOOK_ADDRESS_LOW,
    ID_HOOK_ADDRESS_LOW,
    MODEXP_HOOK_ADDRESS_LOW,
    ECADD_HOOK_ADDRESS_LOW,
    ECMUL_HOOK_ADDRESS_LOW,
    ECPAIRING_HOOK_ADDRESS_LOW,
    #[cfg(any(feature = "eip-152", feature = "mock-unsupported-precompiles"))]
    BLAKE2F_HOOK_ADDRESS_LOW,
    #[cfg(any(
        feature = "point_eval_precompile",
        feature = "mock-unsupported-precompiles"
    ))]
    POINT_EVAL_HOOK_ADDRESS_LOW,
    #[cfg(feature = "p256_precompile")]
    P256_VERIFY_PREHASH_HOOK_ADDRESS_LOW,
    #[cfg(feature = "eip-2537")]
    BLS12_G1ADD_ADDRESS_LOW,
    #[cfg(feature = "eip-2537")]
    BLS12_G1MSM_ADDRESS_LOW,
    #[cfg(feature = "eip-2537")]
    BLS12_G2ADD_ADDRESS_LOW,
    #[cfg(feature = "eip-2537")]
    BLS12_G2MSM_ADDRESS_LOW,
    #[cfg(feature = "eip-2537")]
    BLS12_PAIRING_CHECK_ADDRESS_LOW,
    #[cfg(feature = "eip-2537")]
    BLS12_MAP_FP_TO_G1_ADDRESS_LOW,
    #[cfg(feature = "eip-2537")]
    BLS12_MAP_FP2_TO_G2_ADDRESS_LOW,
];
