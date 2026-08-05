use alloy::{
    consensus::{SignableTransaction, Signed, Transaction, TxEnvelope, TypedTransaction},
    eips::Typed2718,
    network::TxSignerSync,
    primitives::Address,
    rpc::types::TransactionRequest,
    signers::Signature,
};

use crate::zksync_tx::{
    l1_tx::ZKsyncL1Tx, service_tx::ZKsyncServiceTx, upgrade_tx::ZKsyncUpgradeTx,
};

pub mod encoding;
pub mod l1_tx;
pub mod service_tx;
pub mod upgrade_tx;

/// Wrapper over all tx envelope kinds we feed into ZKsync OS.
#[derive(Clone)]
pub enum ZKsyncTxEnvelope {
    /// Standard Ethereum typed envelope signed by the corresponding address.
    Ethereum(TxEnvelope, Address),
    /// ZKsync OS specific typed envelope.
    ZKsync(ZKsyncSpecificTxEnvelope),
    /// Raw request with an explicit type byte (used for negative/fuzz cases).
    Custom(u8, TransactionRequest),
}

impl ZKsyncTxEnvelope {
    /// Builds a custom-typed tx envelope (including intentionally invalid types).
    pub fn new_custom_tx_type(inner: TransactionRequest, tx_type: u8) -> Self {
        Self::Custom(tx_type, inner)
    }

    /// Signs an Ethereum transaction and wraps it as an ZKsync OS-compatible envelope.
    pub fn from_eth_tx<T: SignableTransaction<Signature>, S: TxSignerSync<Signature>>(
        mut tx: T,
        signer: S,
    ) -> Self
    where
        Signed<T>: Into<TxEnvelope>,
    {
        let sig: Signature = signer
            .sign_transaction_sync(&mut tx)
            .expect("transaction signing failed");
        let signed: Signed<T> = tx.into_signed(sig);
        let env: TxEnvelope = signed.into();
        Self::Ethereum(env, signer.address())
    }

    /// Same as `from_eth_tx`, but starts from `TransactionRequest`.
    ///
    /// Can be more convenient if exact tx type is not important.
    pub fn from_eth_tx_from_req<S: TxSignerSync<Signature>>(
        req: TransactionRequest,
        signer: S,
    ) -> Self {
        let typed_tx = if req.blob_versioned_hashes.is_some() {
            // Tests do not attach sidecars; encode 4844 as a bare typed tx.
            req.build_4844_without_sidecar()
                .expect("Failed to build 4844 tx")
                .into()
        } else {
            req.build_typed_tx().expect("Failed to build typed tx")
        };
        match typed_tx {
            TypedTransaction::Legacy(tx) => Self::from_eth_tx(tx, signer),
            TypedTransaction::Eip1559(tx) => Self::from_eth_tx(tx, signer),
            TypedTransaction::Eip7702(tx) => Self::from_eth_tx(tx, signer),
            TypedTransaction::Eip2930(tx) => Self::from_eth_tx(tx, signer),
            TypedTransaction::Eip4844(tx) => Self::from_eth_tx(tx, signer),
        }
    }

    /// Returns the call target address if this tx kind has one.
    pub fn to(&self) -> Option<alloy::primitives::Address> {
        match &self {
            Self::Ethereum(env, _) => env.to(),
            Self::ZKsync(specific_envelope) => Some(specific_envelope.to()),
            Self::Custom(_, req) => match req.to {
                Some(to) => to.to().copied(),
                None => None,
            },
        }
    }
}

impl Typed2718 for ZKsyncTxEnvelope {
    fn ty(&self) -> u8 {
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

/// ZKsync OS specific transactions wrapper.
#[derive(Clone)]
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
}

impl Typed2718 for ZKsyncSpecificTxEnvelope {
    fn ty(&self) -> u8 {
        match self {
            ZKsyncSpecificTxEnvelope::L1(tx) => tx.ty(),
            ZKsyncSpecificTxEnvelope::Upgrade(tx) => tx.ty(),
            ZKsyncSpecificTxEnvelope::Service(tx) => tx.ty(),
        }
    }
}
