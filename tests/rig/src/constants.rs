//! Named constants for ZKsync OS integration tests.

use alloy::primitives::address;
use alloy::primitives::Address;

/// Default chain ID used in tests.
pub const TEST_CHAIN_ID: u64 = 37;

/// A conservative default gas limit suitable for simple calls.
pub const DEFAULT_GAS_LIMIT: u64 = 200_000;

/// Gas limit used for ETH-transfer transactions.
pub const TRANSFER_GAS_LIMIT: u64 = 60_000;

/// Gas limit used when deploying contracts.
pub const DEPLOY_GAS_LIMIT: u64 = 900_000;

/// Gas limit used for generic contract calls.
pub const CALL_GAS_LIMIT: u64 = 200_000;

/// Default EIP-1559 base fee.
pub const DEFAULT_BASEFEE: u128 = 1_000;

/// Default `max_fee_per_gas` value for test transactions.
pub const DEFAULT_MAX_FEE: u128 = 1_000;

/// Default `max_priority_fee_per_gas` value for test transactions.
pub const DEFAULT_PRIORITY_FEE: u128 = 1_000;

/// Default native-token price used by the default block context.
pub const DEFAULT_NATIVE_PRICE: u64 = 10;

/// A large ETH balance suitable for most test senders (1_000_000_000_000_000 wei).
pub const DEFAULT_BALANCE: u64 = 1_000_000_000_000_000;

/// `ContractDeployer` system contract (0x0000…8006).
pub const CONTRACT_DEPLOYER: Address = address!("0000000000000000000000000000000000008006");

/// L2 base-token (ETH) system contract (0x0000…800a).
pub const L2_BASE_TOKEN: Address = address!("000000000000000000000000000000000000800a");

/// L1 messenger system contract (0x0000…8008).
pub const L1_MESSENGER: Address = address!("0000000000000000000000000000000000008008");

/// `MsgValueSimulator` system contract (0x0000…8009).
pub const MSG_VALUE_SIMULATOR: Address = address!("0000000000000000000000000000000000008009");

/// `NonceHolder` system contract (0x0000…8003).
pub const NONCE_HOLDER: Address = address!("0000000000000000000000000000000000008003");

/// `AccountCodeStorage` system contract (0x0000…8002).
pub const ACCOUNT_CODE_STORAGE: Address = address!("0000000000000000000000000000000000008002");
