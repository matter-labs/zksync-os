//! Fluent builder APIs for chain state and transactions.
//!
//! # `ChainBuilder`
//!
//! Wraps [`Chain::empty`] plus the various `set_*` methods into a single chainable call:
//!
//! ```rust,ignore
//! use rig::builder::ChainBuilder;
//! use rig::constants::*;
//!
//! let mut chain = ChainBuilder::new()
//!     .chain_id(TEST_CHAIN_ID)
//!     .with_balance(sender_addr, U256::from(DEFAULT_BALANCE))
//!     .with_evm_bytecode(contract_addr, &bytecode)
//!     .build();
//! ```
//!
//! # `TxBuilder`
//!
//! Wraps transaction construction and signing:
//!
//! ```rust,ignore
//! use rig::builder::TxBuilder;
//! use rig::constants::*;
//!
//! let tx = TxBuilder::new()
//!     .eip1559()
//!     .from(&signer)
//!     .to(contract_addr)
//!     .calldata(my_calldata)
//!     .gas_limit(CALL_GAS_LIMIT)
//!     .build();
//! ```

use crate::chain::Chain;
use crate::constants::{
    CALL_GAS_LIMIT, DEFAULT_MAX_FEE, DEFAULT_PRIORITY_FEE, TEST_CHAIN_ID,
};
use alloy::consensus::{TxEip1559, TxEip2930, TxLegacy};
use alloy::eips::eip2930::AccessList;
use alloy::primitives::{Address, Bytes, TxKind, U256 as AlloyU256};
use alloy::rpc::types::TransactionRequest;
use alloy::signers::local::PrivateKeySigner;
use ruint::aliases::{B160, B256, U256};
use zksync_os_interface::traits::EncodedTx;
use zk_ee::utils::Bytes32;

// ─── ChainBuilder ─────────────────────────────────────────────────────────────

/// Fluent builder for [`Chain<false>`].
///
/// Call [`ChainBuilder::new`], configure the initial state with method chaining, then call
/// [`ChainBuilder::build`] to obtain a ready-to-use [`Chain`].
pub struct ChainBuilder {
    chain_id: u64,
    balances: Vec<(B160, U256)>,
    bytecodes: Vec<(B160, Vec<u8>)>,
    storage_slots: Vec<(B160, U256, B256)>,
    preimages: Vec<(Bytes32, Vec<u8>)>,
}

impl ChainBuilder {
    /// Start a new builder with default chain ID ([`TEST_CHAIN_ID`]).
    pub fn new() -> Self {
        Self {
            chain_id: TEST_CHAIN_ID,
            balances: Vec::new(),
            bytecodes: Vec::new(),
            storage_slots: Vec::new(),
            preimages: Vec::new(),
        }
    }

    /// Override the chain ID (default: [`TEST_CHAIN_ID`] = 37).
    pub fn chain_id(mut self, id: u64) -> Self {
        self.chain_id = id;
        self
    }

    /// Fund `address` with `amount` wei.
    pub fn with_balance(mut self, address: B160, amount: U256) -> Self {
        self.balances.push((address, amount));
        self
    }

    /// Fund `address` with `amount` wei (alloy [`Address`] overload).
    pub fn with_balance_addr(self, address: Address, amount: U256) -> Self {
        self.with_balance(B160::from_be_bytes(address.into_array()), amount)
    }

    /// Deploy EVM `bytecode` at `address`.
    pub fn with_evm_bytecode(mut self, address: B160, bytecode: Vec<u8>) -> Self {
        self.bytecodes.push((address, bytecode));
        self
    }

    /// Deploy EVM `bytecode` at `address` (alloy [`Address`] overload).
    pub fn with_evm_bytecode_addr(self, address: Address, bytecode: Vec<u8>) -> Self {
        self.with_evm_bytecode(B160::from_be_bytes(address.into_array()), bytecode)
    }

    /// Set a storage slot `(address, key) = value` before the first block runs.
    pub fn with_storage_slot(mut self, address: B160, key: U256, value: B256) -> Self {
        self.storage_slots.push((address, key, value));
        self
    }

    /// Register a preimage so it can be looked up by hash during block execution.
    pub fn with_preimage(mut self, hash: Bytes32, data: Vec<u8>) -> Self {
        self.preimages.push((hash, data));
        self
    }

