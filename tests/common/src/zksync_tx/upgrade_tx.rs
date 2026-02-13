use alloy::{
    eips::{eip2718::IsTyped2718, Typed2718},
    primitives::{Address, Bytes, B256, U160, U256},
    rlp::BufMut,
};

use crate::zksync_tx::{
    encoding::{encode_abi_tx, AbiEncodableTx},
    ZKsyncSpecificTxEnvelope,
};

/// ZKsync OS protocol-upgrade tx payload.
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
    /// Canonical 2718 type byte for upgrade txs.
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

impl AbiEncodableTx for ZKsyncUpgradeTx {
    fn abi_encode(&self, out: &mut dyn BufMut) {
        let tx_type = self.ty();
        let refund_recipient: U160 = self.refund_recipient.into();
        let reserved = [
            self.to_mint,
            U256::from(refund_recipient),
            U256::ZERO,
            U256::ZERO,
        ];
        let res = encode_abi_tx(
            tx_type,
            self.from.into_array(),
            Some(self.to.into_array()),
            self.gas_limit,
            Some(self.gas_per_pubdata_byte_limit),
            self.max_fee_per_gas,
            Some(self.max_priority_fee_per_gas),
            Some([0u8; 20]), // ignored in ZKsync OS
            self.nonce,
            self.value.to_be_bytes(),
            reserved,
            self.input.to_vec(),
            vec![],       // ignored in ZKsync OS
            Some(vec![]), // ignored in ZKsync OS
            None,         // ignored in ZKsync OS
            self.factory_deps.clone(),
        );
        out.put_slice(&res);
    }
}
