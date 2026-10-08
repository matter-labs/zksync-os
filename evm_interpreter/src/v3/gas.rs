//! Gas constants of Glamsterdam: the repriced state accesses and writes of EIP-8038, the
//! state gas of EIP-8037 and the transaction costs of EIP-2780, EIP-7976 and EIP-7981.
//! The unchanged opcode costs stay in [`crate::gas_constants`].

// --- access (EIP-8038) ------------------------------------------------------------------

/// Reading state that was read before in the transaction
pub const WARM_ACCESS: u32 = 100;
/// The first read of an account in the transaction
pub const COLD_ACCOUNT_ACCESS: u32 = 3000;
/// The first read of a storage slot in the transaction
pub const COLD_STORAGE_ACCESS: u32 = 2100;

// --- writes (EIP-8038) ------------------------------------------------------------------

/// Changing a storage slot away from its value at the start of the transaction
pub const STORAGE_WRITE: u32 = 10000;
/// Changing an account (a value transfer, a delegation)
pub const ACCOUNT_WRITE: u32 = 9000;
/// The stipend of a value call
pub const CALL_STIPEND: u32 = 2300;
/// A value call: the recipient's write plus the stipend
pub const CALL_VALUE: u32 = ACCOUNT_WRITE + CALL_STIPEND;
/// Creating an account by a deployment: its write and its access
pub const CREATE_ACCESS: u32 = ACCOUNT_WRITE + COLD_ACCOUNT_ACCESS;
/// Per word of init code (EIP-3860)
pub const CODE_INIT_PER_WORD: u32 = 2;
/// Hashing a word of deployed code
pub const KECCAK256_PER_WORD: u32 = 6;
/// The base cost of `SELFDESTRUCT`
pub const SELFDESTRUCT_BASE: u32 = 5000;
/// Refund of clearing a slot that was set at the start of the transaction
pub const REFUND_STORAGE_CLEAR: u32 = (STORAGE_WRITE + COLD_STORAGE_ACCESS) * 4800 / 5000;

// --- state gas (EIP-8037) ---------------------------------------------------------------

/// State gas per byte of state created
pub const COST_PER_STATE_BYTE: u32 = 1530;
pub const STATE_BYTES_PER_NEW_ACCOUNT: u32 = 120;
pub const STATE_BYTES_PER_STORAGE_SET: u32 = 64;
pub const STATE_BYTES_PER_AUTH_BASE: u32 = 23;
/// Setting a storage slot that was zero at the start of the transaction
pub const STORAGE_SET: u32 = STATE_BYTES_PER_STORAGE_SET * COST_PER_STATE_BYTE;
/// Creating an account
pub const NEW_ACCOUNT: u32 = STATE_BYTES_PER_NEW_ACCOUNT * COST_PER_STATE_BYTE;
/// Writing a delegation indicator to an account that had none
pub const AUTH_BASE: u32 = STATE_BYTES_PER_AUTH_BASE * COST_PER_STATE_BYTE;

// --- transactions (EIP-2780, EIP-7976, EIP-7981, EIP-7825, EIP-8037) --------------------

/// The sender's part of a transaction: signature recovery, its account's access and write
pub const TX_BASE: u64 = 12000;
/// A value transfer by a transaction: the recipient's write and the transfer log
pub const TX_VALUE_COST: u64 = 6000;
/// Per calldata token (a zero byte is one token, a non-zero byte four)
pub const TX_DATA_TOKEN_STANDARD: u64 = 4;
/// Per token of the calldata floor
pub const TX_DATA_TOKEN_FLOOR: u64 = 16;
/// Per address of an access list: it is left warm
pub const TX_ACCESS_LIST_ADDRESS: u64 = (COLD_ACCOUNT_ACCESS - WARM_ACCESS) as u64;
/// Per storage key of an access list
pub const TX_ACCESS_LIST_STORAGE_KEY: u64 = (COLD_STORAGE_ACCESS - WARM_ACCESS) as u64;
/// Floor tokens of an access list address (20 bytes, four tokens each)
pub const ACCESS_LIST_ADDRESS_FLOOR_TOKENS: u64 = 80;
/// Floor tokens of an access list storage key (32 bytes, four tokens each)
pub const ACCESS_LIST_STORAGE_KEY_FLOOR_TOKENS: u64 = 128;
/// The execution gas a transaction can spend (EIP-7825)
pub const TX_MAX_GAS_LIMIT: u64 = 1 << 24;
/// The gas limit of a transaction, execution and state gas together (EIP-8037)
pub const TX_MAX_TOTAL_GAS_LIMIT: u64 = u32::MAX as u64;
/// Bytes of an authorization tuple
pub const AUTH_TUPLE_BYTES: u64 = 101;
/// The state-independent cost of an authorization: its data at the floor rate, the
/// signature recovery, the authority's access and two warm reads
pub const EXECUTION_PER_AUTH_BASE_COST: u64 =
    AUTH_TUPLE_BYTES * TX_DATA_TOKEN_FLOOR + 3000 + (COLD_ACCOUNT_ACCESS + 2 * WARM_ACCESS) as u64;

// --- limits (EIP-7954) ------------------------------------------------------------------

pub const MAX_CODE_SIZE: usize = 0x10000;
pub const MAX_INITCODE_SIZE: usize = 2 * MAX_CODE_SIZE;

/// The gas of hashing deployed code (`KECCAK256_PER_WORD` per word) and the state gas of
/// storing it (`COST_PER_STATE_BYTE` per byte)
pub const fn code_deposit_costs(code_len: usize) -> (u32, u32) {
    (
        // at most `MAX_CODE_SIZE / 32` words
        KECCAK256_PER_WORD * (code_len as u32).div_ceil(32),
        // at most `MAX_CODE_SIZE * COST_PER_STATE_BYTE`, which fits
        (code_len as u32) * COST_PER_STATE_BYTE,
    )
}

const _: () = assert!(MAX_CODE_SIZE as u64 * COST_PER_STATE_BYTE as u64 <= u32::MAX as u64);
const _: () = assert!(EXECUTION_PER_AUTH_BASE_COST == 7816);
const _: () = assert!(REFUND_STORAGE_CLEAR == 11616);