    /// Consume the builder and return a fully-configured [`Chain`].
    pub fn build(self) -> Chain<false> {
        let mut chain = Chain::empty(Some(self.chain_id));
        for (addr, amount) in self.balances {
            chain.set_balance(addr, amount);
        }
        for (addr, bytecode) in self.bytecodes {
            chain.set_evm_bytecode(addr, &bytecode);
        }
        for (addr, key, value) in self.storage_slots {
            chain.set_storage_slot(addr, key, value);
        }
        for (hash, data) in self.preimages {
            chain.set_preimage(hash, &data);
        }
        chain
    }
}

impl Default for ChainBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ─── TxBuilder ────────────────────────────────────────────────────────────────

/// Tx type selection for [`TxBuilder`].
#[derive(Clone, Copy, Debug, Default)]
pub enum TxType {
    #[default]
    Eip1559,
    Legacy,
    Eip2930,
    L1,
    Upgrade,
}

/// Fluent builder for signed, RLP-encoded transactions.
///
/// Always call one of the tx-type selectors ([`TxBuilder::eip1559`], [`TxBuilder::legacy`], …)
/// before calling [`TxBuilder::build`].
pub struct TxBuilder {
    tx_type: TxType,
    chain_id: u64,
    signer: Option<PrivateKeySigner>,
    to: TxKind,
    calldata: Vec<u8>,
    value: AlloyU256,
    gas_limit: u64,
    nonce: u64,
    max_fee_per_gas: u128,
    max_priority_fee_per_gas: u128,
    access_list: AccessList,
}

impl TxBuilder {
    /// Create a new builder defaulting to EIP-1559 with sensible test parameters.
    pub fn new() -> Self {
        Self {
            tx_type: TxType::Eip1559,
            chain_id: TEST_CHAIN_ID,
            signer: None,
            to: TxKind::Call(Address::ZERO),
            calldata: Vec::new(),
            value: AlloyU256::ZERO,
            gas_limit: CALL_GAS_LIMIT,
            nonce: 0,
            max_fee_per_gas: DEFAULT_MAX_FEE,
            max_priority_fee_per_gas: DEFAULT_PRIORITY_FEE,
            access_list: AccessList::default(),
        }
    }

    /// Build an EIP-1559 (type 2) transaction.
    pub fn eip1559(mut self) -> Self {
        self.tx_type = TxType::Eip1559;
        self
    }

    /// Build a legacy (type 0) transaction.
    pub fn legacy(mut self) -> Self {
        self.tx_type = TxType::Legacy;
        self
    }

    /// Build an EIP-2930 (type 1) transaction.
    pub fn eip2930(mut self) -> Self {
        self.tx_type = TxType::Eip2930;
        self
    }

    /// Build an L1 → L2 transaction (type 0x7f).
    pub fn l1(mut self) -> Self {
        self.tx_type = TxType::L1;
        self
    }

    /// Build an upgrade transaction (type 0x7e).
    pub fn upgrade(mut self) -> Self {
        self.tx_type = TxType::Upgrade;
        self
    }

    /// Set the chain ID (default: [`TEST_CHAIN_ID`]).
    pub fn chain_id(mut self, id: u64) -> Self {
        self.chain_id = id;
        self
    }

    /// Set the signer/sender wallet.
    pub fn from(mut self, signer: PrivateKeySigner) -> Self {
        self.signer = Some(signer);
        self
    }

    /// Set the recipient address.
    pub fn to(mut self, address: Address) -> Self {
        self.to = TxKind::Call(address);
        self
    }

    /// Mark transaction as a contract creation (`to = null`).
    pub fn create(mut self) -> Self {
        self.to = TxKind::Create;
        self
    }

    /// Set the call input data.
    pub fn calldata(mut self, data: Vec<u8>) -> Self {
        self.calldata = data;
        self
    }

    /// Set the ETH value to send.
    pub fn value(mut self, amount: AlloyU256) -> Self {
        self.value = amount;
        self
    }

    /// Override the gas limit (default: [`CALL_GAS_LIMIT`]).
    pub fn gas_limit(mut self, limit: u64) -> Self {
        self.gas_limit = limit;
        self
    }

    /// Override the nonce (default: 0).
    pub fn nonce(mut self, n: u64) -> Self {
        self.nonce = n;
        self
    }

    /// Override max fee per gas (default: [`DEFAULT_MAX_FEE`]).
    pub fn max_fee(mut self, fee: u128) -> Self {
        self.max_fee_per_gas = fee;
        self
    }

