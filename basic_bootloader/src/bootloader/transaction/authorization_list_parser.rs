//!
//! Parser for authorization lists.
//! See [ZkSyncTransaction] for more details on encoding format.
//!

use crate::bootloader::transaction::reserved_dynamic_parser::{
    parse_address, parse_u256, parse_u32, parse_u64, parse_u8,
};
use ruint::aliases::{B160, U256};

#[derive(Clone, Copy, Debug)]
pub struct AuthorizationListParser {
    pub offset: usize,
}

impl AuthorizationListParser {
    pub fn into_iter<'a>(&self, slice: &'a [u8]) -> Result<AuthorizationListIter<'a>, ()> {
        AuthorizationListIter::new(slice, self.offset)
    }
}

pub struct AuthorizationListIter<'a> {
    slice: &'a [u8],
    pub(crate) count: usize,
    head_start: usize,
    index: usize,
}

const AUTHORIZATION_LIST_ITEM_BYTES: usize = 6 * 32;
pub struct AuthorizationListItem {
    pub chain_id: U256,
    pub address: B160,
    pub nonce: u64,
    pub y_parity: u8,
    pub r: U256,
    pub s: U256,
}

impl<'a> AuthorizationListIter<'a> {
    pub fn empty(slice: &'a [u8]) -> Self {
        // Offset doesn't matter here, as we first check if it's empty
        Self {
            slice,
            count: 0,
            head_start: 0,
            index: 0,
        }
    }

    fn new(slice: &'a [u8], offset: usize) -> Result<Self, ()> {
        let count = parse_u32(slice, offset)?;
        let head_start = offset + 32;

        Ok(AuthorizationListIter {
            slice,
            count,
            head_start,
            index: 0,
        })
    }

    fn parse_current(&mut self) -> Result<AuthorizationListItem, ()> {
        // Assume index < count, checked by iterator impl
        let offset = self.head_start
            + self
                .index
                .checked_mul(AUTHORIZATION_LIST_ITEM_BYTES)
                .ok_or(())?;
        let chain_id = parse_u256(&self.slice, offset)?;
        let address = parse_address(&self.slice, offset + 32)?;
        let nonce = parse_u64(&self.slice, offset + 2 * 32)?;
        let y_parity = parse_u8(&self.slice, offset + 3 * 32)?;
        let r = parse_u256(&self.slice, offset + 4 * 32)?;
        let s = parse_u256(&self.slice, offset + 5 * 32)?;
        Ok(AuthorizationListItem {
            chain_id,
            address,
            nonce,
            y_parity,
            r,
            s,
        })
    }
}

impl<'a> Iterator for AuthorizationListIter<'a> {
    type Item = Result<AuthorizationListItem, ()>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.count {
            return None;
        }
        let current = self.parse_current();
        self.index += 1;
        Some(current)
    }
}
