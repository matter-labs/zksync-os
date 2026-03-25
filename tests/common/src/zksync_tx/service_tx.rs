use alloy::{
    eips::{eip2718::IsTyped2718, Encodable2718, Typed2718},
    primitives::{Address, Bytes},
    rlp::{BufMut, Encodable},
};

use alloy::consensus::transaction::RlpEcdsaEncodableTx;

use crate::zksync_tx::ZKsyncSpecificTxEnvelope;

/// ZKsync OS service tx payload for system-internal calls.
#[derive(Debug, Default, Clone)]
pub struct ZKsyncServiceTx {
    pub to: Address,
    pub input: Bytes,
    pub salt: u64,
}

impl ZKsyncServiceTx {
    /// Canonical 2718 type byte for service txs.
    pub const TX_TYPE: u8 = 0x7d;
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

impl Encodable2718 for ZKsyncServiceTx {
    fn encode_2718_len(&self) -> usize {
        1 + self.length()
    }

    fn encode_2718(&self, out: &mut dyn BufMut) {
        let mut rlp_body = Vec::new();
        Encodable::encode(&self, &mut rlp_body);
        out.put_u8(Self::TX_TYPE);
        out.put_slice(&rlp_body);
    }
}

impl RlpEcdsaEncodableTx for ZKsyncServiceTx {
    fn rlp_encoded_fields_length(&self) -> usize {
        self.to.length() + self.input.length() + self.salt.length()
    }

    fn rlp_encode_fields(&self, out: &mut dyn BufMut) {
        self.to.encode(out);
        self.input.encode(out);
        self.salt.encode(out);
    }
}

impl Encodable for ZKsyncServiceTx {
    fn encode(&self, out: &mut dyn BufMut) {
        self.rlp_encode(out);
    }

    fn length(&self) -> usize {
        self.rlp_encoded_length()
    }
}
