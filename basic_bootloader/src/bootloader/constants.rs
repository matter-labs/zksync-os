use basic_system::cost_constants::{
    ECRECOVER_NATIVE_COST, KECCAK256_CHUNK_SIZE, KECCAK256_ROUND_NATIVE_COST,
};
use basic_system::system_functions::keccak256::keccak256_native_cost_for_rounds_u64;
use basic_system::system_implementation::flat_storage_model::cost_constants::{
    COLD_NEW_STORAGE_READ_NATIVE_COST, PREIMAGE_CACHE_SET_NATIVE_COST,
    WARM_ACCOUNT_CACHE_ACCESS_NATIVE_COST, WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST,
    WARM_STORAGE_READ_NATIVE_COST,
};
use evm_interpreter::native_resource_constants::COPY_BYTE_NATIVE_COST;
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

/// Transaction intrinsic gas cost.
pub const TX_INTRINSIC_GAS: u64 = 21_000;

/// Extra cost for deployment transactions.
pub const DEPLOYMENT_TX_EXTRA_INTRINSIC_GAS: u64 = 32_000;

/// Cost to convert zero byte of calldata into "token"
pub const CALLDATA_ZERO_BYTE_TOKEN_FACTOR: u64 = 1;

/// Cost to convert non-zero byte of calldata into "token"
pub const CALLDATA_NON_ZERO_BYTE_TOKEN_FACTOR: u64 = 4;

/// Cost in gas per "token" of calldata
pub const CALLDATA_TOKEN_GAS_COST: u64 = 4;

/// EIP-7623 minimal "token" cost
pub const TOTAL_COST_FLOOR_PER_TOKEN: u64 = 10;

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

// computational native consts
/// Constant part of l2 tx intrinsic computational native cost.
pub const L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_COST: u64 = ECRECOVER_NATIVE_COST + // signature verification
    WARM_ACCOUNT_CACHE_ACCESS_NATIVE_COST + WARM_STORAGE_READ_NATIVE_COST + COLD_NEW_STORAGE_READ_NATIVE_COST + // worst case account read
    WARM_ACCOUNT_CACHE_ACCESS_NATIVE_COST + WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST + // nonce update
    keccak256_native_cost_for_rounds_u64(3) * 2 + // keccak for signing and full hash, 2 rounds worst case tx size + 1 round precharge for dynamic parts
    WARM_ACCOUNT_CACHE_ACCESS_NATIVE_COST + WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST + // balance change for fee prepayment
    (WARM_ACCOUNT_CACHE_ACCESS_NATIVE_COST + WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST) * 2 + keccak256_native_cost_for_rounds_u64(1); // post execution logic: transferring fee to coinbase, transferring the gas refund, hashing of tx hash into rolling hash

/// Service tx intrinsic computational native cost.
pub const SERVICE_TX_INTRINSIC_COMPUTATIONAL_NATIVE_COST: u64 =
    WARM_ACCOUNT_CACHE_ACCESS_NATIVE_COST + WARM_STORAGE_READ_NATIVE_COST + COLD_NEW_STORAGE_READ_NATIVE_COST + // worst case account read
    keccak256_native_cost_for_rounds_u64(2) + // keccak for full hash, 1 rounds worst case tx size + 1 round precharge for dynamic parts
    WARM_ACCOUNT_CACHE_ACCESS_NATIVE_COST + WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST + // balance change for fee prepayment
    (WARM_ACCOUNT_CACHE_ACCESS_NATIVE_COST + WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST) * 2 + keccak256_native_cost_for_rounds_u64(1); // post execution logic: transferring fee to coinbase, transferring the gas refund, hashing of tx hash into rolling hash

/// L2 tx calldata byte intrinsic computational native cost.
pub const SERVICE_TX_INTRINSIC_COMPUTATIONAL_NATIVE_PER_CALLDATA_BYTE: u64 =
    DYNAMIC_PART_KECCAK_COMPUTATIONAL_NATIVE_PER_BYTE; // to cover full hash

/// Native computational cost to cover keccak256 hashing overhead for dynamic fields of the transaction per byte.
/// NOTE: we are precharging 1 keccak round in the constant part since dynamic part, can consume 136*n + 1 bytes in encoding, so it will pay for ~n rounds, but consume (n + 1) rounds of keccak
const DYNAMIC_PART_KECCAK_COMPUTATIONAL_NATIVE_PER_BYTE: u64 =
    KECCAK256_ROUND_NATIVE_COST.div_ceil(KECCAK256_CHUNK_SIZE as u64);

