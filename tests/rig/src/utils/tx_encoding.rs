use alloy::{
    consensus::{SignableTransaction, TxEnvelope, TxLegacy},
    eips::Encodable2718,
    network::TxSignerSync,
    primitives::Address,
    rpc::types::TransactionRequest,
    signers::local::PrivateKeySigner,
};
use zksync_os_api::helpers::{encode_envelope_2718, encode_tx};
use zksync_os_interface::traits::EncodedTx;
use zksync_os_tests_common::zksync_tx::{ZKsyncTxRequest, ZKsyncTxType};

pub trait EncodableToEncodedTx {
    fn encode(&self) -> EncodedTx;
}

impl EncodableToEncodedTx for ZKsyncTxRequest {
    fn encode(&self) -> EncodedTx {
        match self.tx_type {
            ZKsyncTxType::L1 => encode_l1_tx(self.inner.clone()),
            ZKsyncTxType::Upgrade => encode_upgrade_tx(self.inner.clone()),
            ZKsyncTxType::L2(_) => encode_and_sign_as_legacy_l2(
                self.inner.clone(),
                self.signer
                    .as_ref()
                    .expect("L2 transactions must have a signer"),
            ),
            ZKsyncTxType::Service => unimplemented!("service transactions are not supported yet"),
            ZKsyncTxType::Custom(tx_type) => encode_special_tx_type(self.inner.clone(), tx_type),
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

// If you want “raw tx bytes” to send via eth_sendRawTransaction:
fn encode_and_sign_as_legacy_l2(req: TransactionRequest, signer: &PrivateKeySigner) -> EncodedTx {
    let mut tx: TxLegacy = req.build_legacy().expect("Should build");

    //let signer = signer.with_chain_id(Some(chain_id));

    let sig = signer.sign_transaction_sync(&mut tx).expect("Should sign");

    // Turn it into a signed tx envelope; then encode as EIP-2718 bytes.
    let signed = tx.into_signed(sig);
    EncodedTx::Rlp(signed.encoded_2718(), signer.address())
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
