use ruint::aliases::B160;

// EVM precompiles
// blake2f(0x09) and point evaluation(0x0a) are not supported currently
pub const ECRECOVER_HOOK_ADDRESS_LOW: u16 = 0x0001;
pub const SHA256_HOOK_ADDRESS_LOW: u16 = 0x0002;
pub const RIPEMD160_HOOK_ADDRESS_LOW: u16 = 0x0003;
pub const ID_HOOK_ADDRESS_LOW: u16 = 0x0004;
pub const MODEXP_HOOK_ADDRESS_LOW: u16 = 0x0005;
pub const ECADD_HOOK_ADDRESS_LOW: u16 = 0x0006;
pub const ECMUL_HOOK_ADDRESS_LOW: u16 = 0x0007;
pub const ECPAIRING_HOOK_ADDRESS_LOW: u16 = 0x0008;
#[cfg(feature = "mock-unsupported-precompiles")]
pub const BLAKE_HOOK_ADDRESS_LOW: u16 = 0x0009;
#[cfg(feature = "mock-unsupported-precompiles")]
pub const POINT_EVAL_HOOK_ADDRESS_LOW: u16 = 0x000a;
#[cfg(feature = "p256_precompile")]
pub const P256_VERIFY_PREHASH_HOOK_ADDRESS_LOW: u16 = 0x0100;

// bootloader formal address used to collect fees
pub const BOOTLOADER_FORMAL_ADDRESS: B160 = B160::from_limbs([0x8001, 0, 0]);

// Contract Deployer system hook (contract) needed for all envs (force deploy)
pub const CONTRACT_DEPLOYER_ADDRESS_LOW: u16 = 0x8006;
pub const CONTRACT_DEPLOYER_ADDRESS: B160 =
    B160::from_limbs([CONTRACT_DEPLOYER_ADDRESS_LOW as u64, 0, 0]);

// l2 to l1 messenger system hook(contact) needed for all envs
pub const L1_MESSENGER_ADDRESS_LOW: u16 = 0x8008;
pub const L1_MESSENGER_ADDRESS: B160 = B160::from_limbs([L1_MESSENGER_ADDRESS_LOW as u64, 0, 0]);

// l2 base token system hook (contract) needed for all envs (base token withdrawals)
pub const L2_BASE_TOKEN_ADDRESS_LOW: u16 = 0x800a;
pub const L2_BASE_TOKEN_ADDRESS: B160 = B160::from_limbs([L2_BASE_TOKEN_ADDRESS_LOW as u64, 0, 0]);

// ERA VM system contracts (in fact we need implement only the methods that should be available for user contracts)
// TODO: may be better to implement as ifs inside EraVM EE
pub const ACCOUNT_CODE_STORAGE_STORAGE_ADDRESS: B160 = B160::from_limbs([0x8002, 0, 0]);
pub const KNOWN_CODE_STORAGE_ADDRESS: B160 = B160::from_limbs([0x8004, 0, 0]);
pub const IMMUTABLE_SIMULATOR_ADDRESS: B160 = B160::from_limbs([0x8005, 0, 0]);
// TODO: is a contract?
pub const FORCE_DEPLOYER_ADDRESS: B160 = B160::from_limbs([0x8007, 0, 0]);
pub const MSG_VALUE_SIMULATOR_ADDRESS: B160 = B160::from_limbs([0x8009, 0, 0]);
pub const BASE_TOKEN_ADDRESS: B160 = B160::from_limbs([0x800a, 0, 0]);
pub const SYSTEM_CONTEXT_ADDRESS: B160 = B160::from_limbs([0x800b, 0, 0]);
// TODO: bootloader utilities is no longer needed
pub const BOOTLOADER_UTILITIES_ADDRESS: B160 = B160::from_limbs([0x800c, 0, 0]);
pub const EVENT_WRITER_ADDRESS: B160 = B160::from_limbs([0x800d, 0, 0]);
pub const COMPRESSOR_ADDRESS: B160 = B160::from_limbs([0x800e, 0, 0]);
pub const COMPLEX_UPGRADER_ADDRESS: B160 = B160::from_limbs([0x800f, 0, 0]);
pub const KECCAK_SYSTEM_CONTRACT_ADDRESS: B160 = B160::from_limbs([0x8010, 0, 0]);
pub const PUBDATA_CHUNK_PUBLISHER_ADDRESS: B160 = B160::from_limbs([0x8011, 0, 0]);

/// Helper to check if an address the bootloader
#[inline(always)]
pub fn is_bootloader(address: &B160) -> bool {
    address == &BOOTLOADER_FORMAL_ADDRESS
}
