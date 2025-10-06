use crate::bootloader::transaction::ethereum_tx_format::{
    apply_list_concatenation_encoding_to_hash,
    minimal_rlp_parser::{Parser, RLPParsable},
};
use crypto::MiniDigest;

pub(crate) struct EIP2718PayloadParser<'a, P: RLPParsable<'a>> {
    _marker: core::marker::PhantomData<&'a P>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct EIP2718SignatureData<'a> {
    pub(crate) y_parity: bool,
    pub(crate) r: &'a [u8],
    pub(crate) s: &'a [u8],
}

impl<'a> RLPParsable<'a> for EIP2718SignatureData<'a> {
    fn try_parse(parser: &mut Parser<'a>) -> Result<Self, ()> {
        let y_parity = RLPParsable::try_parse(parser)?;
        let r = RLPParsable::try_parse(parser)?;
        let s = RLPParsable::try_parse(parser)?;

        let new = Self { y_parity, r, s };

        Ok(new)
    }
}

impl<'a, P: RLPParsable<'a>> EIP2718PayloadParser<'a, P> {
    /// We expect that transaction type was already placed into hasher. Will try to parse
    /// P, and the try to parse signature manually
    /// NOTE: double hashing is inevitable, as signature is verified upon keccak256(0x01 || rlp([chainId, nonce, gasPrice, gasLimit, to, value, data, accessList])),
    /// while for indexing purposes divergence starts at the very start as RLP pre-encodes total length
    pub(crate) fn try_parse_and_hash_for_signature_verification(
        src: &'a [u8],
        hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
    ) -> Result<(P, EIP2718SignatureData<'a>), ()> {
        let mut outer_parser = Parser::new(src);
        // quick and dirty strip of the list encoding
        let mut parser = outer_parser.try_make_list_subparser()?;
        if outer_parser.is_empty() == false {
            return Err(());
        }
        // recreate parser to parse internals
        let start = unsafe { parser.pos() };
        let payload: P = RLPParsable::try_parse(&mut parser)?;
        // we consumed P, and are ready to compute the hash for signing
        let inner_slice = unsafe { parser.consumed_slice(start) };

        // now we can use the same parser and parse fixed "tail" of parity/r/s
        let sig_data: EIP2718SignatureData<'a> = RLPParsable::try_parse(&mut parser)?;
        if parser.is_empty() == false {
            return Err(());
        }

        apply_list_concatenation_encoding_to_hash(inner_slice.len() as u32, hasher);
        hasher.update(inner_slice);

        Ok((payload, sig_data))
    }
}
