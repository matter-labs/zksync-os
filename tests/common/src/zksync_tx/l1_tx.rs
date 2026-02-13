use alloy::{
    eips::{eip2718::IsTyped2718, Typed2718},
    primitives::{Address, Bytes, B256, U256},
};

use crate::zksync_tx::ZKsyncSpecificTxEnvelope;

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
