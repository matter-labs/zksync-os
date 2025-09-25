//!
//! Parser for reserved dynamic.
//! See [ZkSyncTransaction] for more details on encoding format.
//!

#[cfg(feature = "pectra")]
use super::authorization_list::{AuthorizationListIter, AuthorizationListParser};
use super::{AccessListIter, AccessListParser};
use ruint::aliases::{B160, U256};

#[derive(Clone, Copy, Debug)]

pub struct Parsers {
    access_list_parser: AccessListParser,
    #[cfg(feature = "pectra")]
    authorization_list_parser: AuthorizationListParser,
}

#[derive(Clone, Copy, Debug)]
pub struct ReservedDynamicParser {
    parsers: Option<Parsers>,
}

impl ReservedDynamicParser {
    pub fn new<'a>(slice: &'a [u8], offset: usize) -> Result<Self, ()> {
        // Reserved dynamic is a bytestring of a list,
        // so that we can add fields later on.
        let bytestring_len = parse_u32(slice, offset)?;
        if bytestring_len == 0 {
            // If empty bytestring, interpret as empty list
            return Ok(Self { parsers: None });
        }
        let offset = offset + 32;

        // For now, it has the access list and authorization list
        // First, parse the tuple offset
        let outer_offset = parse_u32(slice, offset)?;
        check_offset(outer_offset, 32)?;
        let outer_base = offset + outer_offset;
        let outer_len = parse_u32(slice, outer_base)?;
        if outer_len != 2 {
            return Err(());
        }

        let access_list_rel_offset = parse_u32(slice, outer_base + 32)?;
        // Must be 64 (there's the authorization list offset before)
        check_offset(access_list_rel_offset, 64)?;
        #[cfg(feature = "pectra")]
        let authorization_list_rel_offset = parse_u32(slice, outer_base + 64)?;
        let access_list_base = outer_base + 32 + access_list_rel_offset;
        // We cannot check strictness of authorization_list_rel_offset yet, we
        // have to do that once we know the full length of the access list
        #[cfg(feature = "pectra")]
        let authorization_list_base = outer_base + 32 + authorization_list_rel_offset;

        Ok(Self {
            parsers: Some(Parsers {
                access_list_parser: AccessListParser {
                    offset: access_list_base,
                },
                #[cfg(feature = "pectra")]
                authorization_list_parser: AuthorizationListParser {
                    offset: authorization_list_base,
                },
            }),
        })
    }

    pub fn access_list_iter<'a>(&self, slice: &'a [u8]) -> Result<AccessListIter<'a>, ()> {
        match self.parsers {
            None => Ok(AccessListIter::empty(slice)),
            Some(Parsers {
                access_list_parser, ..
            }) => access_list_parser.into_iter(slice),
        }
    }

    pub fn access_list_is_empty<'a>(&self, slice: &'a [u8]) -> Result<bool, ()> {
        Ok(self.access_list_iter(slice)?.next().is_none())
    }

    #[cfg(feature = "pectra")]
    pub fn authorization_list_iter<'a>(
        &self,
        slice: &'a [u8],
    ) -> Result<AuthorizationListIter<'a>, ()> {
        match self.parsers {
            None => Ok(AuthorizationListIter::empty(slice)),
            Some(Parsers {
                authorization_list_parser,
                ..
            }) => authorization_list_parser.into_iter(slice),
        }
    }

    #[cfg(feature = "pectra")]
    pub fn authorization_list_is_empty<'a>(&self, slice: &'a [u8]) -> Result<bool, ()> {
        Ok(self.authorization_list_iter(slice)?.next().is_none())
    }
}

// Return usize for ease of use for indexing
pub(crate) fn parse_u32<'a>(slice: &'a [u8], offset: usize) -> Result<usize, ()> {
    let bytes = slice.get(offset..(offset + 32)).ok_or(())?;
    for byte in bytes.iter().take(28) {
        if *byte != 0 {
            return Err(());
        }
    }
    let value = u32::from_be_bytes(bytes[28..32].try_into().unwrap());
    Ok(value as usize)
}

// Used for pectra feature
#[allow(dead_code)]
pub(crate) fn parse_u8<'a>(slice: &'a [u8], offset: usize) -> Result<u8, ()> {
    let bytes = slice.get(offset..(offset + 32)).ok_or(())?;
    for byte in bytes.iter().take(31) {
        if *byte != 0 {
            return Err(());
        }
    }
    let value = u8::from_be_bytes(bytes[31..32].try_into().unwrap());
    Ok(value)
}

// Used for pectra feature
#[allow(dead_code)]
pub(crate) fn parse_u64<'a>(slice: &'a [u8], offset: usize) -> Result<u64, ()> {
    let bytes = slice.get(offset..(offset + 32)).ok_or(())?;
    for byte in bytes.iter().take(24) {
        if *byte != 0 {
            return Err(());
        }
    }
    let value = u64::from_be_bytes(bytes[24..32].try_into().unwrap());
    Ok(value)
}

