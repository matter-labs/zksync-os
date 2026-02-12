use alloy::{
    consensus::{SignableTransaction, Signed, Transaction, TxEnvelope, TypedTransaction},
    network::TxSignerSync,
    rpc::types::TransactionRequest,
    signers::{local::PrivateKeySigner, Signature},
};

pub enum ZKsyncTxType {
    ZKsync(ZKsyncSpecificTxType, TransactionRequest),
    Ethereum(TxEnvelope),
}

pub enum ZKsyncSpecificTxType {
    L1,
    Upgrade,
    Service,
    Custom(u8),
}

pub struct ZKsyncTxEnvelope {
    pub inner: ZKsyncTxType,
    pub signer: Option<PrivateKeySigner>, // For L2 transactions, we must have a signer
}

impl ZKsyncTxEnvelope {
    pub fn new_l1(inner: TransactionRequest) -> Self {
        Self {
            inner: ZKsyncTxType::ZKsync(ZKsyncSpecificTxType::L1, inner),
            signer: None,
        }
    }

    pub fn new_l2_req(req: TransactionRequest, signer: PrivateKeySigner) -> Self {
        let typed_tx = if req.blob_versioned_hashes.is_some() {
            req.build_4844_without_sidecar()
                .expect("Failed to build 4844 tx")
                .into()
        } else {
            req.build_typed_tx().expect("Failed to build typed tx")
        };
        match typed_tx {
            TypedTransaction::Legacy(tx) => Self::new_l2_tx(tx, signer),
            TypedTransaction::Eip1559(tx) => Self::new_l2_tx(tx, signer),
            TypedTransaction::Eip7702(tx) => Self::new_l2_tx(tx, signer),
            TypedTransaction::Eip2930(tx) => Self::new_l2_tx(tx, signer),
            TypedTransaction::Eip4844(tx) => Self::new_l2_tx(tx, signer),
        }
    }

    pub fn new_upgrade_tx(inner: TransactionRequest) -> Self {
        Self {
            inner: ZKsyncTxType::ZKsync(ZKsyncSpecificTxType::Upgrade, inner),
            signer: None,
        }
    }

    pub fn new_service_tx(inner: TransactionRequest) -> Self {
        Self {
            inner: ZKsyncTxType::ZKsync(ZKsyncSpecificTxType::Service, inner),
            signer: None,
        }
    }

    pub fn new_special_tx_type(inner: TransactionRequest, tx_type: u8) -> Self {
        Self {
            inner: ZKsyncTxType::ZKsync(ZKsyncSpecificTxType::Custom(tx_type), inner),
            signer: None,
        }
    }

    pub fn new_l2_tx<T: SignableTransaction<Signature>>(mut tx: T, signer: PrivateKeySigner) -> Self
    where
        Signed<T>: Into<TxEnvelope>,
    {
        let sig: Signature = signer
            .sign_transaction_sync(&mut tx)
            .expect("transaction signing failed");
        let signed: Signed<T> = tx.into_signed(sig);
        let env: TxEnvelope = signed.into();
        Self {
            inner: ZKsyncTxType::Ethereum(env),
            signer: Some(signer),
        }
    }

    pub fn to(&self) -> Option<alloy::primitives::Address> {
        match &self.inner {
            ZKsyncTxType::ZKsync(_, req) => req.to.as_ref().map(|to| to.to().copied().unwrap()),
            ZKsyncTxType::Ethereum(env) => env.to(),
        }
    }

    pub fn ty(&self) -> u8 {
        match &self.inner {
            ZKsyncTxType::ZKsync(zk_specific_type, _) => match zk_specific_type {
                ZKsyncSpecificTxType::L1 => 0x7f,
                ZKsyncSpecificTxType::Upgrade => 0x7e,
                ZKsyncSpecificTxType::Service => 0x7d,
                ZKsyncSpecificTxType::Custom(tx_type) => *tx_type,
            },
            ZKsyncTxType::Ethereum(ethereum_tx_envelope) => {
                ethereum_tx_envelope.tx_type().clone().into()
            }
        }
    }
}
