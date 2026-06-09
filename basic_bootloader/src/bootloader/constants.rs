use basic_system::cost_constants::{
    blake2s_native_cost, ECRECOVER_NATIVE_COST, KECCAK256_CHUNK_SIZE, KECCAK256_ROUND_NATIVE_COST,
};
use basic_system::system_functions::keccak256::keccak256_native_cost_for_rounds_u64;
#[cfg(feature = "eip-2935")]
use basic_system::system_implementation::flat_storage_model::cost_constants::COLD_NEW_STORAGE_WRITE_EXTRA_NATIVE_COST;
use basic_system::system_implementation::flat_storage_model::cost_constants::{
    ACCOUNT_PERSIST_EXISTING_WRITE_NATIVE_COST, ACCOUNT_PERSIST_NEW_WRITE_NATIVE_COST,
    COLD_EXISTING_STORAGE_READ_NATIVE_COST, COLD_NEW_STORAGE_READ_NATIVE_COST,
    EVENT_STORAGE_BASE_NATIVE_COST, PREIMAGE_CACHE_GET_NATIVE_COST, PREIMAGE_CACHE_SET_NATIVE_COST,
    WARM_ACCOUNT_CACHE_ACCESS_NATIVE_COST, WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST,
    WARM_STORAGE_READ_NATIVE_COST,
};
use basic_system::system_implementation::flat_storage_model::AccountProperties;
use evm_interpreter::native_resource_constants::COPY_BYTE_NATIVE_COST;
use evm_interpreter::ERGS_PER_GAS;
use ruint::aliases::B160;

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

/// FRI proof verification cost charged per submitted statement hash.
/// Sized as an upper bound on the RISC-V `process_transaction` cycles for a
/// single 100-bit FRI proof (~16.5M effective).
pub const FRI_PROOF_INTRINSIC_NATIVE_COST_PER_PROOF: u64 = 17_000_000;
/// Per-statement intrinsic gas surcharge for `FriProofTx`.
pub const FRI_PROOF_TX_INTRINSIC_GAS: u64 = 100_000;
/// Current statement-hash version of FRI proof transactions.
pub const FRI_STATEMENT_HASH_VERSION: u8 = 1;

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

/// Fixed `native_per_gas` ratio used for all L1->L2 transactions
/// (including upgrade, service txs):
/// - high enough that computational part becomes negligible (with current
///   ratio, ~350 gas is enough to exceed computational native limit)
/// - low enough that native resources doesn't overflow `u64` for
///   any realistic L1 gas_limit.
pub const L1_TX_NATIVE_PER_GAS: u64 = 100_000_000;

// computational native consts
/// Constant part of l2 tx intrinsic computational native cost.
pub const L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_COST: u64 = ECRECOVER_NATIVE_COST + // signature verification
    NEW_COLD_ACCOUNT_READ_COST + // worst case sender account read (new on free-native chains)
    ACCOUNT_UPDATE_COST + // nonce update
    keccak256_native_cost_for_rounds_u64(3) * 2 + // keccak for signing and full hash, 2 rounds worst case tx size + 1 round precharge for dynamic parts
    ACCOUNT_UPDATE_COST + // balance change for fee prepayment
    ACCOUNT_UPDATE_COST * 2 + keccak256_native_cost_for_rounds_u64(1) + // post execution logic: transferring fee to coinbase, transferring the gas refund, hashing of tx hash into rolling hash
    ACCOUNT_PERSIST_NEW_NATIVE_COST + // sender persist (worst case: new on free-native chains)
    ACCOUNT_PERSIST_EXISTING_NATIVE_COST; // coinbase persist (operator ensures coinbase exists)

/// Service tx intrinsic computational native cost.
/// Service txs are not signed, so there is no ecrecover and only a single
/// (full-tx) keccak is performed.
pub const SERVICE_TX_INTRINSIC_COMPUTATIONAL_NATIVE_COST: u64 = NEW_COLD_ACCOUNT_READ_COST + // worst case sender (bootloader) cold read
    keccak256_native_cost_for_rounds_u64(2) + // keccak for full hash, 1 round worst case tx size + 1 round precharge for dynamic parts
    ACCOUNT_UPDATE_COST + // balance change for fee prepayment
    ACCOUNT_UPDATE_COST * 2 + keccak256_native_cost_for_rounds_u64(1); // coinbase + refund materializes, hashing of tx hash into rolling hash; no persist (gas_price=0, all balance updates are no-ops)