pub(crate) fn parse_address<'a>(slice: &'a [u8], offset: usize) -> Result<B160, ()> {
    let bytes = slice.get(offset..(offset + 32)).ok_or(())?;
    for byte in bytes.iter().take(12) {
        if *byte != 0 {
            return Err(());
        }
    }
    let value = B160::from_be_bytes::<20>(bytes[12..32].try_into().unwrap());
    Ok(value)
}

// Used for pectra feature
#[allow(dead_code)]
pub(crate) fn parse_u256<'a>(slice: &'a [u8], offset: usize) -> Result<U256, ()> {
    let bytes = slice.get(offset..(offset + 32)).ok_or(())?;
    let value = U256::from_be_bytes::<32>(bytes.try_into().unwrap());
    Ok(value)
}

// Check an offset is the expected value, to enforce strict encoding.
pub(crate) fn check_offset(offset: usize, expected: usize) -> Result<(), ()> {
    if offset != expected {
        Err(())
    } else {
        Ok(())
    }
}

// In tests/rig/src/utils.rs we actually check the correctness of fields,
// we just check lengths here.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_access_list_parser_empty_bytes() {
        // Empty list encoded as empty bytes (backwards compatibility)
        let encoded = hex::decode ("00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000000").unwrap();

        let parser = ReservedDynamicParser::new(&encoded, 0).expect("Must create parser");
        let mut iter = parser
            .access_list_iter(&encoded)
            .expect("Must parse access list");
        assert!(iter.next().is_none())
    }

    #[test]
    fn test_access_list_parser_empty_list() {
        // Empty access list encoded as bytes of empty list
        let encoded = hex::decode("00000000000000000000000000000000000000000000000000000000000000c0000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").unwrap();

        let parser = ReservedDynamicParser::new(&encoded, 0).expect("Must create parser");
        let mut iter = parser
            .access_list_iter(&encoded)
            .expect("Must parse access list");
        assert!(iter.next().is_none())
    }

    #[test]
    fn test_access_list_parser_1_1_0() {
        // Access list with 1 element with 0 storage keys
        let encoded = hex::decode("000000000000000000000000000000000000000000000000000000000000014000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000e0000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000200000000000000000000000001111111111111111111111111111111111111111000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").unwrap();

        let parser = ReservedDynamicParser::new(&encoded, 0).expect("Must create parser");
        let mut iter = parser
            .access_list_iter(&encoded)
            .expect("Must parse access list");
        // Check list has 1 items:
        assert_eq!(iter.count, 1);
        let (_, keys1) = iter
            .next()
            .expect("Must have first")
            .expect("Must parse first");
        assert_eq!(keys1.count, 0);
        assert!(iter.next().is_none())
    }

    #[test]
    fn test_access_list_parser_1_1_2() {
        // Access list with 1 element with 2 storage keys
        let encoded = hex::decode("0000000000000000000000000000000000000000000000000000000000000180000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000012000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000020000000000000000000000000111111111111111111111111111111111111111100000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000002222222222222222222222222222222222222222222222222222222222222222233333333333333333333333333333333333333333333333333333333333333330000000000000000000000000000000000000000000000000000000000000000").unwrap();

        let parser = ReservedDynamicParser::new(&encoded, 0).expect("Must create parser");
        let mut iter = parser
            .access_list_iter(&encoded)
            .expect("Must parse access list");
        // Check list has 1 items:
        assert_eq!(iter.count, 1);
        let (_, keys1) = iter
            .next()
            .expect("Must have first")
            .expect("Must parse first");
        assert_eq!(keys1.count, 2);
        assert!(iter.next().is_none())
    }

    #[cfg(feature = "pectra")]
    #[test]
    fn test_access_list_parser_2_2_2() {
        // Access list with 2 elements, each with 2 keys
        let encoded = hex::decode("000000000000000000000000000000000000000000000000000000000000024000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000001e00000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000e000000000000000000000000011111111111111111111111111111111111111110000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000222222222222222222222222222222222222222222222222222222222222222223333333333333333333333333333333333333333333333333333333333333333000000000000000000000000101010101010101010101010101010101010101000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000002444444444444444444444444444444444444444444444444444444444444444455555555555555555555555555555555555555555555555555555555555555550000000000000000000000000000000000000000000000000000000000000000").unwrap();

        let parser = ReservedDynamicParser::new(&encoded, 0).expect("Must create parser");
        let mut iter = parser
            .access_list_iter(&encoded)
            .expect("Must parse access list");
        // Check list has 2 items:
        assert_eq!(iter.count, 2);
        let (_, keys1) = iter
            .next()
            .expect("Must have first")
            .expect("Must parse first");
        assert_eq!(keys1.count, 2);
        let (_, keys2) = iter
            .next()
            .expect("Must have second")
            .expect("Must parse second");
        assert_eq!(keys2.count, 2);
        assert!(iter.next().is_none());
        // Check authorization list is empty

        assert!(parser
            .authorization_list_is_empty(&encoded)
            .expect("Must parse"));
    }

    // Also use an access list
    #[cfg(feature = "pectra")]
    #[test]
    fn test_authorization_list_2() {
        // Access list with 2 elements, each with 2 keys
        // Authorization list with 2 elements
        let encoded = hex::decode("00000000000000000000000000000000000000000000000000000000000003c000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000001e00000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000e00000000000000000000000001111111111111111111111111111111111111111000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000022222222222222222222222222222222222222222222222222222222222222222333333333333333333333333333333333333333333333333333333333333333300000000000000000000000010101010101010101010101010101010101010100000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000244444444444444444444444444444444444444444444444444444444444444445555555555555555555555555555555555555555555555555555555555555555000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000030000000000000000000000000101010101010101010101010101010101010101000000000000000000000000000000000000000000000000000000000000000a0000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000002a000000000000000000000000000000000000000000000000000000000000002b00000000000000000000000000000000000000000000000000000000000000030000000000000000000000000101010101010101010101010101010101010101000000000000000000000000000000000000000000000000000000000000000a0000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000002a000000000000000000000000000000000000000000000000000000000000002b").unwrap();

        let parser = ReservedDynamicParser::new(&encoded, 0).expect("Must create parser");
        let mut iter = parser
            .authorization_list_iter(&encoded)
            .expect("Must parse authorization list");
        // Check list has 2 items:
        assert_eq!(iter.count, 2);
        let _ = iter
            .next()
            .expect("Must have first")
            .expect("Must parse first");
        let _ = iter
            .next()
            .expect("Must have second")
            .expect("Must parse second");
        assert!(iter.next().is_none())
    }

    #[test]
    fn test_parse_u32_validation() {
        // parse_u32 should validate that only the last 4 bytes contain data

        // Valid u32 (all leading bytes are zero)
        let mut valid_data = vec![0u8; 32];
        valid_data[28..32].copy_from_slice(&42u32.to_be_bytes());
        let result = parse_u32(&valid_data, 0);
        assert!(result.is_ok(), "Valid u32 should parse successfully");
        assert_eq!(result.unwrap(), 42, "Should parse correct u32 value");

        // Invalid u32 (non-zero byte in leading 28 bytes)
        let mut invalid_data = vec![0u8; 32];
        invalid_data[27] = 1; // Set byte 27 to non-zero
        invalid_data[28..32].copy_from_slice(&42u32.to_be_bytes());
        let result = parse_u32(&invalid_data, 0);
        assert!(
            result.is_err(),
            "Invalid u32 with non-zero leading byte should fail"
        );

        // Another invalid case (non-zero byte at position 0)
        let mut invalid_data2 = vec![0u8; 32];
        invalid_data2[0] = 1; // Set first byte to non-zero
        invalid_data2[28..32].copy_from_slice(&42u32.to_be_bytes());
        let result = parse_u32(&invalid_data2, 0);
        assert!(
            result.is_err(),
            "Invalid u32 with non-zero first byte should fail"
        );

        // Edge case: maximum valid u32
        let mut max_u32_data = vec![0u8; 32];
        max_u32_data[28..32].copy_from_slice(&u32::MAX.to_be_bytes());
        let result = parse_u32(&max_u32_data, 0);
        assert!(
            result.is_ok(),
            "Maximum u32 value should parse successfully"
        );
        assert_eq!(
            result.unwrap(),
            u32::MAX as usize,
            "Should parse maximum u32 value"
        );
    }

    #[test]
    fn test_check_offset_validation() {
        // Valid offset (matches expected)
        let result = check_offset(32, 32);
        assert!(result.is_ok(), "Matching offset should be valid");

        // Invalid offset (doesn't match expected)
        let result = check_offset(64, 32);
        assert!(result.is_err(), "Non-matching offset should be invalid");

        // Zero offset cases
        let result = check_offset(0, 0);
        assert!(
            result.is_ok(),
            "Zero offset matching expected should be valid"
        );

        let result = check_offset(0, 32);
        assert!(
            result.is_err(),
            "Zero offset not matching expected should be invalid"
        );
    }

    #[test]
    fn test_parse_u32_boundary_conditions() {
        // Buffer too short
        let short_data = vec![0u8; 31]; // Only 31 bytes instead of 32
        let result = parse_u32(&short_data, 0);
        assert!(result.is_err(), "Buffer too short should fail");

        // Offset out of bounds
        let data = vec![0u8; 32];
        let result = parse_u32(&data, 1); // Would read beyond buffer
        assert!(
            result.is_err(),
            "Offset causing out-of-bounds read should fail"
        );

        // All bytes in leading section must be zero
        for i in 0..28 {
            let mut invalid_data = vec![0u8; 32];
            invalid_data[i] = 1; // Set one leading byte to non-zero
            invalid_data[28..32].copy_from_slice(&100u32.to_be_bytes());
            let result = parse_u32(&invalid_data, 0);
            assert!(
                result.is_err(),
                "Non-zero byte at position {} should cause parsing to fail",
                i
            );
        }
    }
}
