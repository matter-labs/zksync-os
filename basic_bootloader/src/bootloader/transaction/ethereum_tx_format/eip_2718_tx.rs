use crate::bootloader::transaction::ethereum_tx_format::{
    apply_list_concatenation_encoding_to_hash,
    minimal_rlp_parser::{Rlp, RlpListDecode},
};
use crypto::MiniDigest;

/// Parser for typed EIP-2718 transactions where the payload (P) and signature
/// are encoded as two consecutive list items inside a single outer list:
/// outer = [ payload_list(P), signature_list(yParity, r, s) ]
pub(crate) struct EIP2718PayloadParser<'a, P: RlpListDecode<'a>> {
    _marker: core::marker::PhantomData<&'a P>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct EIP2718SignatureData<'a> {
    pub(crate) y_parity: bool,
    pub(crate) r: &'a [u8],
    pub(crate) s: &'a [u8],
}

impl<'a> RlpListDecode<'a> for EIP2718SignatureData<'a> {
    fn decode_list_body(r: &mut Rlp<'a>) -> Result<Self, ()> {
        let y_parity = r.bool()?;
        let r_bytes = r.bytes()?;
        let s = r.bytes()?;
        let new = Self {
            y_parity,
            r: r_bytes,
            s,
        };
        Ok(new)
    }
}

impl<'a, P: RlpListDecode<'a>> EIP2718PayloadParser<'a, P> {
    /// We expect that transaction type was already placed into hasher. Will try to parse
    /// P, and the try to parse signature manually
    /// NOTE: double hashing is inevitable, as signature is verified upon keccak256(0x01 || rlp([chainId, nonce, gasPrice, gasLimit, to, value, data, accessList])),
    /// while for indexing purposes divergence starts at the very start as RLP pre-encodes total length
    pub(crate) fn try_parse_and_hash_for_signature_verification(
        src: &'a [u8],
        hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
    ) -> Result<(P, EIP2718SignatureData<'a>), ()> {
        let mut outer = Rlp::new(src);
        // Strip the list encoding
        let mut inner = outer.list()?;
        // Outer list must be fully consumed
        if !outer.is_empty() {
            return Err(());
        }
        // Take mark to include payload for hashing
        let mark = inner.mark();
        // Parse payload part (transaction fields without signature)
        let payload = P::decode_list_body(&mut inner)?;
        let inner_slice = inner.consumed_since(mark);

        // Parse signature suffix [yParity, r, s] from same parser
        let sig = EIP2718SignatureData::decode_list_body(&mut inner)?;

        if !inner.is_empty() {
            return Err(());
        }

        // Hash payload list header + payload bytes.
        // Caller already hashed the type byte.
        apply_list_concatenation_encoding_to_hash(inner_slice.len() as u32, hasher);
        hasher.update(inner_slice);

        Ok((payload, sig))
    }
}
