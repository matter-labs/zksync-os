use crate::bootloader::transaction::ethereum_tx_format::minimal_rlp_parser::ListEncapsulated;
use crate::bootloader::transaction::ethereum_tx_format::minimal_rlp_parser::{
    FixedLenScalar, Parser, RLPParsable,
};
use ruint::aliases::B160;
use ruint::aliases::U256;

#[derive(Clone, Copy, Debug)]
pub(crate) struct EIP2930Tx<'a> {
    pub(crate) chain_id: u64,
    pub(crate) nonce: u64,
    pub(crate) gas_price: U256,
    pub(crate) gas_limit: u64,
    pub(crate) to: &'a [u8], // NOTE: it may be empty for deployments
    pub(crate) value: U256,
    pub(crate) data: &'a [u8],
    pub(crate) access_list: AccessList<'a>,
}

impl<'a> RLPParsable<'a> for EIP2930Tx<'a> {
    fn try_parse(parser: &mut Parser<'a>) -> Result<Self, ()> {
        let chain_id = RLPParsable::try_parse(parser)?;
        let nonce = RLPParsable::try_parse(parser)?;
        let gas_price = RLPParsable::try_parse(parser)?;
        let gas_limit = RLPParsable::try_parse(parser)?;
        let to: &'a [u8] = RLPParsable::try_parse(parser)?;
        if !(to.len() == 0 || to.len() == 20) {
            return Err(());
        }
        let value = RLPParsable::try_parse(parser)?;
        let data = RLPParsable::try_parse(parser)?;
        let access_list = RLPParsable::try_parse(parser)?;

        let new = Self {
            chain_id,
            nonce,
            gas_price,
            gas_limit,
            to,
            value,
            data,
            access_list,
        };

        Ok(new)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RLPListOfFixedLengthItems<'a, T: FixedLenScalar<'a>> {
    parser: Parser<'a>,
    pub count: usize,
    _marker: core::marker::PhantomData<T>,
}

#[derive(Clone, Copy, Debug)]
pub struct RLPListOfFixedLengthItemsIter<'a, T: FixedLenScalar<'a>> {
    parser: Parser<'a>,
    len: usize,
    _marker: core::marker::PhantomData<T>,
}

impl<'a, T: FixedLenScalar<'a>> RLPParsable<'a> for RLPListOfFixedLengthItems<'a, T> {
    fn try_parse(parser: &mut Parser<'a>) -> Result<Self, ()> {
        // from the parser - it's itself a list
        let parser = parser.try_make_list_subparser()?;

        if parser.slice.len() % T::ENCODING_LEN != 0 {
            return Err(());
        }
        let count = parser.slice.len() / T::ENCODING_LEN;

        let new = Self {
            parser,
            count,
            _marker: core::marker::PhantomData,
        };

        Ok(new)
    }
}

impl<'a, T: FixedLenScalar<'a>> RLPListOfFixedLengthItems<'a, T> {
    pub fn iter(&self) -> RLPListOfFixedLengthItemsIter<'a, T> {
        RLPListOfFixedLengthItemsIter {
            parser: self.parser,
            len: self.count,
            _marker: core::marker::PhantomData,
        }
    }
}

impl<'a, T: FixedLenScalar<'a>> Iterator for RLPListOfFixedLengthItemsIter<'a, T> {
    type Item = Result<T, ()>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.len == 0 {
            None
        } else {
            self.len -= 1;
            let item = self
                .parser
                .try_parse_slice()
                .and_then(|slice| T::try_parse(slice));
            Some(item)
        }
    }
}

impl<'a, T: FixedLenScalar<'a>> ExactSizeIterator for RLPListOfFixedLengthItemsIter<'a, T> {
    fn len(&self) -> usize {
        self.len
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RLPListOfHomogeneousItems<'a, T: RLPParsable<'a>, const VALIDATE: bool> {
    parser: Parser<'a>,
    pub count: Option<usize>,
    _marker: core::marker::PhantomData<T>,
}

#[derive(Clone, Copy, Debug)]
pub struct RLPListOfHomogeneousItemsIter<'a, T: RLPParsable<'a>, const VALIDATE: bool> {
    parser: Parser<'a>,
    _marker: core::marker::PhantomData<T>,
}

impl<'a, T: RLPParsable<'a>, const VALIDATE: bool> RLPListOfHomogeneousItems<'a, T, VALIDATE> {
    pub fn try_parse_slice_in_full(input: &'a [u8]) -> Result<Self, ()> {
        let mut parser = Parser::new(input);
        let new = Self::try_parse(&mut parser)?;
        if parser.is_empty() == false {
            return Err(());
        }

        Ok(new)
    }
}

impl<'a, T: RLPParsable<'a>, const VALIDATE: bool> RLPParsable<'a>
    for RLPListOfHomogeneousItems<'a, T, VALIDATE>
{
    fn try_parse(parser: &mut Parser<'a>) -> Result<Self, ()> {
        // from the parser - it's itself a list
        let parser = parser.try_make_list_subparser()?;
        if VALIDATE {
            let mut count = 0;
            let mut validation_parser = parser;
            // NOTE: loop is safe, as our parsers move forward
            loop {
                if validation_parser.is_empty() {
                    break;
                }
                let _: T = RLPParsable::try_parse(&mut validation_parser)?;
                count += 1;
            }

            let new = Self {
                parser,
                count: Some(count),
                _marker: core::marker::PhantomData,
            };

            Ok(new)
        } else {
            let new = Self {
                parser,
                count: None,
                _marker: core::marker::PhantomData,
            };

            Ok(new)
        }
    }
}

impl<'a, T: RLPParsable<'a>, const VALIDATE: bool> RLPListOfHomogeneousItems<'a, T, VALIDATE> {
    pub fn iter(&self) -> RLPListOfHomogeneousItemsIter<'a, T, VALIDATE> {
        RLPListOfHomogeneousItemsIter {
            parser: self.parser,
            _marker: core::marker::PhantomData,
        }
    }
}

impl<'a, T: RLPParsable<'a>> Iterator for RLPListOfHomogeneousItemsIter<'a, T, true> {
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        if self.parser.is_empty() {
            None
        } else {
            Some(RLPParsable::try_parse(&mut self.parser).expect("was pre-validated"))
        }
    }
}

impl<'a, T: RLPParsable<'a>> Iterator for RLPListOfHomogeneousItemsIter<'a, T, false> {
    type Item = Result<T, ()>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.parser.is_empty() {
            None
        } else {
            Some(RLPParsable::try_parse(&mut self.parser).map_err(|_| {
                self.parser = Parser::new(&[]);
            }))
        }
    }
}

pub type StorageSlotsList<'a> = RLPListOfFixedLengthItems<'a, &'a [u8; 32]>;

#[derive(Clone, Copy, Debug)]
pub struct AccessListForAddress<'a> {
    pub address: B160,
    pub slots_list: StorageSlotsList<'a>,
}

impl<'a> RLPParsable<'a> for AccessListForAddress<'a> {
    fn try_parse(parser: &mut Parser<'a>) -> Result<Self, ()> {
        let address = <B160 as RLPParsable<'a>>::try_parse(parser)?;
        let slots_list = <StorageSlotsList as RLPParsable<'a>>::try_parse(parser)?;

        let new = Self {
            address,
            slots_list,
        };

        Ok(new)
    }
}

pub type AccessList<'a> =
    RLPListOfHomogeneousItems<'a, ListEncapsulated<'a, AccessListForAddress<'a>>, true>;
