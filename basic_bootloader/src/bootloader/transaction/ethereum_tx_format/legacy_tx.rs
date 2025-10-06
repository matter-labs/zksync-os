use crate::bootloader::transaction::ethereum_tx_format::minimal_rlp_parser::RLPParsable;
use crate::bootloader::transaction::ethereum_tx_format::Parser;
use ruint::aliases::U256;

#[derive(Clone, Copy, Debug)]
pub struct LegacyTXInner<'a> {
    pub nonce: u64,
    pub gas_price: U256,
    pub gas_limit: u64,
    pub to: &'a [u8],
    pub value: U256,
    pub data: &'a [u8],
}

impl<'a> RLPParsable<'a> for LegacyTXInner<'a> {
    fn try_parse(parser: &mut Parser<'a>) -> Result<Self, ()> {
        let nonce = RLPParsable::try_parse(parser)?;
        let gas_price = RLPParsable::try_parse(parser)?;
        let gas_limit = RLPParsable::try_parse(parser)?;
        let to: &'a [u8] = RLPParsable::try_parse(parser)?;
        if !(to.len() == 0 || to.len() == 20) {
            return Err(());
        }
        let value = RLPParsable::try_parse(parser)?;
        let data = RLPParsable::try_parse(parser)?;

        let new = Self {
            nonce,
            gas_price,
            gas_limit,
            to,
            value,
            data,
        };

        Ok(new)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct LegacySignatureData<'a> {
    pub(crate) v: u64,
    pub(crate) r: &'a [u8],
    pub(crate) s: &'a [u8],
}

impl<'a> RLPParsable<'a> for LegacySignatureData<'a> {
    fn try_parse(parser: &mut Parser<'a>) -> Result<Self, ()> {
        let v = RLPParsable::try_parse(parser)?;
        let r = RLPParsable::try_parse(parser)?;
        let s = RLPParsable::try_parse(parser)?;

        let new = Self { v, r, s };

        Ok(new)
    }
}
