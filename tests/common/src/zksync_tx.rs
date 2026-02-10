use alloy::{rpc::types::TransactionRequest, signers::local::PrivateKeySigner};

pub enum ZKsyncTxType {
    L1,
    L2,
    Upgrade,
    Service,
}

pub struct ZKsyncTxRequest {
    pub tx_type: ZKsyncTxType,
    pub inner: TransactionRequest,
    pub signer: Option<PrivateKeySigner>, // For L2 transactions, we must have a signer
}

impl ZKsyncTxRequest {
    pub fn new_l1(inner: TransactionRequest) -> Self {
        Self {
            tx_type: ZKsyncTxType::L1,
            inner,
            signer: None,
        }
    }

    pub fn new_l2(inner: TransactionRequest, signer: PrivateKeySigner) -> Self {
        Self {
            tx_type: ZKsyncTxType::L2,
            inner,
            signer: Some(signer),
        }
    }

    pub fn new_upgrade_tx(inner: TransactionRequest) -> Self {
        Self {
            tx_type: ZKsyncTxType::Upgrade,
            inner,
            signer: None,
        }
    }

    pub fn new_service_tx(inner: TransactionRequest) -> Self {
        Self {
            tx_type: ZKsyncTxType::Service,
            inner,
            signer: None,
        }
    }

    pub fn ty(&self) -> u8 {
        match self.tx_type {
            ZKsyncTxType::L1 => 0x7f,
            ZKsyncTxType::L2 => 0x0, // Currently only support legacy transactions for L2, which have type 0
            ZKsyncTxType::Upgrade => 0x7d,
            ZKsyncTxType::Service => 0x7c,
        }
    }
}
