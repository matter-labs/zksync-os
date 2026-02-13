use alloy::{
    eips::{eip2718::IsTyped2718, Typed2718},
    primitives::{Address, Bytes},
};

use crate::zksync_tx::ZKsyncSpecificTxEnvelope;

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
