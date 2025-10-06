use evm_interpreter::ERGS_PER_GAS;
use ruint::aliases::{B160, U256};

pub const SPECIAL_ADDRESS_SPACE_BOUND: u64 = 0x010000;
pub const SPECIAL_ADDRESS_TO_WASM_DEPLOY: B160 = B160::from_limbs([0x9000, 0, 0]);

/// We want to have a buffer before the transaction.
pub const TX_OFFSET: usize = (6 + 33) * 32 + 4;
pub const MAX_TX_LEN_BYTES: usize = 1 << 23;
pub const TX_OFFSET_WORDS: usize = TX_OFFSET / core::mem::size_of::<u32>();
pub const MAX_TX_LEN_WORDS: usize = MAX_TX_LEN_BYTES / core::mem::size_of::<u32>();

const _: () = const {
    assert!(TX_OFFSET % core::mem::size_of::<u32>() == 0);
    assert!(MAX_TX_LEN_BYTES % core::mem::size_of::<usize>() == 0);
};

// 1024 for EVM equivalence
// We actually use 1025 one more because we fail when pushing to the stack,
// while geth checks if the stack depth limit was passed later on in
// the execution.
pub const MAX_CALLSTACK_DEPTH: usize = 1025;

/// Offset for the beginning of the tx data as passed in calldata.
/// The value (96) is the sum of 32 bytes for the tx_hash,
/// 32 for the suggested_signed_hash and 32 for the offset itself.
pub const TX_CALLDATA_OFFSET: usize = 0x60;

/// Maximum value of gas that can be represented as ergs in a u64.
pub const MAX_BLOCK_GAS_LIMIT: u64 = u64::MAX / ERGS_PER_GAS;

// Just for EVM compatibility.
pub const L1_TX_INTRINSIC_L2_GAS: u64 = 21_000;

// Includes:
//  - Storing and hashing the l1 tx log.
//  - Transferring fee to coinbase.
//  - Hashing of tx hash into rolling hash.
//  - Adding tx hash into l1 tx linear hasher
pub const L1_TX_INTRINSIC_NATIVE_COST: u64 = 130_000;

// Needed to publish the l1 tx log.
pub const L1_TX_INTRINSIC_PUBDATA: u64 = 88;

/// Does not include signature verification.
pub const L2_TX_INTRINSIC_GAS: u64 = 18_000;

/// Extra cost for deployment transactions.
pub const DEPLOYMENT_TX_EXTRA_INTRINSIC_GAS: u64 = 32_000;

/// Value taken from system-contracts, to adjust.
pub const L2_TX_INTRINSIC_PUBDATA: u64 = 0;

// Includes:
//  - Transferring fee to coinbase.
//  - Transferring the gas refund.
//  - Hashing of tx hash into rolling hash.
pub const L2_TX_INTRINSIC_NATIVE_COST: u64 = 30_000;

/// Cost in gas to store one zero byte of calldata
pub const CALLDATA_ZERO_BYTE_GAS_COST: u64 = 4;

/// Cost in gas to store one non-zero byte of calldata
pub const CALLDATA_NON_ZERO_BYTE_GAS_COST: u64 = 16;

/// EVM tester requires a high native_per_gas, but it hard-codes
/// low gas prices. We need to bypass the usual way to compute this
/// value. The value is so high because of modexp tests.
pub const TESTER_NATIVE_PER_GAS: u64 = 25_000;

/// native_per_gas value to use for simulation. Should be in line with
/// the value of basefee / native_price provided by operator.
/// Needed because simulation is done with basefee = 0.
pub const SIMULATION_NATIVE_PER_GAS: U256 = U256::from_limbs([100, 0, 0, 0]);

// Default native price for L1->L2 transactions.
// TODO (EVM-1157): find a reasonable value for it.
pub const L1_TX_NATIVE_PRICE: U256 = U256::from_limbs([10, 0, 0, 0]);

// Upgrade transactions are expected to have ~72 million gas. We will use enough
// gas to ensure that multiplied by the 72 million they exceed the native computational limit.
pub const UPGRADE_TX_NATIVE_PER_GAS: U256 = U256::from_limbs([10000, 0, 0, 0]);
