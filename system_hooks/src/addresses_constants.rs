use ruint::aliases::B160;

// EVM precompiles addresses

// Contract Deployer system contract
pub const CONTRACT_DEPLOYER_ADDRESS_LOW: u16 = 0x8006;
pub const CONTRACT_DEPLOYER_ADDRESS: B160 =
    B160::from_limbs([CONTRACT_DEPLOYER_ADDRESS_LOW as u64, 0, 0]);

// L1 messenger system contract needed for all envs
pub const L1_MESSENGER_ADDRESS_LOW: u16 = 0x8008;
pub const L1_MESSENGER_ADDRESS: B160 = B160::from_limbs([L1_MESSENGER_ADDRESS_LOW as u64, 0, 0]);

// L1 messenger system hook needed for all envs
pub const L1_MESSENGER_ADDRESS_HOOK_LOW: u16 = 0x7001;
pub const L1_MESSENGER_ADDRESS_HOOK: B160 =
    B160::from_limbs([L1_MESSENGER_ADDRESS_HOOK_LOW as u64, 0, 0]);

// system hook to set bytecode on address, needed for protocol upgrades
pub const SET_BYTECODE_ON_ADDRESS_HOOK_LOW: u16 = 0x7002;
pub const SET_BYTECODE_ON_ADDRESS_HOOK: B160 =
    B160::from_limbs([SET_BYTECODE_ON_ADDRESS_HOOK_LOW as u64, 0, 0]);

// L2 base token system contract needed for all envs (base token withdrawals)
pub const L2_BASE_TOKEN_ADDRESS_LOW: u16 = 0x800a;
pub const L2_BASE_TOKEN_ADDRESS: B160 = B160::from_limbs([L2_BASE_TOKEN_ADDRESS_LOW as u64, 0, 0]);

// Base token mint system hook - allows L2 base token contract to mint tokens
// Only callable by L2_BASE_TOKEN_ADDRESS (0x800a) with 32-byte calldata containing mint amount
pub const MINT_HOOK_ADDRESS_LOW: u16 = 0x7100;
pub const MINT_HOOK_ADDRESS: B160 = B160::from_limbs([MINT_HOOK_ADDRESS_LOW as u64, 0, 0]);

// L2 interop root storage system contract
pub const L2_INTEROP_ROOT_STORAGE_ADDRESS_LOW: u32 = 0x10008;
pub const L2_INTEROP_ROOT_STORAGE_ADDRESS: B160 =
    B160::from_limbs([L2_INTEROP_ROOT_STORAGE_ADDRESS_LOW as u64, 0, 0]);

// ERA VM system contracts (in fact we need implement only the methods that should be available for user contracts)
// TODO: may be better to implement as ifs inside EraVM EE
pub const ACCOUNT_CODE_STORAGE_STORAGE_ADDRESS: B160 = B160::from_limbs([0x8002, 0, 0]);
pub const KNOWN_CODE_STORAGE_ADDRESS: B160 = B160::from_limbs([0x8004, 0, 0]);
pub const IMMUTABLE_SIMULATOR_ADDRESS: B160 = B160::from_limbs([0x8005, 0, 0]);
// TODO: is a contract?
pub const FORCE_DEPLOYER_ADDRESS: B160 = B160::from_limbs([0x8007, 0, 0]);
pub const MSG_VALUE_SIMULATOR_ADDRESS: B160 = B160::from_limbs([0x8009, 0, 0]);
pub const BASE_TOKEN_ADDRESS: B160 = B160::from_limbs([0x800a, 0, 0]);

pub const SYSTEM_CONTEXT_ADDRESS_LOW: u32 = 0x800b;
pub const SYSTEM_CONTEXT_ADDRESS: B160 =
    B160::from_limbs([SYSTEM_CONTEXT_ADDRESS_LOW as u64, 0, 0]);
// TODO: bootloader utilities is no longer needed
pub const BOOTLOADER_UTILITIES_ADDRESS: B160 = B160::from_limbs([0x800c, 0, 0]);
pub const EVENT_WRITER_ADDRESS: B160 = B160::from_limbs([0x800d, 0, 0]);
pub const COMPRESSOR_ADDRESS: B160 = B160::from_limbs([0x800e, 0, 0]);
pub const COMPLEX_UPGRADER_ADDRESS: B160 = B160::from_limbs([0x800f, 0, 0]);
pub const KECCAK_SYSTEM_CONTRACT_ADDRESS: B160 = B160::from_limbs([0x8010, 0, 0]);
pub const PUBDATA_CHUNK_PUBLISHER_ADDRESS: B160 = B160::from_limbs([0x8011, 0, 0]);
