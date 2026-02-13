use std::ops::Add;

use alloy::{
    consensus::TxEnvelope,
    dyn_abi::DynSolValue,
    eips::Typed2718,
    primitives::{Address, B256, U160, U256},
    rpc::types::TransactionRequest,
    signers::local::PrivateKeySigner,
};
use zksync_os_api::helpers::encode_envelope_2718;
use zksync_os_interface::traits::EncodedTx;
use zksync_os_tests_common::zksync_tx::{
    ZKsyncL1Tx, ZKsyncServiceTx, ZKsyncSpecificTxEnvelope, ZKsyncTxEnvelope, ZKsyncTxType,
    ZKsyncUpgradeTx,
};

pub trait EncodableToEncodedTx {
    fn encode(&self) -> EncodedTx;
}

impl EncodableToEncodedTx for ZKsyncTxEnvelope {
    fn encode(&self) -> EncodedTx {
        match &self.inner {
            ZKsyncTxType::Ethereum(ethereum_tx_envelope) => encode_ethereum_tx_envelope(
                ethereum_tx_envelope,
                self.signer
                    .as_ref()
                    .expect("L2 transactions must have a signer"),
            ),
            ZKsyncTxType::ZKsyncEnvelope(zksync_specific_tx_envelope) => {
                match zksync_specific_tx_envelope {
                    ZKsyncSpecificTxEnvelope::L1(zksync_l1_tx) => {
                        encode_l1_tx_from_tx(zksync_l1_tx)
                    }
                    ZKsyncSpecificTxEnvelope::Upgrade(zksync_upgrade_tx) => {
                        encode_upgrade_tx_from_tx(zksync_upgrade_tx)
                    }
                    ZKsyncSpecificTxEnvelope::Service(zksync_service_tx) => {
                        encode_service_tx_from_tx(zksync_service_tx)
                    }
                }
            }
            ZKsyncTxType::Custom(custom_type, tx_req) => {
                encode_special_tx_type(tx_req.clone(), *custom_type)
            }
        }
    }
}

#[allow(deprecated)]
pub fn encode_alloy_rpc_tx(tx: alloy::rpc::types::Transaction) -> EncodedTx {
    let from = tx.as_recovered().signer().into_array();
    let env: TxEnvelope = tx.into();
    let bytes = encode_envelope_2718(&env);
    EncodedTx::Rlp(bytes, Address::from_slice(&from))
}

fn encode_ethereum_tx_envelope(tx_envelope: &TxEnvelope, signer: &PrivateKeySigner) -> EncodedTx {
    let bytes = encode_envelope_2718(tx_envelope);
    EncodedTx::Rlp(bytes, signer.address())
}

fn encode_special_tx_type(tx: TransactionRequest, tx_type: u8) -> EncodedTx {
    let from = tx.from.unwrap().into_array();
    let to = Some(tx.to.unwrap().to().unwrap().into_array());
    let gas_limit = tx.gas.unwrap() as u128;
    let gas_per_pubdata_byte_limit = Some(0u128);
    let max_fee_per_gas = tx.max_fee_per_gas.unwrap();
    let max_priority_fee_per_gas = Some(tx.max_priority_fee_per_gas.unwrap_or_default());
    let paymaster = Some([0u8; 20]);
    let nonce = tx.nonce.unwrap() as u128;
    let value = tx.value.unwrap_or_default().to_be_bytes();
    let data = tx.input.input.unwrap_or_default().to_vec();
    let signature = vec![];
    let paymaster_input = Some(vec![]);

    let reserved = [
        (if tx_type == 0 {
            // is_eip155 is true
            U256::ONE
        } else if tx_type == 0x7f {
            U256::from_be_bytes(value).add(U256::from(gas_limit * max_fee_per_gas))
        } else {
            U256::ZERO
        })
        .into(),
        (if to.is_none() { U256::ONE } else { U256::ZERO }).into(),
        U256::ZERO.into(),
        U256::ZERO.into(),
    ];

    encode_abi_tx(
        tx_type,
        from,
        to,
        gas_limit,
        gas_per_pubdata_byte_limit,
        max_fee_per_gas,
        max_priority_fee_per_gas,
        paymaster,
        nonce,
        value,
        reserved,
        data,
        signature,
        paymaster_input,
        None,
        vec![], // not supported here
    )
}

