//!
//! Parser for access lists.
//! See [ZkSyncTransaction] for more details on encoding format.
//!

use crate::bootloader::transaction::reserved_dynamic_parser::{check_offset, parse_u32};
use crate::bootloader::Bytes32;
use ruint::aliases::B160;

use super::reserved_dynamic_parser::parse_address;

#[derive(Clone, Copy, Debug)]
pub struct AccessListParser {
    pub offset: usize,
}

impl AccessListParser {
    pub fn into_iter<'a>(&self, slice: &'a [u8]) -> Result<AccessListIter<'a>, ()> {
        AccessListIter::new(slice, self.offset)
    }
}

// Used to enforce strict encoding
struct PreviousItemInfo {
    offset: usize,
    nb_keys: usize,
}

impl PreviousItemInfo {
    fn next_expected_offset(&self) -> usize {
        // Next expected offset is equal to:
        // offset + len(address, keys_offset, keys_len, keys)
        self.offset + 32 * (3 + self.nb_keys)
    }
}

pub struct AccessListIter<'a> {
    slice: &'a [u8],
    pub(crate) count: usize,
    head_start: usize,
    index: usize,
    prev_item_info: Option<PreviousItemInfo>,
}

impl<'a> AccessListIter<'a> {
    pub fn empty(slice: &'a [u8]) -> Self {
        // Offset doesn't matter here, as we first check if it's empty
        Self {
            slice,
            count: 0,
            head_start: 0,
            index: 0,
            prev_item_info: None,
        }
    }

    fn new(slice: &'a [u8], offset: usize) -> Result<Self, ()> {
        let count = parse_u32(slice, offset)?;
        let head_start = offset + 32;

        Ok(AccessListIter {
            slice,
            count,
            head_start,
            index: 0,
            prev_item_info: None,
        })
    }

    fn parse_current(&mut self) -> Result<(B160, StorageKeysIter<'a>), ()> {
        // Assume index < count, checked by iterator impl
        let offset = self.head_start + self.index.checked_mul(32).ok_or(())?;
        let item_ptr_offset = parse_u32(self.slice, offset)?;
        check_offset(
            item_ptr_offset,
            self.prev_item_info
                .as_ref()
                .map_or(32 * self.count, |p| p.next_expected_offset()),
        )?;
        let item_offset = self.head_start + item_ptr_offset;
        let address = parse_address(self.slice, item_offset)?;
        let keys_ptr_offset = parse_u32(self.slice, item_offset + 32)?;
        // Always 64 = len(offset, keys_len)
        check_offset(keys_ptr_offset, 64)?;
        let keys_offset = item_offset + keys_ptr_offset;
        let keys_len = parse_u32(self.slice, keys_offset)?;
        let keys_slice = self.slice.get(keys_offset + 32..).ok_or(())?;

        self.prev_item_info = Some(PreviousItemInfo {
            offset: item_ptr_offset,
            nb_keys: keys_len,
        });

        Ok((
            address,
            StorageKeysIter {
                slice: keys_slice,
                index: 0,
                count: keys_len,
            },
        ))
    }
}

pub struct StorageKeysIter<'a> {
    slice: &'a [u8],
    index: usize,
    pub(crate) count: usize,
}

impl<'a> StorageKeysIter<'a> {
    fn parse_current(&mut self) -> Result<Bytes32, ()> {
        // Assume index < count, checked by iterator impl
        let offset = self.index.checked_mul(32).ok_or(())?;
        let bytes = self.slice.get(offset..offset + 32).ok_or(())?;
        let item = Bytes32::from_array(bytes.try_into().unwrap());
        Ok(item)
    }
}

impl<'a> Iterator for AccessListIter<'a> {
    type Item = Result<(B160, StorageKeysIter<'a>), ()>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.count {
            return None;
        }
        let current = self.parse_current();
        self.index += 1;
        Some(current)
    }
}

impl<'a> Iterator for StorageKeysIter<'a> {
    type Item = Result<Bytes32, ()>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.count {
            return None;
        }
        let current = self.parse_current();
        self.index += 1;
        Some(current)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_previous_item_info_calculation() {
        let prev_item = PreviousItemInfo {
            offset: 64,
            nb_keys: 3,
        };

        // Expected calculation: offset + 32 * (3 + nb_keys)
        // = 64 + 32 * (3 + 3) = 64 + 32 * 6 = 64 + 192 = 256
        let expected_next_offset = prev_item.next_expected_offset();
        assert_eq!(
            expected_next_offset, 256,
            "Should calculate correct next offset based on formula: offset + 32 * (3 + nb_keys)"
        );

        // Test with different values
        let prev_item2 = PreviousItemInfo {
            offset: 128,
            nb_keys: 5,
        };

        // Expected: 128 + 32 * (3 + 5) = 128 + 32 * 8 = 128 + 256 = 384
        let expected_next_offset2 = prev_item2.next_expected_offset();
        assert_eq!(
            expected_next_offset2, 384,
            "Should handle different offset and key counts correctly"
        );

        // Test with zero keys
        let prev_item3 = PreviousItemInfo {
            offset: 96,
            nb_keys: 0,
        };

        // Expected: 96 + 32 * (3 + 0) = 96 + 32 * 3 = 96 + 96 = 192
        let expected_next_offset3 = prev_item3.next_expected_offset();
        assert_eq!(
            expected_next_offset3, 192,
            "Should handle zero keys case correctly"
        );
    }
}
