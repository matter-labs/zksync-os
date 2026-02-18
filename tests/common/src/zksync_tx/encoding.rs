use alloy::{
    consensus::TxEnvelope,
    dyn_abi::DynSolValue,
    eips::Encodable2718,
    primitives::{address, Address, B256, U256},
    rlp::BufMut,
    rpc::types::TransactionRequest,
};
use zksync_os_interface::traits::EncodedTx;

use crate::zksync_tx::{ZKsyncSpecificTxEnvelope, ZKsyncTxEnvelope};

pub trait AbiEncodableTx {
    fn abi_encode(&self, out: &mut dyn BufMut);
}

pub trait ZKsyncOsEncodable {
    fn encode(self) -> EncodedTx;
}

pub const BOOTLOADER_FORMAL_ADDRESS: Address =
    address!("0x0000000000000000000000000000000000008001");

impl ZKsyncOsEncodable for ZKsyncTxEnvelope {
    fn encode(self) -> EncodedTx {
        match self {
            ZKsyncTxEnvelope::Ethereum(ethereum_tx_envelope, signer) => {
                encode_2718_tx_envelope(ethereum_tx_envelope, signer)
            }
            ZKsyncTxEnvelope::ZKsync(zksync_specific_tx_envelope) => {
                match zksync_specific_tx_envelope {
                    ZKsyncSpecificTxEnvelope::L1(zksync_l1_tx) => {
                        encode_abi_tx_envelope(zksync_l1_tx)
                    }
                    ZKsyncSpecificTxEnvelope::Upgrade(zksync_upgrade_tx) => {
                        encode_abi_tx_envelope(zksync_upgrade_tx)
                    }
                    ZKsyncSpecificTxEnvelope::Service(zksync_service_tx) => {
                        encode_2718_tx_envelope(zksync_service_tx, BOOTLOADER_FORMAL_ADDRESS)
                    }
                }
            }
            ZKsyncTxEnvelope::Custom(custom_type, tx_req) => {
                encode_special_tx_type(tx_req.clone(), custom_type)
            }
        }
    }
}

impl ZKsyncOsEncodable for alloy::rpc::types::Transaction {
    #[allow(deprecated)]
    fn encode(self) -> EncodedTx {
        let from = self.as_recovered().signer().into_array();
        let env: TxEnvelope = self.into();
        encode_2718_tx_envelope(env, Address::from_slice(&from))
    }
}

fn encode_abi_tx_envelope<T: AbiEncodableTx>(tx_envelope: T) -> EncodedTx {
    let mut bytes = vec![];
    tx_envelope.abi_encode(&mut bytes);
    EncodedTx::Abi(bytes)
}

fn encode_2718_tx_envelope<T: Encodable2718>(tx_envelope: T, signer: Address) -> EncodedTx {
    let mut bytes = vec![];
    tx_envelope.encode_2718(&mut bytes);
    EncodedTx::Rlp(bytes, signer)
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
            U256::from(gas_limit * max_fee_per_gas)
        } else {
            U256::ZERO
        }),
        (if to.is_none() { U256::ONE } else { U256::ZERO }),
        U256::ZERO,
        U256::ZERO,
    ];

    let bytes = encode_abi_tx(
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
    );
    EncodedTx::Abi(bytes)
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
) -> Vec<u8> {
    DynSolValue::Tuple(vec![
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
    .abi_encode_params()
}

fn address_to_value(address: &[u8; 20]) -> DynSolValue {
    let mut padded = [0u8; 32];
    padded[12..].copy_from_slice(address.as_slice());
    U256::from_be_bytes(padded).into()
}

pub fn encode_alloy_rpc_tx(tx: alloy::rpc::types::Transaction) -> EncodedTx {
    let from = tx.as_recovered().signer().into_array();
    encode_2718_tx_envelope(tx.inner, Address::from_slice(&from))
}
