use super::minimal_rlp_parser::{Rlp, RlpListDecode};
use ruint::aliases::U256;

/// Legacy (type 0x00) inner payload used for signing:
/// [nonce, gasPrice, gasLimit, to, value, data]
/// `to` must be empty for contract creation or exactly 20 bytes for a call.
#[derive(Clone, Copy, Debug)]
pub struct LegacyTXInner<'a> {
    pub nonce: u64,
    pub gas_price: U256,
    pub gas_limit: u64,
    pub to: &'a [u8],
    pub value: U256,
    pub data: &'a [u8],
}

impl<'a> RlpListDecode<'a> for LegacyTXInner<'a> {
    /// Decode the 6-field legacy tx list body:
    /// [nonce, gasPrice, gasLimit, to, value, data]
    fn decode_list_body(r: &mut Rlp<'a>) -> Result<Self, ()> {
        let nonce = r.u64()?;
        let gas_price = r.u256()?;
        let gas_limit = r.u64()?;

        let to = {
            let s = r.bytes()?;
            if s.is_empty() || s.len() == 20 {
                s
            } else {
                return Err(());
            }
        };

        let value = r.u256()?;
        let data = r.bytes()?;

        Ok(Self {
            nonce,
            gas_price,
            gas_limit,
            to,
            value,
            data,
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct LegacySignatureData<'a> {
    pub(crate) v: u64,
    pub(crate) r: &'a [u8],
    pub(crate) s: &'a [u8],
}

impl<'a> RlpListDecode<'a> for LegacySignatureData<'a> {
    fn decode_list_body(r: &mut Rlp<'a>) -> Result<Self, ()> {
        let v = r.u64()?;
        let r_bytes = r.bytes()?;
        let s = r.bytes()?;
        let new = Self { v, r: r_bytes, s };
        Ok(new)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::bootloader::transaction::ethereum_tx_format::minimal_rlp_parser::RlpListDecode;

    // Alloy imports
    use alloy::consensus::TxLegacy;
    use alloy_primitives::{address, Address, Bytes, TxKind, U256};
    use alloy_rlp::Encodable;

    use ruint::aliases::U256 as RuintU256;

    fn alloy_legacy_payload_transfer(
        nonce: u64,
        gas_price: u128,
        gas_limit: u64,
        to_addr: Address,
        value: u128,
        data: Bytes,
    ) -> Vec<u8> {
        let tx = TxLegacy {
            chain_id: None, // unprotected payload: 6-field list
            nonce,
            gas_price,
            gas_limit,
            to: TxKind::Call(to_addr),
            value: U256::from(value),
            input: data,
        };
        let mut out = Vec::new();
        tx.encode(&mut out);
        out
    }

    fn alloy_legacy_payload_create(
        nonce: u64,
        gas_price: u128,
        gas_limit: u64,
        value: u128,
        initcode: Bytes,
    ) -> Vec<u8> {
        let tx = TxLegacy {
            chain_id: None, // unprotected payload: 6-field list
            nonce,
            gas_price,
            gas_limit,
            to: TxKind::Create,
            value: U256::from(value),
            input: initcode,
        };
        let mut out = Vec::new();
        tx.encode(&mut out);
        out
    }

    #[test]
    fn parses_legacy_transfer_from_alloy_payload() {
        let to = address!("0x1111111111111111111111111111111111111111");
        let value = 4242u128;
        let data = Bytes::new();

        let bytes = alloy_legacy_payload_transfer(
            9,          // nonce
            50_000_000, // gas_price
            21_000,     // gas_limit
            to,
            value,
            data.clone(),
        );

        let tx: LegacyTXInner =
            RlpListDecode::decode_list_full(&bytes).expect("parse should succeed");

        assert_eq!(tx.nonce, 9);
        assert_eq!(tx.gas_limit, 21_000);
        assert_eq!(tx.gas_price, RuintU256::from(50_000_000u128));

        assert_eq!(tx.to.len(), 20);
        assert_eq!(tx.to, to.as_slice());

        assert_eq!(tx.value, RuintU256::from(value));
        assert_eq!(tx.data, &*data);
    }

    #[test]
    fn parses_legacy_create_from_alloy_payload() {
        let initcode = Bytes::from(vec![0x60, 0x60, 0x60, 0x40, 0x52]);

        let bytes = alloy_legacy_payload_create(
            0,             // nonce
            1_000_000_000, // gas_price
            1_000_000,     // gas_limit
            0,             // value
            initcode.clone(),
        );

        let tx: LegacyTXInner =
            RlpListDecode::decode_list_full(&bytes).expect("parse should succeed");

        assert_eq!(tx.to.len(), 0, "contract creation must have empty `to`");
        assert_eq!(tx.data, &*initcode);
    }
}