/// Service tx calldata byte intrinsic computational native cost.
pub const SERVICE_TX_INTRINSIC_COMPUTATIONAL_NATIVE_PER_CALLDATA_BYTE: u64 =
    DYNAMIC_PART_KECCAK_COMPUTATIONAL_NATIVE_PER_BYTE; // to cover full hash

/// Native computational cost to cover keccak256 hashing overhead for dynamic fields of the transaction per byte.
/// NOTE: this is approximate cost for hashing of 1 byte, but it shouldn't be used to estimate cost of one keccak call,
/// it doesn't include static keccak256 cost part and keccak256 cost depends on the number of rounds, not byte length.
/// So these things should be accounted separately: constant part of tx intrinsic cost includes keccak static part, and
/// we are precharging 1 keccak round in the constant part to cover worst case number of rounds.
/// Without extra round charge, fields can consume 136*n + 1 bytes in encoding, so cost will cover ~n rounds, but it should cover (n + 1) rounds of keccak.
const DYNAMIC_PART_KECCAK_COMPUTATIONAL_NATIVE_PER_BYTE: u64 =
    KECCAK256_ROUND_NATIVE_COST.div_ceil(KECCAK256_CHUNK_SIZE as u64);

/// L2 tx calldata byte intrinsic computational native cost.
pub const L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_PER_CALLDATA_BYTE: u64 =
    COPY_BYTE_NATIVE_COST + 2 * DYNAMIC_PART_KECCAK_COMPUTATIONAL_NATIVE_PER_BYTE; // to cover copying + signing hash + full hash

/// L2 tx access list account computational native cost.
pub const L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_ACCESS_LIST_PER_ADDRESS: u64 =
    PER_ADDRESS_ACCESS_LIST_NATIVE_COMPUTATIONAL_OVERHEAD + // computational overhead
    NEW_COLD_ACCOUNT_READ_COST + // worst case account read
    30 * DYNAMIC_PART_KECCAK_COMPUTATIONAL_NATIVE_PER_BYTE * 2; // keccak for signing + full hash, 30 - worst case contribution to rlp encoding (21 address, 9 keys list length encoding)

/// L2 tx access list storage slot computational native cost.
pub const L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_ACCESS_LIST_PER_STORAGE_KEY: u64 =
    PER_SLOT_ACCESS_LIST_NATIVE_COMPUTATIONAL_OVERHEAD + // computational overhead
    WARM_STORAGE_READ_NATIVE_COST + // materialize_element always charges warm read
    COLD_NEW_STORAGE_READ_NATIVE_COST + // worst case cold read extra
    33 * DYNAMIC_PART_KECCAK_COMPUTATIONAL_NATIVE_PER_BYTE * 2; // keccak for signing + full hash, 33 contribution to rlp encoding length

/// L2 tx authorization computational native cost.
pub const L2_TX_INTRINSIC_COMPUTATIONAL_NATIVE_PER_AUTHORIZATION: u64 =
    PER_AUTH_NATIVE_COMPUTATIONAL_OVERHEAD + // computational overhead
    keccak256_native_cost_for_rounds_u64(1) + // auth message keccak cost (1 round)
    ECRECOVER_NATIVE_COST + // signature verification
    NEW_COLD_ACCOUNT_READ_COST + // worst case account read
    ACCOUNT_UPDATE_COST + // nonce update
    ACCOUNT_UPDATE_COST + PREIMAGE_CACHE_SET_NATIVE_COST + keccak256_native_cost_for_rounds_u64(1) /*bytecode hashing */ + blake2s_native_cost(24) /* blake2s padded bytecode */ + // delegation write
    133 * DYNAMIC_PART_KECCAK_COMPUTATIONAL_NATIVE_PER_BYTE * 2 + // keccak for tx signing + full hash, 133 - worst case contribution to rlp encoding (33 chain_id, 21 address, 9 nonce, 1 y_parity, 33 r, 33 s, 3 list overhead)
    ACCOUNT_PERSIST_NEW_NATIVE_COST; // delegatee persist (worst case: new account)

/// Native computational overhead of 7702 auth.
pub const PER_AUTH_NATIVE_COMPUTATIONAL_OVERHEAD: u64 = 2000;

/// Native computational overhead of 2930 access list per address.
pub const PER_ADDRESS_ACCESS_LIST_NATIVE_COMPUTATIONAL_OVERHEAD: u64 = 2000;

