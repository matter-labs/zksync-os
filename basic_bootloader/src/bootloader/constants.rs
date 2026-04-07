use evm_interpreter::ERGS_PER_GAS;
use ruint::aliases::{B160, U256};

pub const SPECIAL_ADDRESS_SPACE_BOUND: u64 = 0x010000;
pub const SPECIAL_ADDRESS_TO_WASM_DEPLOY: B160 = B160::from_limbs([0x9000, 0, 0]);

/// Bootloader's formal address for system-level operations
pub const BOOTLOADER_FORMAL_ADDRESS: B160 = B160::from_limbs([0x8001, 0, 0]);

pub const MAX_TX_LEN_BYTES: usize = 1 << 23;
pub const MAX_TX_LEN_WORDS: usize = MAX_TX_LEN_BYTES / core::mem::size_of::<u32>();

const _: () = const {
    assert!(MAX_TX_LEN_BYTES.is_multiple_of(core::mem::size_of::<usize>()));
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

/// Maximum value of gas that can be represented as ergs in an u64.
pub const MAX_BLOCK_GAS_LIMIT: u64 = u64::MAX / ERGS_PER_GAS;

// Just for EVM compatibility.
pub const L1_TX_INTRINSIC_L2_GAS: u64 = 21_000;

// Covers intrinsic L1 tx work not charged as tx-body computation.
//
//  - storing and hashing the L1 tx log:
//      EVENT_STORAGE_BASE_NATIVE_COST
//    + keccak256_native_cost(88)
//    + 2 * keccak256_native_cost(64)
//    = 6_000 + 20_000 + 40_000
//    = 66_000
//  - hashing tx hash into the rolling hash and linear hashers:
//      3 * keccak256_native_cost(64)
//    = 3 * 20_000
//    = 60_000
//  - coinbase transfer:
//      warm existing balance write
//    = WARM_STORAGE_READ_NATIVE_COST + WARM_STORAGE_WRITE_EXTRA_NATIVE_COST x 2 (to account for treasury)
//    = (4_000 + 1_000) x 2
//    = 10_000
//  - coinbase L2AssetTracker notification:
//      cold call into L2AssetTracker
//    + BASE_TOKEN_ASSET_ID read
//    + isAssetRegistered read
//    + assetMigrationNumber read
//    + L2BaseTokenZKOS.totalSupply() path
//    + L2_CHAIN_ASSET_HANDLER.migrationNumber() call
//    + assetMigrationNumber write
//    + SystemContext.currentSettlementLayerChainId() call
//    + interopInfo.totalSuccessfulDepositsFromL1 += amount
//    = 132_600
//    + 125_120
//    + 145_120
//    + 286_240
//    + 392_340
//    + 277_720
//    + 164_800
//    + 257_720
//    + 391_040
//    ~= 2_172_700
//  - refund transfer:
//      treasury cold existing write
//    + refund recipient cold new write
//    = 171_680 + 363_040
//    = 534_720
//  - refund L2AssetTracker notification:
//      warm-path estimate
//    = 32_000
//
// We use the cold-path cost for asset tracker first notification because
// first mint / call to L2AssetTracker can fail due to out-of-native
pub const L1_TX_INTRINSIC_NATIVE_COST: u64 = 2_875_420;

// Pubdata needed for the diff in balance as a result of
// the fee payment to the coinbase.
// We take a worst-case value of 32 byte for the key and 34 for
// the uncompressed update.
const COINBASE_BALANCE_INTRINSIC_PUBDATA: u64 = 32 + 34;

// Pubdata needed for the treasury balance diff caused by transfers
// from treasury. Use the same worst-case balance-diff estimate as
// for coinbase balance updates.
const TREASURY_BALANCE_INTRINSIC_PUBDATA: u64 = 32 + 34;

// Pubdata needed for the refund recipient balance diff in the worst case.
// As with the coinbase/treasury balance updates, price a 32-byte key and
// 34-byte uncompressed value update.
const REFUND_RECIPIENT_BALANCE_INTRINSIC_PUBDATA: u64 = 32 + 34;

// Pubdata produced by the L2AssetTracker.handleFinalizeBaseTokenBridgingOnL2
// call that the bootloader makes inside the L1 tx execution frame (value-mint
// notification). In the steady-state case (base token already registered,
// settled on L1), the contract performs a single SSTORE:
//   interopInfo[assetId].totalSuccessfulDepositsFromL1 += _amount
// Each storage diff is encoded as 32 bytes (derived key) + compressed value
// diff. The worst-case compressed value using the Add strategy with a
// 256-bit amount falls back to Nothing encoding = 33 bytes.
pub const ASSET_TRACKER_INTRINSIC_PUBDATA: u64 = 32 + 33;

// Needed to publish the L1 tx log, coinbase balance, treasury balance, refund
// recipient balance, and asset tracker state diff.
pub const L1_TX_INTRINSIC_PUBDATA: u64 = 88
    + COINBASE_BALANCE_INTRINSIC_PUBDATA
    + TREASURY_BALANCE_INTRINSIC_PUBDATA
    + REFUND_RECIPIENT_BALANCE_INTRINSIC_PUBDATA
    + ASSET_TRACKER_INTRINSIC_PUBDATA;

pub const L2_TX_INTRINSIC_GAS: u64 = 21_000;

/// Extra cost for deployment transactions.
pub const DEPLOYMENT_TX_EXTRA_INTRINSIC_GAS: u64 = 32_000;

/// Value taken from system-contracts, to adjust.
pub const L2_TX_INTRINSIC_PUBDATA: u64 = COINBASE_BALANCE_INTRINSIC_PUBDATA;

// Includes:
//  - Transferring fee to coinbase.
//  - Transferring the gas refund.
//  - Hashing of tx hash into rolling hash.
pub const L2_TX_INTRINSIC_NATIVE_COST: u64 = 30_000;

/// Cost to convert zero byte of calldata into "token"
pub const CALLDATA_ZERO_BYTE_TOKEN_FACTOR: u64 = 1;

/// Cost to convert non-zero byte of calldata into "token"
pub const CALLDATA_NON_ZERO_BYTE_TOKEN_FACTOR: u64 = 4;

/// Cost in gas per "token" of calldata
pub const CALLDATA_TOKEN_GAS_COST: u64 = 4;

/// EIP-7623 minimal "token" cost
pub const TOTAL_COST_FLOOR_PER_TOKEN: u64 = 10;

/// Computational cost of 7702 auth
pub const PER_AUTH_NATIVE_COST: u64 = 2000;

/// Computational cost of 2930 access list per address
pub const PER_ADDRESS_ACCESS_LIST_NATIVE_COST: u64 = 2000;

/// Computational cost of 2930 access list per slot
pub const PER_SLOT_ACCESS_LIST_NATIVE_COST: u64 = 2000;

/// EVM tester requires a high native_per_gas, but it hard-codes
/// low gas prices. We need to bypass the usual way to compute this
/// value. The value is so high because of modexp tests.
pub const TESTER_NATIVE_PER_GAS: u64 = 25_000;

// Default native price for L1->L2 transactions.
// TODO (EVM-1157): find a reasonable value for it.
pub const L1_TX_NATIVE_PRICE: U256 = U256::from_limbs([10, 0, 0, 0]);

// Upgrade, service and gateway mailbox transactions are expected to have ~72 million gas. We will use enough
// gas to ensure that multiplied by the 72 million they exceed the native computational limit.
pub const FREE_L1_TX_NATIVE_PER_GAS: u64 = 10000;
