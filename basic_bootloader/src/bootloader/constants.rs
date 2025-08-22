use evm_interpreter::ERGS_PER_GAS;
use ruint::aliases::{B160, U256};

pub const SPECIAL_ADDRESS_SPACE_BOUND: u64 = 0x010000;
pub const SPECIAL_ADDRESS_TO_WASM_DEPLOY: B160 = B160::from_limbs([0x9000, 0, 0]);

/// We want to have a buffer before the transaction, it may be useful to encode calls calldata for account absatraction.
/// The size is calculated for `postOp` operation calldata, the biggest in zkSync Era account abstranction model.
pub const TX_OFFSET: usize = (6 + 33) * 32 + 4;
pub const MAX_TX_LEN_BYTES: usize = 1 << 23;
pub const TX_OFFSET_WORDS: usize = TX_OFFSET / core::mem::size_of::<u32>();
pub const MAX_TX_LEN_WORDS: usize = MAX_TX_LEN_BYTES / core::mem::size_of::<u32>();

const _: () = const {
    assert!(TX_OFFSET % core::mem::size_of::<u32>() == 0);
    assert!(MAX_TX_LEN_BYTES % core::mem::size_of::<usize>() == 0);
};

pub const MAX_PAYMASTER_CONTEXT_LEN_BYTES: usize = 1024 + 32;
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

// TODO: compute using solidity abi
// 202bcce7
pub const VALIDATE_SELECTOR: &[u8] = &[0x20, 0x2b, 0xcc, 0xe7];

// 0xdf9c1589
pub const EXECUTE_SELECTOR: &[u8] = &[0xdf, 0x9c, 0x15, 0x89];

// 0xa28c1aee
pub const PREPARE_FOR_PAYMASTER_SELECTOR: &[u8] = &[0xa2, 0x8c, 0x1a, 0xee];

// 0xe2f318e3
pub const PAY_FOR_TRANSACTION_SELECTOR: &[u8] = &[0xe2, 0xf3, 0x18, 0xe3];

// 0x949431dc
pub const PAYMASTER_APPROVAL_BASED_SELECTOR: &[u8] = &[0x94, 0x94, 0x31, 0xdc];

// 0x8c5a3445
pub const PAYMASTER_GENERAL_SELECTOR: &[u8] = &[0x8c, 0x5a, 0x34, 0x45];

// 0x038a24bc
pub const PAYMASTER_VALIDATE_AND_PAY_SELECTOR: &[u8] = &[0x03, 0x8a, 0x24, 0xbc];

// 0x817b17f0
pub const PAYMASTER_POST_TRANSACTION_SELECTOR: &[u8] = &[0x81, 0x7b, 0x17, 0xf0];

// 0xdd62ed3e
pub const ERC20_ALLOWANCE_SELECTOR: &[u8] = &[0xdd, 0x62, 0xed, 0x3e];

// 0x095ea7b3
pub const ERC20_APPROVE_SELECTOR: &[u8] = &[0x09, 0x5e, 0xa7, 0xb3];

// Value taken from system-contracts, to adjust.
pub const L1_TX_INTRINSIC_L2_GAS: u64 = 11000;

// Includes storing the l1 tx log.
pub const L1_TX_INTRINSIC_NATIVE_COST: u64 = 10_000;

// Value taken from system-contracts, to adjust.
pub const L1_TX_INTRINSIC_PUBDATA: u64 = 88;

/// Does not include signature verification.
pub const L2_TX_INTRINSIC_GAS: u64 = 18_000;

/// Extra cost for deployment transactions.
pub const DEPLOYMENT_TX_EXTRA_INTRINSIC_GAS: u64 = 32_000;

/// Value taken from system-contracts, to adjust.
pub const L2_TX_INTRINSIC_PUBDATA: u64 = 0;

// To be adjusted
pub const L2_TX_INTRINSIC_NATIVE_COST: u64 = 4_000;

/// Cost in gas to store one zero byte of calldata
pub const CALLDATA_ZERO_BYTE_GAS_COST: u64 = 4;

/// Cost in gas to store one non-zero byte of calldata
pub const CALLDATA_NON_ZERO_BYTE_GAS_COST: u64 = 16;

/// Default value of gasPerPubdata for non EIP-712 txs.
pub const DEFAULT_GAS_PER_PUBDATA: U256 = U256::from_limbs([1, 0, 0, 0]);

/// EVM tester requires a high native_per_gas, but it hard-codes
/// low gas prices. We need to bypass the usual way to compute this
/// value. The value is so high because of modexp tests.
pub const TESTER_NATIVE_PER_GAS: u64 = 25_000;

/// native_per_gas value to use for simulation. Should be in line with
/// the value of basefee / native_price provided by operator.
/// Needed because simulation is done with basefee = 0.
pub const SIMULATION_NATIVE_PER_GAS: U256 = U256::from_limbs([100, 0, 0, 0]);

// Default native price for L1->L2 transactions.
// TODO: find a reasonable value for it.
pub const L1_TX_NATIVE_PRICE: U256 = U256::from_limbs([10, 0, 0, 0]);

// Upgrade transactions are expected to have ~72 million gas. We will use enough
// gas to ensure that multiplied by the 72 million they exceed the native computational limit.
pub const UPGRADE_TX_NATIVE_PER_GAS: U256 = U256::from_limbs([10000, 0, 0, 0]);