/// Native computational overhead 2930 access list per slot.
pub const PER_SLOT_ACCESS_LIST_NATIVE_COMPUTATIONAL_OVERHEAD: u64 = 2000;

/// Account read native computational cost in the worst case - cold, new(not present in the tree).
pub const NEW_COLD_ACCOUNT_READ_COST: u64 = WARM_ACCOUNT_CACHE_ACCESS_NATIVE_COST
    + WARM_STORAGE_READ_NATIVE_COST
    + COLD_NEW_STORAGE_READ_NATIVE_COST;

/// Account update native computational cost.
pub const ACCOUNT_UPDATE_COST: u64 =
    WARM_ACCOUNT_CACHE_ACCESS_NATIVE_COST + WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST;

/// Decommitment cost for non-empty account properties.
const ACCOUNT_DECOMMITMENT_NATIVE_COST: u64 =
    PREIMAGE_CACHE_GET_NATIVE_COST + blake2s_native_cost(AccountProperties::ENCODED_SIZE);

/// Cold balance write cost for an existing non-empty account (e.g. treasury).
/// Covers: materialize (cold access + decommitment) + cache write.
const COLD_EXISTING_BALANCE_WRITE_NATIVE_COST: u64 = WARM_ACCOUNT_CACHE_ACCESS_NATIVE_COST
    + WARM_STORAGE_READ_NATIVE_COST
    + COLD_EXISTING_STORAGE_READ_NATIVE_COST
    + ACCOUNT_DECOMMITMENT_NATIVE_COST
    + WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST;

/// Cold balance write cost for a new empty account (e.g. first-time refund recipient).
/// Covers: materialize (cold access, no decommitment) + cache write.
const COLD_NEW_BALANCE_WRITE_NATIVE_COST: u64 = WARM_ACCOUNT_CACHE_ACCESS_NATIVE_COST
    + WARM_STORAGE_READ_NATIVE_COST
    + COLD_NEW_STORAGE_READ_NATIVE_COST
    + WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST;

/// Preimage hash cost for account properties.
const ACCOUNT_PROPERTIES_PREIMAGE_HASH_NATIVE_COST: u64 =
    blake2s_native_cost(AccountProperties::ENCODED_SIZE);

/// Combined persist cost for existing accounts (0x8003 write + preimage hash).
const ACCOUNT_PERSIST_EXISTING_NATIVE_COST: u64 =
    ACCOUNT_PERSIST_EXISTING_WRITE_NATIVE_COST + ACCOUNT_PROPERTIES_PREIMAGE_HASH_NATIVE_COST;

/// Combined persist cost for new accounts (0x8003 write + preimage hash).
const ACCOUNT_PERSIST_NEW_NATIVE_COST: u64 =
    ACCOUNT_PERSIST_NEW_WRITE_NATIVE_COST + ACCOUNT_PROPERTIES_PREIMAGE_HASH_NATIVE_COST;

/// Constant part of l1 tx intrinsic computational native cost.
// Covers intrinsic L1 tx work not charged as tx-body computation.
//
// Hardcoded component: L2AssetTracker contract execution native cost.
// Cold path: first call in a tx, contract storage is cold.
// Warm path: subsequent calls in the same tx, contract storage is warm.
//
// To re-measure: run an L1 tx test without no_print and grep for
// "L1 notify_l2_asset_tracker native" in the output. The first value
// per tx is cold, subsequent values are warm.
const L1_TX_ASSET_TRACKER_COLD_NOTIFICATION_NATIVE_COST: u64 = 2_043_860;
const L1_TX_ASSET_TRACKER_WARM_NOTIFICATION_NATIVE_COST: u64 = 212_100;

pub const L1_TX_INTRINSIC_NATIVE_COST: u64 =
    // Pre-budgeted (not charged against inf_resources, but reserved upfront):
    EVENT_STORAGE_BASE_NATIVE_COST + 3 * keccak256_native_cost_for_rounds_u64(1) + // L1 tx log: storage + keccak(88) + 2 * keccak(64)
    3 * keccak256_native_cost_for_rounds_u64(1) + // hashing tx hash into rolling hash and linear hashers
    // Coinbase mint (notify AssetTracker + transfer treasury→coinbase):
    L1_TX_ASSET_TRACKER_COLD_NOTIFICATION_NATIVE_COST +
    COLD_EXISTING_BALANCE_WRITE_NATIVE_COST + // treasury debit (cold — may be first access if deposit=0)
    ACCOUNT_UPDATE_COST + // coinbase credit (warm — pre-warmed by tx_loop)
    // Refund mint (notify AssetTracker + transfer treasury→recipient):
    L1_TX_ASSET_TRACKER_WARM_NOTIFICATION_NATIVE_COST +
    ACCOUNT_UPDATE_COST + // treasury debit (warm — accessed in coinbase mint)
    COLD_NEW_BALANCE_WRITE_NATIVE_COST + // recipient credit (cold new — worst case first-time depositor)
    2 * ACCOUNT_PERSIST_EXISTING_NATIVE_COST + // coinbase + treasury persist (operator ensures these exist)
    ACCOUNT_PERSIST_NEW_NATIVE_COST; // refund recipient persist (may be new — set by L1 sender)