fn encode_l1_tx_from_tx(tx: &ZKsyncL1Tx) -> EncodedTx {
    let tx_type = tx.ty();
    let refund_recipient: U160 = tx.refund_recipient.into();
    let reserved = [
        tx.to_mint,
        U256::from(refund_recipient),
        U256::ZERO,
        U256::ZERO,
    ];
    encode_abi_tx(
        tx_type,
        tx.from.into_array(),
        Some(tx.to.into_array()),
        tx.gas_limit,
        Some(tx.gas_per_pubdata_byte_limit),
        tx.max_fee_per_gas,
        Some(tx.max_priority_fee_per_gas),
        Some([0u8; 20]), // ignored in ZKsync OS
        tx.nonce,
        tx.value.to_be_bytes(),
        reserved,
        tx.input.to_vec(),
        vec![],       // ignored in ZKsync OS
        Some(vec![]), // ignored in ZKsync OS
        None,         // ignored in ZKsync OS
        tx.factory_deps.clone(),
    )
}

fn encode_upgrade_tx_from_tx(tx: &ZKsyncUpgradeTx) -> EncodedTx {
    let tx_type = tx.ty();
    let refund_recipient: U160 = tx.refund_recipient.into();
    let reserved = [
        tx.to_mint,
        U256::from(refund_recipient),
        U256::ZERO,
        U256::ZERO,
    ];
    encode_abi_tx(
        tx_type,
        tx.from.into_array(),
        Some(tx.to.into_array()),
        tx.gas_limit,
        Some(tx.gas_per_pubdata_byte_limit),
        tx.max_fee_per_gas,
        Some(tx.max_priority_fee_per_gas),
        Some([0u8; 20]), // ignored in ZKsync OS
        tx.nonce,
        tx.value.to_be_bytes(),
        reserved,
        tx.input.to_vec(),
        vec![],       // ignored in ZKsync OS
        Some(vec![]), // ignored in ZKsync OS
        None,         // ignored in ZKsync OS
        tx.factory_deps.clone(),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn encode_abi_tx(
    tx_type: u8,
    from: [u8; 20],
    to: Option<[u8; 20]>,
    gas_limit: u128,
    gas_per_pubdata_byte_limit: Option<u128>,
    max_fee_per_gas: u128,
    max_priority_fee_per_gas: Option<u128>,
    paymaster: Option<[u8; 20]>,
    nonce: u128,
    value: [u8; 32],
    reserved: [U256; 4],
    data: Vec<u8>,
    signature: Vec<u8>,
    paymaster_input: Option<Vec<u8>>,
    reserved_dynamic: Option<Vec<u8>>,
    factory_deps: Vec<B256>,
) -> EncodedTx {
    let bytes = DynSolValue::Tuple(vec![
        U256::from(tx_type).into(),
        address_to_value(&from),
        address_to_value(&to.unwrap_or_default()),
        U256::from(gas_limit).into(),
        gas_per_pubdata_byte_limit.unwrap_or_default().into(),
        max_fee_per_gas.into(),
        max_priority_fee_per_gas.unwrap_or(max_fee_per_gas).into(),
        address_to_value(&paymaster.unwrap_or_default()),
        U256::from(nonce).into(),
        U256::from_be_bytes(value).into(),
        DynSolValue::FixedArray(reserved.map(|r| r.into()).to_vec()),
        DynSolValue::Bytes(data),
        DynSolValue::Bytes(signature),
        DynSolValue::Array(factory_deps.into_iter().map(|r| r.into()).collect()),
        DynSolValue::Bytes(paymaster_input.unwrap_or_default()),
        DynSolValue::Bytes(reserved_dynamic.unwrap_or_default()),
    ])
    .abi_encode_params();
    EncodedTx::Abi(bytes)
}

fn encode_service_tx_from_tx(tx: &ZKsyncServiceTx) -> EncodedTx {
    let tx_type = tx.ty();
    let bytes = DynSolValue::Tuple(vec![
        U256::from(tx_type).into(),
        address_to_value(&tx.to.into_array()),
        DynSolValue::Bytes(tx.input.to_vec()),
    ])
    .abi_encode_params();
    EncodedTx::Abi(bytes)
}

fn address_to_value(address: &[u8; 20]) -> DynSolValue {
    let mut padded = [0u8; 32];
    padded[12..].copy_from_slice(address.as_slice());
    U256::from_be_bytes(padded).into()
}
