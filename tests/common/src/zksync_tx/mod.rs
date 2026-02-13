use alloy::{
    consensus::{SignableTransaction, Signed, Transaction, TxEnvelope, TypedTransaction},
    eips::Typed2718,
    network::TxSignerSync,
    primitives::Address,
    rpc::types::TransactionRequest,
    signers::{local::PrivateKeySigner, Signature},
};

use crate::zksync_tx::{
    l1_tx::ZKsyncL1Tx, service_tx::ZKsyncServiceTx, upgrade_tx::ZKsyncUpgradeTx,
};

pub mod l1_tx;
pub mod service_tx;
pub mod upgrade_tx;

pub enum ZKsyncTxEnvelope {
    Ethereum(TxEnvelope, PrivateKeySigner),
    ZKsync(ZKsyncSpecificTxEnvelope),
    Custom(u8, TransactionRequest), // For custom transaction types
}

impl ZKsyncTxEnvelope {
    // Used to create transactions with custom (or invalid) types, for testing purposes.
    pub fn new_custom_tx_type(inner: TransactionRequest, tx_type: u8) -> Self {
        Self::Custom(tx_type, inner)
    }

    // Convert Ethereum transaction into ZKsync OS compatible envelope.
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

    // More flexible option, uses TransactionRequest.
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
            Self::ZKsync(specific_envelope) => Some(specific_envelope.to()),
            Self::Custom(_, req) => req.to.as_ref().map(|to| to.to().copied().unwrap()), // TODO unwrap is incorrect here
        }
    }

    pub fn ty(&self) -> u8 {
        match &self {
            Self::Ethereum(ethereum_tx_envelope, _) => ethereum_tx_envelope.ty(),
            Self::ZKsync(specific_envelope) => specific_envelope.ty(),
            Self::Custom(tx_type, _) => *tx_type,
        }
    }
}

impl From<ZKsyncL1Tx> for ZKsyncTxEnvelope {
    fn from(val: ZKsyncL1Tx) -> Self {
        ZKsyncTxEnvelope::ZKsync(val.into())
    }
}

impl From<ZKsyncUpgradeTx> for ZKsyncTxEnvelope {
    fn from(val: ZKsyncUpgradeTx) -> Self {
        ZKsyncTxEnvelope::ZKsync(val.into())
    }
}

impl From<ZKsyncServiceTx> for ZKsyncTxEnvelope {
    fn from(val: ZKsyncServiceTx) -> Self {
        ZKsyncTxEnvelope::ZKsync(val.into())
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
            ZKsyncSpecificTxEnvelope::L1(tx) => tx.ty(),
            ZKsyncSpecificTxEnvelope::Upgrade(tx) => tx.ty(),
            ZKsyncSpecificTxEnvelope::Service(tx) => tx.ty(),
        }
    }
}