    /// Override max priority fee per gas (default: [`DEFAULT_PRIORITY_FEE`]).
    pub fn priority_fee(mut self, fee: u128) -> Self {
        self.max_priority_fee_per_gas = fee;
        self
    }

    /// Set the access list (EIP-2930 / EIP-1559).
    ///
    /// Has no effect on legacy or L1/Upgrade tx types.
    ///
    /// # Example
    /// ```rust,ignore
    /// use alloy::eips::eip2930::{AccessList, AccessListItem};
    /// use alloy::primitives::Address;
    ///
    /// let al = AccessList(vec![AccessListItem {
    ///     address: Address::ZERO,
    ///     storage_keys: vec![],
    /// }]);
    /// let tx = TxBuilder::new().eip2930().from(signer).to(addr).access_list(al).build();
    /// ```
    pub fn access_list(mut self, al: AccessList) -> Self {
        self.access_list = al;
        self
    }

    /// Sign and encode the transaction.
    ///
    /// # Panics
    /// Panics if no signer was provided via [`TxBuilder::from`].
    pub fn build(self) -> EncodedTx {
        use crate::utils::*;

        let signer = self.signer.expect("TxBuilder: no signer set — call .from(signer)");

        match self.tx_type {
            TxType::Eip1559 => {
                let tx = TxEip1559 {
                    chain_id: self.chain_id,
                    nonce: self.nonce,
                    max_fee_per_gas: self.max_fee_per_gas,
                    max_priority_fee_per_gas: self.max_priority_fee_per_gas,
                    gas_limit: self.gas_limit,
                    to: self.to,
                    value: self.value,
                    access_list: self.access_list,
                    input: Bytes::from(self.calldata),
                };
                sign_and_encode_alloy_tx(tx, &signer)
            }
            TxType::Legacy => {
                let tx = TxLegacy {
                    chain_id: Some(self.chain_id),
                    nonce: self.nonce,
                    gas_price: self.max_fee_per_gas,
                    gas_limit: self.gas_limit,
                    to: self.to,
                    value: self.value,
                    input: Bytes::from(self.calldata),
                };
                sign_and_encode_alloy_tx(tx, &signer)
            }
            TxType::Eip2930 => {
                let tx = TxEip2930 {
                    chain_id: self.chain_id,
                    nonce: self.nonce,
                    gas_price: self.max_fee_per_gas,
                    gas_limit: self.gas_limit,
                    to: self.to,
                    value: self.value,
                    access_list: self.access_list,
                    input: Bytes::from(self.calldata),
                };
                sign_and_encode_alloy_tx(tx, &signer)
            }
            TxType::L1 => {
                let to_addr = match self.to {
                    TxKind::Call(a) => a,
                    TxKind::Create => panic!("TxBuilder: L1 tx cannot be a Create"),
                };
                let req = TransactionRequest {
                    chain_id: Some(self.chain_id),
                    from: Some(signer.address()),
                    to: Some(TxKind::Call(to_addr)),
                    input: Bytes::from(self.calldata).into(),
                    gas: Some(self.gas_limit as u64),
                    max_fee_per_gas: Some(self.max_fee_per_gas),
                    max_priority_fee_per_gas: Some(self.max_priority_fee_per_gas),
                    value: Some(self.value),
                    nonce: Some(self.nonce),
                    ..TransactionRequest::default()
                };
                encode_l1_tx(req)
            }
            TxType::Upgrade => {
                let to_addr = match self.to {
                    TxKind::Call(a) => a,
                    TxKind::Create => panic!("TxBuilder: upgrade tx cannot be a Create"),
                };
                let req = TransactionRequest {
                    chain_id: Some(self.chain_id),
                    from: Some(signer.address()),
                    to: Some(TxKind::Call(to_addr)),
                    input: Bytes::from(self.calldata).into(),
                    gas: Some(self.gas_limit as u64),
                    max_fee_per_gas: Some(self.max_fee_per_gas),
                    max_priority_fee_per_gas: Some(self.max_priority_fee_per_gas),
                    value: Some(self.value),
                    nonce: Some(self.nonce),
                    ..TransactionRequest::default()
                };
                encode_upgrade_tx(req)
            }
        }
    }
}

impl Default for TxBuilder {
    fn default() -> Self {
        Self::new()
    }
}
