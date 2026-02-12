use alloy::{
    consensus::TxEnvelope, primitives::Address, rpc::types::TransactionRequest,
    signers::local::PrivateKeySigner,
};
use zksync_os_api::helpers::{encode_envelope_2718, encode_tx};
use zksync_os_interface::traits::EncodedTx;
use zksync_os_tests_common::zksync_tx::{ZKsyncSpecificTxType, ZKsyncTxEnvelope, ZKsyncTxType};

pub trait EncodableToEncodedTx {
    fn encode(&self) -> EncodedTx;
}

impl EncodableToEncodedTx for ZKsyncTxEnvelope {
    fn encode(&self) -> EncodedTx {
        match &self.inner {
            ZKsyncTxType::ZKsync(zksync_tx_type, tx_req) => match zksync_tx_type {
                ZKsyncSpecificTxType::L1 => encode_l1_tx(tx_req.clone()),
                ZKsyncSpecificTxType::Upgrade => encode_upgrade_tx(tx_req.clone()),
                ZKsyncSpecificTxType::Service => {
                    unimplemented!("service transactions are not supported yet")
                }
                ZKsyncSpecificTxType::Custom(custom_type) => {
                    encode_special_tx_type(tx_req.clone(), *custom_type)
                }
            },
            ZKsyncTxType::Ethereum(ethereum_tx_envelope) => encode_ethereum_tx_envelope(
                ethereum_tx_envelope,
                self.signer
                    .as_ref()
                    .expect("L2 transactions must have a signer"),
            ),
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

///
/// Encode given request as l1 -> l2 transaction.
///
/// Panics if needed fields are unset/set incorrectly.
///
fn encode_l1_tx(tx: TransactionRequest) -> EncodedTx {
    let tx_type = 0x7f;
    encode_special_tx_type(tx, tx_type)
}

///
/// Encode given request as an upgrade transaction.
///
/// Panics if needed fields are unset/set incorrectly.
///
fn encode_upgrade_tx(tx: TransactionRequest) -> EncodedTx {
    let tx_type = 0x7e;
    encode_special_tx_type(tx, tx_type)
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

    encode_tx(
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
        data,
        signature,
        paymaster_input,
        None,
        true,
    )
}
