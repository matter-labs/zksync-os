use crate::bootloader::transaction::ethereum_tx_format::eip_2930_tx::AccessList;
use crate::bootloader::transaction::ethereum_tx_format::eip_2930_tx::RLPListOfHomogeneousItems;
use crate::bootloader::transaction::ethereum_tx_format::minimal_rlp_parser::{
    ListEncapsulated, Parser, RLPParsable,
};
use ruint::aliases::U256;

#[derive(Clone, Copy, Debug)]
pub struct AuthorizationEntry<'a> {
    pub chain_id: U256,
    pub address: &'a [u8; 20], // NOTE: Can not be empty
    pub nonce: u64,
    pub y_parity: u8, // not bool
    pub r: &'a [u8],  // not fixed size
    pub s: &'a [u8],  // not fixed size
}

impl<'a> RLPParsable<'a> for AuthorizationEntry<'a> {
    fn try_parse(parser: &mut Parser<'a>) -> Result<Self, ()> {
        let chain_id = RLPParsable::try_parse(parser)?;
        let address = RLPParsable::try_parse(parser)?;
        let nonce = RLPParsable::try_parse(parser)?;
        let y_parity = RLPParsable::try_parse(parser)?;
        let r = RLPParsable::try_parse(parser)?;
        let s = RLPParsable::try_parse(parser)?;

        let new = Self {
            chain_id,
            address,
            nonce,
            y_parity,
            r,
            s,
        };

        Ok(new)
    }
}

pub type AuthorizationList<'a> =
    RLPListOfHomogeneousItems<'a, ListEncapsulated<'a, AuthorizationEntry<'a>>, true>;

#[derive(Clone, Copy, Debug)]
pub(crate) struct EIP7702Tx<'a> {
    pub(crate) chain_id: u64,
    pub(crate) nonce: u64,
    pub(crate) max_priority_fee_per_gas: U256,
    pub(crate) max_fee_per_gas: U256,
    pub(crate) gas_limit: u64,
    pub(crate) to: &'a [u8; 20], // NOTE: Can not be empty
    pub(crate) value: U256,
    pub(crate) data: &'a [u8],
    pub(crate) access_list: AccessList<'a>,
    pub(crate) authorization_list: AuthorizationList<'a>,
}

impl<'a> RLPParsable<'a> for EIP7702Tx<'a> {
    fn try_parse(parser: &mut Parser<'a>) -> Result<Self, ()> {
        let chain_id = RLPParsable::try_parse(parser)?;
        let nonce = RLPParsable::try_parse(parser)?;
        let max_priority_fee_per_gas = RLPParsable::try_parse(parser)?;
        let max_fee_per_gas = RLPParsable::try_parse(parser)?;
        let gas_limit = RLPParsable::try_parse(parser)?;
        let to = RLPParsable::try_parse(parser)?;
        let value = RLPParsable::try_parse(parser)?;
        let data = RLPParsable::try_parse(parser)?;
        let access_list = RLPParsable::try_parse(parser)?;
        let authorization_list = RLPParsable::try_parse(parser)?;

        let new = Self {
            chain_id,
            nonce,
            max_priority_fee_per_gas,
            max_fee_per_gas,
            gas_limit,
            to,
            value,
            data,
            access_list,
            authorization_list,
        };

        Ok(new)
    }
}
