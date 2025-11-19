use crate::bootloader::{
    errors::{InvalidTransaction, TxError},
    transaction::rlp_encoded::{
        rlp::{
            apply_list_concatenation_encoding_to_hash,
            minimal_rlp_parser::{Rlp, RlpListDecode},
        },
        transaction_types::EthereumTxType,
    },
};
use crypto::MiniDigest;
use zk_ee::utils::Bytes32;

/// Parser for typed EIP-2718 transactions where the payload (P) and signature
/// are encoded as two consecutive list items inside a single outer list:
/// outer = [ payload_list(P), signature_list(yParity, r, s) ]
pub(crate) struct EIP2718PayloadParser<'a, P: RlpListDecode<'a> + EthereumTxType> {
    _marker: core::marker::PhantomData<&'a P>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct EIP2718SignatureData<'a> {
    pub(crate) y_parity: bool,
    pub(crate) r: &'a [u8],
    pub(crate) s: &'a [u8],
}

impl<'a> RlpListDecode<'a> for EIP2718SignatureData<'a> {
    fn decode_list_body(r: &mut Rlp<'a>) -> Result<Self, InvalidTransaction> {
        let y_parity = r.bool()?;
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
        let new = Self {
            y_parity,
            r: r_bytes,
            s,
        };
        Ok(new)
    }
}

impl<'a, P: RlpListDecode<'a> + EthereumTxType> EIP2718PayloadParser<'a, P> {
    /// Will try to parse P, and the try to parse signature manually
    /// NOTE: double hashing is inevitable, as signature is verified upon keccak256(0x01 || rlp([chainId, nonce, gasPrice, gasLimit, to, value, data, accessList])),
    /// while for indexing purposes divergence starts at the very start as RLP pre-encodes total length
    pub(crate) fn try_parse_and_hash_for_signature_verification(
        src: &'a [u8],
    ) -> Result<(P, EIP2718SignatureData<'a>, Bytes32), TxError> {
        let mut outer = Rlp::new(src);
        // Strip the list encoding
        let mut inner = outer.list()?;
        // Outer list must be fully consumed
        if !outer.is_empty() {
            return Err(InvalidTransaction::InvalidStructure.into());
        }
        // Take mark to include payload for hashing
        let mark = inner.mark();
        // Parse payload part (transaction fields without signature)
        let payload = P::decode_list_body(&mut inner)?;
        let inner_slice = inner.consumed_since(mark);

        // Parse signature suffix [yParity, r, s] from same parser
        let sig = EIP2718SignatureData::decode_list_body(&mut inner)?;

        if !inner.is_empty() {
            return Err(InvalidTransaction::InvalidStructure.into());
        }

        let mut hasher = crypto::sha3::Keccak256::new();
        hasher.update(&[P::TX_TYPE]);

        // Hash payload list header + payload bytes.
        // Caller already hashed the type byte.
        apply_list_concatenation_encoding_to_hash(inner_slice.len() as u32, &mut hasher);
        hasher.update(inner_slice);
        let sig_hash = hasher.finalize().into();

        Ok((payload, sig, sig_hash))
    }
}

#[cfg(test)]
mod tests {
    use crate::bootloader::transaction::rlp_encoded::rlp::minimal_rlp_parser::RlpListDecode;
    use crate::bootloader::transaction::rlp_encoded::EIP2718SignatureData;

    fn malformed_sig_rlp_r_33_s_31() -> Vec<u8> {
        let v = 0x01_u8;
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
        EIP2718SignatureData::decode_list_full(&bytes).expect_err("Parsing should fail");
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
        EIP2718SignatureData::decode_list_full(&bytes).expect_err("Parsing should fail");
    }
}
