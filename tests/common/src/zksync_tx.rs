use std::ops::Add;

use alloy::{
    consensus::{SignableTransaction, Signed, Transaction, TxEnvelope, TypedTransaction},
    eips::{eip2718::IsTyped2718, Typed2718},
    network::TxSignerSync,
    primitives::{Address, Bytes, B256, U160, U256},
    rpc::types::TransactionRequest,
    signers::{local::PrivateKeySigner, Signature},
};

pub enum ZKsyncTxEnvelope {
    Ethereum(TxEnvelope, PrivateKeySigner),
    ZKsyncEnvelope(ZKsyncSpecificTxEnvelope),
    Custom(u8, TransactionRequest), // For custom transaction types
}

impl ZKsyncTxEnvelope {
    pub fn new_l1(inner: TransactionRequest) -> Self {
        let to_mint = inner.value.unwrap_or_default().add(U256::from(
            inner.gas.unwrap_or_default() as u128 * inner.max_fee_per_gas.unwrap_or_default(),
        )); // TODO overflow
            // This behavior was implemented incorrectly before and we keep it as is for now to avoid breaking existing tests
        let refund_recipient_as_uint = if inner.to.is_none() {
            U256::ONE
        } else {
            U256::ZERO
        };
        let refund_recipient = Address::from(U160::from(refund_recipient_as_uint));

        let l1_tx = ZKsyncL1Tx {
            from: inner.from.expect("L1 tx should have from field"),
            to: inner
                .to
                .expect("L1 tx should have to field")
                .to()
                .cloned()
                .expect("L1 tx should not be of Create type"),
            gas_limit: inner.gas.unwrap_or_default() as u128,
            gas_per_pubdata_byte_limit: 0, // This field is not present in the TransactionRequest, set to 0
            max_fee_per_gas: inner.max_fee_per_gas.unwrap_or_default(),
            max_priority_fee_per_gas: inner.max_priority_fee_per_gas.unwrap_or_default(),
            nonce: inner.nonce.unwrap_or_default() as u128,
            value: inner.value.unwrap_or_default(),
            to_mint,
            refund_recipient,
            input: inner.input.input().cloned().unwrap_or_default(),
            factory_deps: vec![], // Not supported
        };
        l1_tx.into()
    }

    pub fn new_special_tx_type(inner: TransactionRequest, tx_type: u8) -> Self {
        Self::Custom(tx_type, inner)
    }

    pub fn new_eth_tx<T: SignableTransaction<Signature>>(
        mut tx: T,
        signer: PrivateKeySigner,
    ) -> Self
    where
        Signed<T>: Into<TxEnvelope>,
    {
        let sig: Signature = signer
            .sign_transaction_sync(&mut tx)
            .expect("transaction signing failed");
        let signed: Signed<T> = tx.into_signed(sig);
        let env: TxEnvelope = signed.into();
        Self::Ethereum(env, signer)
    }

    pub fn new_eth_tx_from_req(req: TransactionRequest, signer: PrivateKeySigner) -> Self {
        let typed_tx = if req.blob_versioned_hashes.is_some() {
            req.build_4844_without_sidecar()
                .expect("Failed to build 4844 tx")
                .into()
        } else {
            req.build_typed_tx().expect("Failed to build typed tx")
        };
        match typed_tx {
            TypedTransaction::Legacy(tx) => Self::new_eth_tx(tx, signer),
            TypedTransaction::Eip1559(tx) => Self::new_eth_tx(tx, signer),
            TypedTransaction::Eip7702(tx) => Self::new_eth_tx(tx, signer),
            TypedTransaction::Eip2930(tx) => Self::new_eth_tx(tx, signer),
            TypedTransaction::Eip4844(tx) => Self::new_eth_tx(tx, signer),
        }
    }

    pub fn to(&self) -> Option<alloy::primitives::Address> {
        match &self {
            Self::Ethereum(env, _) => env.to(),
            Self::ZKsyncEnvelope(specific_envelope) => Some(specific_envelope.to()),
            Self::Custom(_, req) => req.to.as_ref().map(|to| to.to().copied().unwrap()), // TODO unwrap is incorrect here
        }
    }

    pub fn ty(&self) -> u8 {
        match &self {
            Self::Ethereum(ethereum_tx_envelope, _) => ethereum_tx_envelope.ty(),
            Self::ZKsyncEnvelope(specific_envelope) => specific_envelope.ty(),
            Self::Custom(tx_type, _) => *tx_type,
        }
    }
}

impl From<ZKsyncL1Tx> for ZKsyncTxEnvelope {
    fn from(val: ZKsyncL1Tx) -> Self {
        ZKsyncTxEnvelope::ZKsyncEnvelope(val.into())
    }
}

impl From<ZKsyncUpgradeTx> for ZKsyncTxEnvelope {
    fn from(val: ZKsyncUpgradeTx) -> Self {
        ZKsyncTxEnvelope::ZKsyncEnvelope(val.into())
    }
}