/// L1 tx calldata byte intrinsic computational native cost.
pub const L1_TX_INTRINSIC_COMPUTATIONAL_NATIVE_PER_CALLDATA_BYTE: u64 = COPY_BYTE_NATIVE_COST;

/// Worst-case pubdata for tx sender balance change
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

/// Native cost of the EIP-2935 pre-tx-loop work: a cold read of the
/// `HISTORY_STORAGE_ADDRESS` account properties followed by a cold write of
/// the history slot.
///
/// We assume `HISTORY_STORAGE_ADDRESS` exists in the tree — it's deployed via
/// genesis or an earlier block and is only absent at block number 0, which
/// cannot run EIP-2935 anyway. That lets us use the cold-EXISTING path for
/// the account read rather than the NEW worst case (which
/// `NEW_COLD_ACCOUNT_READ_COST` bundles for other callers). Because the
/// account is non-empty, the cold read also pays a decommitment of the
/// account properties preimage.
///
/// The slot write keeps the cold-NEW worst case so the reserve holds during
/// the first 8191-block cycle when slots are freshly touched. This matches
/// what the storage layer actually charges in `materialize_element`
/// (warm read + cold-new read-extra) followed by `charge_storage_write_extra`
/// (cold-new write-extra).
#[cfg(feature = "eip-2935")]
const EIP_2935_INTRINSIC_NATIVE: u64 =
    // Cold read of HISTORY_STORAGE_ADDRESS account properties (assume exists)
    WARM_ACCOUNT_CACHE_ACCESS_NATIVE_COST
        + WARM_STORAGE_READ_NATIVE_COST
        + COLD_EXISTING_STORAGE_READ_NATIVE_COST
        + PREIMAGE_CACHE_GET_NATIVE_COST
        + blake2s_native_cost(AccountProperties::ENCODED_SIZE)
        // Cold write of the history slot (worst case: new slot)
        + WARM_STORAGE_READ_NATIVE_COST
        + COLD_NEW_STORAGE_READ_NATIVE_COST
        + COLD_NEW_STORAGE_WRITE_EXTRA_NATIVE_COST;
#[cfg(not(feature = "eip-2935"))]
const EIP_2935_INTRINSIC_NATIVE: u64 = 0;

/// Pubdata cost of the EIP-2935 history-slot write. One state diff:
/// 32-byte derived key + worst-case 33-byte compressed value (the parent-hash
/// value does not compress, so the `Nothing` strategy applies — same shape
/// as `ASSET_TRACKER_INTRINSIC_PUBDATA`).
#[cfg(feature = "eip-2935")]
const EIP_2935_INTRINSIC_PUBDATA: u64 = 32 + 33;
#[cfg(not(feature = "eip-2935"))]
const EIP_2935_INTRINSIC_PUBDATA: u64 = 0;

/// Intrinsic per-block pubdata overhead, applied to block-limit enforcement
/// from block start. Accounts for:
/// - the fixed envelope written by `write_pubdata`: 1 byte
///   (PUBDATA_ENCODING_VERSION) + 32 bytes (block hash) + 8 bytes (timestamp),
/// - the EIP-2935 history-slot diff when the feature is enabled.
pub const BLOCK_INTRINSIC_PUBDATA_BYTES: u64 = 1 + 32 + 8 + EIP_2935_INTRINSIC_PUBDATA;

/// Intrinsic per-block native overhead, applied to block-limit enforcement
/// from block start. Covers fixed pre-tx-loop system work — currently just
/// the EIP-2935 historical block hash write when the feature is enabled.
pub const BLOCK_INTRINSIC_NATIVE: u64 = EIP_2935_INTRINSIC_NATIVE;