/// L2 tx calldata byte intrinsic computational native cost.
pub const L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_PER_CALLDATA_BYTE: u64 =
    COPY_BYTE_NATIVE_COST + 2 * DYNAMIC_PART_KECCAK_COMPUTATIONAL_NATIVE_PER_BYTE; // to cover copying + signing hash + full hash

/// L2 tx access list account computational native cost.
pub const L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_ACCESS_LIST_PER_ADDRESS: u64 =
    PER_ADDRESS_ACCESS_LIST_NATIVE_COMPUTATIONAL_OVERHEAD + // computational overhead
    WARM_ACCOUNT_CACHE_ACCESS_NATIVE_COST + WARM_STORAGE_READ_NATIVE_COST + COLD_NEW_STORAGE_READ_NATIVE_COST + // worst case account read
    30 * DYNAMIC_PART_KECCAK_COMPUTATIONAL_NATIVE_PER_BYTE * 2; // keccak for signing + full hash, 30 - worst case contribution to rlp encoding (21 address, 9 keys list length encoding)

/// L2 tx access list storage slot computational native cost.
pub const L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_ACCESS_LIST_PER_STORAGE_KEY: u64 =
    PER_SLOT_ACCESS_LIST_NATIVE_COMPUTATIONAL_OVERHEAD + // computational overhead
    COLD_NEW_STORAGE_READ_NATIVE_COST + // worst case storage slot read
    33 * DYNAMIC_PART_KECCAK_COMPUTATIONAL_NATIVE_PER_BYTE * 2; // keccak for signing + full hash, 33 contribution to rlp encoding length

/// L2 tx authorization computational native cost.
pub const L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_PER_AUTHORIZATION: u64 =
    PER_AUTH_NATIVE_COMPUTATIONAL_OVERHEAD + // computational overhead
    keccak256_native_cost_for_rounds_u64(1) + // auth message keccak cost (1 round)
    ECRECOVER_NATIVE_COST + // signature verification
    WARM_ACCOUNT_CACHE_ACCESS_NATIVE_COST + WARM_STORAGE_READ_NATIVE_COST + COLD_NEW_STORAGE_READ_NATIVE_COST + // worst case account read
    WARM_ACCOUNT_CACHE_ACCESS_NATIVE_COST + WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST + // nonce update
    WARM_ACCOUNT_CACHE_ACCESS_NATIVE_COST + WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST + PREIMAGE_CACHE_SET_NATIVE_COST + keccak256_native_cost_for_rounds_u64(1) /*bytecode hashing */ + 1140 /* 1 round blake2s padded bytecode */ + // delegation write
    133 * DYNAMIC_PART_KECCAK_COMPUTATIONAL_NATIVE_PER_BYTE * 2; // keccak for tx signing + full hash, 133 - worst case contribution to rlp encoding (33 chain_id, 21 address, 9 nonce, 1 y_parity, 33 r, 33 s, 3 list overhead)

/// Native computational overhead of 7702 auth.
pub const PER_AUTH_NATIVE_COMPUTATIONAL_OVERHEAD: u64 = 2000;

/// Native computational overhead of 2930 access list per address.
pub const PER_ADDRESS_ACCESS_LIST_NATIVE_COMPUTATIONAL_OVERHEAD: u64 = 2000;

/// Native computational overhead 2930 access list per slot.
pub const PER_SLOT_ACCESS_LIST_NATIVE_COMPUTATIONAL_OVERHEAD: u64 = 2000;

/// Constant part of l1 tx intrinsic computational native cost.
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

/// L1 tx calldata byte intrinsic computational native cost.
pub const L1_TX_INTRINSIC_COMPUTATIONAL_NATIVE_PER_CALLDATA_BYTE: u64 = COPY_BYTE_NATIVE_COST;

/// worse case pubdata for tx sender balance change
const SENDER_BALANCE_INTRINSIC_PUBDATA: u64 = 32 /*key*/ + 1 /*account metadata*/ + 2 /*nonce increase*/ + 33/*worst case balance*/;

/// Constant part of l2 tx intrinsic pubdata.
pub const L2_TX_INTRINSIC_PUBDATA: u64 =
    SENDER_BALANCE_INTRINSIC_PUBDATA + COINBASE_BALANCE_INTRINSIC_PUBDATA;

/// L2 tx authorization intrinsic pubdata.
pub const L2_TX_INTRINSIC_PUBDATA_PER_AUTHORIZATION: u64 = // Full diff compression:
    32 + // key
    1 + // account metadata
    8 + // versioning data
    2 + // nonce
    1 + // balance
    4 + // unpadded code length
    4 + // artifacts length
    24 + // padded bytecode
    4; // observable length

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
