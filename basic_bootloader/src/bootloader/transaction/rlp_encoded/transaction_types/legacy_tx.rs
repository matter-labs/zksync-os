use crate::bootloader::errors::{InvalidTransaction, TxError};
use crate::bootloader::transaction::rlp_encoded::transaction_types::EthereumTxType;

use crypto::MiniDigest;

use crate::bootloader::transaction::rlp_encoded::rlp::minimal_rlp_parser::{Rlp, RlpListDecode};
use crate::bootloader::transaction::rlp_encoded::rlp::{
    apply_list_concatenation_encoding_to_hash, apply_u64_encoding_to_hash, u64_encoding_len,
};
use ruint::aliases::U256;
use zk_ee::utils::Bytes32;

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

impl<'a> EthereumTxType for LegacyTXInner<'a> {
    const TX_TYPE: u8 = 0;
}

impl<'a> RlpListDecode<'a> for LegacyTXInner<'a> {
    /// Decode the 6-field legacy tx list body:
    /// [nonce, gasPrice, gasLimit, to, value, data]
    fn decode_list_body(r: &mut Rlp<'a>) -> Result<Self, InvalidTransaction> {
        let nonce = r.u64()?;
        let gas_price = r.u256()?;
        let gas_limit = r.u64()?;

        let to = {
            let s = r.bytes()?;
            if s.is_empty() || s.len() == 20 {
                s
            } else {
                return Err(InvalidTransaction::InvalidStructure);
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

pub(crate) struct LegacyPayloadParser {}

impl LegacyPayloadParser {
    pub(crate) fn try_parse_and_hash_for_signature_verification<'a>(
        src: &'a [u8],
        expected_chain_id: u64,
    ) -> Result<(LegacyTXInner<'a>, LegacySignatureData<'a>, Bytes32), TxError> {
        // Legacy path: input must be a single list with 9 elements total.
        let mut outer = Rlp::new(src);

        // Strip the list encoding
        let mut inner = outer.list()?;

        // Outer list must be fully consumed
        if !outer.is_empty() {
            return Err(InvalidTransaction::InvalidStructure.into());
        }

        // Capture the concatenation bytes of the first 6 fields for hashing.
        let mark = inner.mark();
        let legacy_inner: LegacyTXInner<'a> = LegacyTXInner::decode_list_body(&mut inner)?;
        let inner_slice = inner.consumed_since(mark);

        let legacy_signature = LegacySignatureData::decode_list_body(&mut inner)?;
        if !inner.is_empty() {
            return Err(InvalidTransaction::InvalidStructure.into());
        }

        let sig_hash: Bytes32 = if legacy_signature.is_eip155() == false {
            // Unprotected legacy
            let mut hasher = crypto::sha3::Keccak256::new();
            apply_list_concatenation_encoding_to_hash(inner_slice.len() as u32, &mut hasher);
            hasher.update(inner_slice);
            hasher.finalize_reset().into()
        } else {
            // EIP-155 protected legacy: v must match 35 + 2*chainId (+ {0,1})
            let min_v = U256::from(35) + U256::from(expected_chain_id) * U256::from(2);
            if !(legacy_signature.v == min_v || legacy_signature.v == min_v + U256::ONE) {
                return Err(InvalidTransaction::InvalidEncoding.into());
            }

            // Compute signing hash over the 6-field payload plus chainId and two empty strings.
            let chain_id = expected_chain_id;
            let chain_id_encoding_len = u64_encoding_len(chain_id);

            let mut hasher = crypto::sha3::Keccak256::new();
            apply_list_concatenation_encoding_to_hash(
                (inner_slice.len() + chain_id_encoding_len + 2) as u32, // 0x80, 0x80 for r/s
                &mut hasher,
            );
            hasher.update(inner_slice);
            apply_u64_encoding_to_hash(chain_id, &mut hasher);
            hasher.update(&[0x80, 0x80]);
            hasher.finalize_reset().into()
        };

        Ok((legacy_inner, legacy_signature, sig_hash))
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct LegacySignatureData<'a> {
    pub(crate) v: U256,
    pub(crate) r: &'a [u8],
    pub(crate) s: &'a [u8],
}

impl<'a> LegacySignatureData<'a> {
    pub fn is_eip155(&self) -> bool {
        self.v != 27 && self.v != 28
    }
}

impl<'a> RlpListDecode<'a> for LegacySignatureData<'a> {
    fn decode_list_body(r: &mut Rlp<'a>) -> Result<Self, InvalidTransaction> {
        let v = r.u256()?;
        let r_bytes = r.bytes()?;
        let s = r.bytes()?;
        // Check that r and s are at most 32 bytes each, and are not
        // non-canonically encoded (no leading zeroes).
        if r_bytes.len() > 32
            || s.len() > 32
            || (!r_bytes.is_empty() && r_bytes[0] == 0)
            || (!s.is_empty() && s[0] == 0)
        {
            return Err(InvalidTransaction::InvalidStructure);
        }
        let new = Self { v, r: r_bytes, s };
        Ok(new)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::bootloader::transaction::rlp_encoded::rlp::minimal_rlp_parser::RlpListDecode;

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

    fn malformed_sig_rlp_r_33_s_31() -> Vec<u8> {
        let v = 0x1b_u8;
        let r_payload = [0x11u8; 33];
        let s_payload = [0x22u8; 31];

        let mut payload = Vec::new();
        payload.push(v);
        payload.push(0x80 + r_payload.len() as u8);
        payload.extend_from_slice(&r_payload);
        payload.push(0x80 + s_payload.len() as u8);
        payload.extend_from_slice(&s_payload);

        let mut out = Vec::new();
        out.push(0xf8);
        out.push(payload.len() as u8);
        out.extend_from_slice(&payload);
        out
    }

    #[test]
    fn rejects_too_long_signature_fields() {
        // Regression: both r and s should be at most 32 bytes each.
        let bytes = malformed_sig_rlp_r_33_s_31();
        LegacySignatureData::decode_list_full(&bytes).expect_err("Parsing should fail");
    }

    fn malformed_sig_rlp_r_leading_zeroes() -> Vec<u8> {
        let v = 0x1b_u8;
        let r_payload = [0x00, 0x11];
        let s_payload = [0x22u8; 31];

        let mut payload = Vec::new();
        payload.push(v);
        payload.push(0x80 + r_payload.len() as u8);
        payload.extend_from_slice(&r_payload);
        payload.push(0x80 + s_payload.len() as u8);
        payload.extend_from_slice(&s_payload);

        let mut out = Vec::new();
        out.push(0xf8);
        out.push(payload.len() as u8);
        out.extend_from_slice(&payload);
        out
    }

    #[test]
    fn rejects_sig_leading_zeroes() {
        // Regression: leading zeroes in r or s are not allowed.
        let bytes = malformed_sig_rlp_r_leading_zeroes();
        LegacySignatureData::decode_list_full(&bytes).expect_err("Parsing should fail");
    }

    #[test]
    fn rejects_outer_with_more_than_list() {
        // Regression: outer RLP must be a single list only.
        let mut encoded = hex::decode("f901ab820215840cc9aa6c82ca9c94bf7cf0d775d6ac130912a22861773c21661095a280b90144baae8abf0000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000200000000000000000000000008f3ffa11cd5915f0e869192663b905504a2ef4a500000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000064d22c0930000000000000000000000000953ca96b057d5397ce7791c5ae9b5a19b135234100000000000000000000000000000000000000000000000000000000000f424000000000000000000000000000000000000000000000000000000000689010fd0000000000000000000000000000000000000000000000000000000026a005b37d188e6af6851c1036a5c42113ada300c03403d340d4c9ba8102146e9a76a0471b7967f289f3248f4250d0dbcb8e7391ea0b9252385377909911420f164db7").unwrap();

        encoded.push(0x00); // extra byte at the end
        let res = LegacyPayloadParser::try_parse_and_hash_for_signature_verification(&encoded, 1);

        assert!(
            res.is_err(),
            "trailing bytes after the outer RLP list must cause a parse error"
        );
    }
}