impl From<ZKsyncServiceTx> for ZKsyncTxEnvelope {
    fn from(val: ZKsyncServiceTx) -> Self {
        ZKsyncTxEnvelope::ZKsyncEnvelope(val.into())
    }
}

pub enum ZKsyncSpecificTxEnvelope {
    L1(ZKsyncL1Tx),
    Upgrade(ZKsyncUpgradeTx),
    Service(ZKsyncServiceTx),
}

impl ZKsyncSpecificTxEnvelope {
    pub fn to(&self) -> Address {
        match self {
            ZKsyncSpecificTxEnvelope::L1(tx) => tx.to,
            ZKsyncSpecificTxEnvelope::Upgrade(tx) => tx.to,
            ZKsyncSpecificTxEnvelope::Service(tx) => tx.to,
        }
    }

    pub fn ty(&self) -> u8 {
        match self {
            ZKsyncSpecificTxEnvelope::L1(_) => ZKsyncL1Tx::TX_TYPE,
            ZKsyncSpecificTxEnvelope::Upgrade(_) => ZKsyncUpgradeTx::TX_TYPE,
            ZKsyncSpecificTxEnvelope::Service(_) => ZKsyncServiceTx::TX_TYPE,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct ZKsyncL1Tx {
    pub from: Address,
    pub to: Address,
    pub gas_limit: u128,
    pub gas_per_pubdata_byte_limit: u128,
    pub max_fee_per_gas: u128,
    pub max_priority_fee_per_gas: u128,
    pub nonce: u128,
    pub value: U256,
    /// The amount of base token that should be minted on L2 as the result of this transaction.
    pub to_mint: U256,
    /// The recipient of the refund for the transaction on L2. If the transaction fails, then this
    /// address will receive the `value` of this transaction.
    pub refund_recipient: Address,
    /// data: An unlimited size byte array specifying the input data of the message call.
    pub input: Bytes,
    /// The set of L2 bytecode hashes whose preimages were shown on L1.
    pub factory_deps: Vec<B256>,
}

impl ZKsyncL1Tx {
    const TX_TYPE: u8 = 0x7f;
}

impl Typed2718 for ZKsyncL1Tx {
    fn ty(&self) -> u8 {
        Self::TX_TYPE
    }
}

impl From<ZKsyncL1Tx> for ZKsyncSpecificTxEnvelope {
    fn from(val: ZKsyncL1Tx) -> Self {
        ZKsyncSpecificTxEnvelope::L1(val)
    }
}

impl IsTyped2718 for ZKsyncL1Tx {
    fn is_type(type_id: u8) -> bool {
        matches!(type_id, Self::TX_TYPE)
    }
}

#[derive(Debug, Default, Clone)]
pub struct ZKsyncUpgradeTx {
    pub from: Address,
    pub to: Address,
    pub gas_limit: u128,
    pub gas_per_pubdata_byte_limit: u128,
    pub max_fee_per_gas: u128,
    pub max_priority_fee_per_gas: u128,
    pub nonce: u128,
    pub value: U256,
    /// The amount of base token that should be minted on L2 as the result of this transaction.
    pub to_mint: U256,
    /// The recipient of the refund for the transaction on L2. If the transaction fails, then this
    /// address will receive the `value` of this transaction.
    pub refund_recipient: Address,
    /// data: An unlimited size byte array specifying the input data of the message call.
    pub input: Bytes,
    /// The set of L2 bytecode hashes whose preimages were shown on L1.
    pub factory_deps: Vec<B256>,
}

impl ZKsyncUpgradeTx {
    const TX_TYPE: u8 = 0x7e;
}

impl Typed2718 for ZKsyncUpgradeTx {
    fn ty(&self) -> u8 {
        Self::TX_TYPE
    }
}

impl IsTyped2718 for ZKsyncUpgradeTx {
    fn is_type(type_id: u8) -> bool {
        matches!(type_id, Self::TX_TYPE)
    }
}

impl From<ZKsyncUpgradeTx> for ZKsyncSpecificTxEnvelope {
    fn from(val: ZKsyncUpgradeTx) -> Self {
        ZKsyncSpecificTxEnvelope::Upgrade(val)
    }
}

#[derive(Debug, Default, Clone)]
pub struct ZKsyncServiceTx {
    pub to: Address,
    pub input: Bytes,
}

impl ZKsyncServiceTx {
    const TX_TYPE: u8 = 0x7d;
}

impl Typed2718 for ZKsyncServiceTx {
    fn ty(&self) -> u8 {
        Self::TX_TYPE
    }
}

impl IsTyped2718 for ZKsyncServiceTx {
    fn is_type(type_id: u8) -> bool {
        matches!(type_id, Self::TX_TYPE)
    }
}

impl From<ZKsyncServiceTx> for ZKsyncSpecificTxEnvelope {
    fn from(val: ZKsyncServiceTx) -> Self {
        ZKsyncSpecificTxEnvelope::Service(val)
    }
}
