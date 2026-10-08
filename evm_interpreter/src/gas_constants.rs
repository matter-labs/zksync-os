// Based on https://github.com/bluealloy/revm/blob/main/crates/interpreter/src/gas/constants.rs

pub const ZERO: u32 = 0;
pub const BASE: u32 = 2;
pub const VERYLOW: u32 = 3;
pub const LOW: u32 = 5;
pub const MID: u32 = 8;
pub const HIGH: u32 = 10;
pub const JUMPDEST: u32 = 1;
pub const SELFDESTRUCT: u32 = 5000;
pub const CREATE: u32 = 32000;
pub const CALLVALUE: u32 = 9000;
pub const NEWACCOUNT: u32 = 25000;
pub const EXP: u32 = 10;
pub const MEMORY: u32 = 3;
pub const LOG: u32 = 375;
pub const LOGDATA: u32 = 8;
pub const LOGTOPIC: u32 = 375;
pub const SHA3: u32 = 30;
pub const SHA3WORD: u32 = 6;
pub const COPY: u32 = 3;
pub const BLOCKHASH: u32 = 20;
pub const CODEDEPOSIT: u32 = 200;
pub const BLOBHASH: u32 = 3;

// SSTORE write extras.
pub const REFUND_SSTORE_CLEARS: i64 = 15000;
pub const SSTORE_SET_EXTRA: u32 = 19900;
pub const SSTORE_RESET_EXTRA: u32 = 2800;

pub const TRANSACTION_ZERO_DATA: u32 = 4;
pub const TRANSACTION_NON_ZERO_DATA_INIT: u32 = 16;
pub const TRANSACTION_NON_ZERO_DATA_FRONTIER: u32 = 68;

// berlin eip2929 constants
pub const ACCESS_LIST_ADDRESS: u32 = 2400;
pub const ACCESS_LIST_STORAGE_KEY: u32 = 1900;
pub const COLD_SLOAD_COST: u32 = 2100;
pub const COLD_ACCOUNT_ACCESS_COST: u32 = 2600;
pub const WARM_STORAGE_READ_COST: u32 = 100;

/// EIP-3860 : Limit and meter initcode
pub const INITCODE_WORD_COST: u32 = 2;

pub const CALL_STIPEND: u32 = 2300;

pub const ADDRESS_ACCESS_COST_COLD: u32 = 2600;
pub const ADDRESS_ACCESS_COST_WARM: u32 = 100;

pub const TSTORE: u32 = 100;
pub const TLOAD: u32 = 100;
pub const SELFBALANCE: u32 = 5;

pub const PER_AUTH_BASE_COST: u32 = 12_500;
