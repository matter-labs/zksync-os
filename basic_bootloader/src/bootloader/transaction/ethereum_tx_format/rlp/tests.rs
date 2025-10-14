use crate::bootloader::transaction::ethereum_tx_format::rlp::minimal_rlp_parser::{
    HomList, Rlp, RlpFixedItem,
};

use alloy_primitives::Bytes;
use alloy_rlp::Encodable;
use alloy_rlp::Rlp as AlloyRlp;
use ruint::aliases::B160;

#[test]
fn test_alloy_compatibility_u64() {
    // Test u64 encoding compatibility
    let values = [0u64, 1, 127, 128, 255, 256, 65535, 65536, u64::MAX];

    for &value in &values {
        let mut alloy_encoded = Vec::new();
        value.encode(&mut alloy_encoded);

        let mut rlp_decoder = Rlp::new(&alloy_encoded);
        let decoded = rlp_decoder.u64().unwrap();
        assert_eq!(decoded, value, "u64 value {} mismatch", value);

        let mut alloy_rlp_decoder = AlloyRlp::new(&alloy_encoded).unwrap();
        let alloy_decoded: u64 = alloy_rlp_decoder.get_next().unwrap().unwrap();
        assert_eq!(
            decoded, alloy_decoded,
            "u64 value {} mismatch with alloy",
            value
        );

        assert!(
            rlp_decoder.is_empty(),
            "Should consume all bytes for u64 {}",
            value
        );
    }
}

#[test]
fn test_alloy_compatibility_strings() {
    let test_strings = [
        "",
        "a",
        "dog",
        "Lorem ipsum dolor sit amet, consectetur adipiscing elit", // > 55 chars
        &"x".repeat(56),  // Exactly 56 chars (triggers long string encoding)
        &"y".repeat(100), // Long string
    ];

    for test_str in &test_strings {
        let mut alloy_encoded = Vec::new();
        test_str.as_bytes().encode(&mut alloy_encoded);

        let mut rlp_decoder = Rlp::new(&alloy_encoded);
        let decoded = rlp_decoder.bytes().unwrap();

        let mut alloy_rlp_decoder = AlloyRlp::new(&alloy_encoded).unwrap();
        let alloy_decoded: Bytes = alloy_rlp_decoder.get_next().unwrap().unwrap();
        assert_eq!(
            decoded, alloy_decoded.0,
            "str {} mismatch with alloy",
            test_str
        );

        assert_eq!(
            decoded,
            test_str.as_bytes(),
            "String '{}' mismatch",
            test_str
        );
        assert!(
            rlp_decoder.is_empty(),
            "Should consume all bytes for string '{}'",
            test_str
        );
    }
}

#[test]
fn test_alloy_compatibility_u256() {
    let test_values = [
        ruint::aliases::U256::ZERO,
        ruint::aliases::U256::from(1),
        ruint::aliases::U256::from(255),
        ruint::aliases::U256::from(256),
        ruint::aliases::U256::from(65535),
        ruint::aliases::U256::from(65536),
        ruint::aliases::U256::from(u64::MAX),
        ruint::aliases::U256::MAX,
    ];

    for &value in &test_values {
        let mut alloy_encoded = Vec::new();
        value.encode(&mut alloy_encoded);

        let mut rlp_decoder = Rlp::new(&alloy_encoded);
        let decoded = rlp_decoder.u256().unwrap();

        let mut alloy_rlp_decoder = AlloyRlp::new(&alloy_encoded).unwrap();
        let alloy_decoded: alloy_primitives::U256 = alloy_rlp_decoder.get_next().unwrap().unwrap();
        assert_eq!(decoded, alloy_decoded, "U256 {} mismatch with alloy", value);

        assert_eq!(decoded, value, "U256 value {} mismatch", value);
        assert!(
            rlp_decoder.is_empty(),
            "Should consume all bytes for U256 {}",
            value
        );
    }
}

#[test]
fn test_alloy_compatibility_lists() {
    // Test simple list: ["cat", "dog"]
    let items = vec![b"cat".as_slice(), b"dog".as_slice()];
    let mut alloy_encoded = Vec::new();
    items.encode(&mut alloy_encoded);

    let mut rlp_decoder = Rlp::new(&alloy_encoded);
    let mut list = rlp_decoder.list().unwrap();

    let first = list.bytes().unwrap();
    assert_eq!(first, b"cat");

    let second = list.bytes().unwrap();
    assert_eq!(second, b"dog");

    assert!(list.is_empty());
    assert!(rlp_decoder.is_empty());
}

#[test]
fn test_alloy_compatibility_empty_values() {
    // Test empty string
    let mut alloy_encoded = Vec::new();
    b"".encode(&mut alloy_encoded);
    assert_eq!(alloy_encoded, &[0x80]); // Empty string should be 0x80

    let mut rlp_decoder = Rlp::new(&alloy_encoded);
    let decoded = rlp_decoder.bytes().unwrap();
    assert_eq!(decoded, b"");
    assert!(rlp_decoder.is_empty());

    // Test empty list
    let mut alloy_encoded = Vec::new();
    let empty_list: Vec<u8> = vec![];
    empty_list.encode(&mut alloy_encoded);
    assert_eq!(alloy_encoded, &[0xc0]); // Empty list should be 0xc0

    let mut rlp_decoder = Rlp::new(&alloy_encoded);
    let list = rlp_decoder.list().unwrap();
    assert!(list.is_empty());
    assert!(rlp_decoder.is_empty());
}

#[test]
fn test_alloy_compatibility_long_list() {
    // Create a list with many elements to test long list encoding
    let items: Vec<u64> = (0..100).collect();
    let mut alloy_encoded = Vec::new();
    items.encode(&mut alloy_encoded);

    let mut rlp_decoder = Rlp::new(&alloy_encoded);
    let mut list = rlp_decoder.list().unwrap();

    // Verify all items can be decoded
    for expected in 0..100 {
        let actual = list.u64().unwrap();
        assert_eq!(actual, expected);
    }

    assert!(list.is_empty());
    assert!(rlp_decoder.is_empty());
}

#[test]
fn test_alloy_compatibility_addresses() {
    // Test Ethereum address encoding/decoding
    let test_addresses = [
        [0x00; 20],
        [0xFF; 20],
        [
            0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66,
            0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC,
        ],
    ];

    for &addr_bytes in &test_addresses {
        let mut alloy_encoded = Vec::new();
        addr_bytes.encode(&mut alloy_encoded);

        // Should be 0x94 (0x80 + 20) followed by 20 bytes
        assert_eq!(alloy_encoded[0], 0x94);
        assert_eq!(alloy_encoded.len(), 21);

        // Test with our B160 decoder
        let addr = B160::decode_from_fixed(&alloy_encoded).unwrap();
        assert_eq!(addr.to_be_bytes(), addr_bytes);

        // Test with generic bytes decoder
        let mut rlp_decoder = Rlp::new(&alloy_encoded);
        let decoded = rlp_decoder.bytes().unwrap();
        assert_eq!(decoded, &addr_bytes);
        assert!(rlp_decoder.is_empty());
    }
}

#[test]
fn test_alloy_compatibility_boundary_cases() {
    // Test boundary cases for string encoding

    // 55-byte string (last short string)
    let str_55 = "x".repeat(55);
    let mut alloy_encoded = Vec::new();
    str_55.as_bytes().encode(&mut alloy_encoded);

    let mut rlp_decoder = Rlp::new(&alloy_encoded);
    let decoded = rlp_decoder.bytes().unwrap();
    assert_eq!(decoded, str_55.as_bytes());

    // 56-byte string (first long string)
    let str_56 = "y".repeat(56);
    let mut alloy_encoded = Vec::new();
    str_56.as_bytes().encode(&mut alloy_encoded);
    assert_eq!(alloy_encoded[0], 0xb8); // Should be 0xb7 + 1 (long encoding)
    assert_eq!(alloy_encoded[1], 56); // Length byte

    let mut rlp_decoder = Rlp::new(&alloy_encoded);
    let decoded = rlp_decoder.bytes().unwrap();
    assert_eq!(decoded, str_56.as_bytes());

    // Similar tests for lists
    // 55-byte list payload (last short list)
    let items_55: Vec<u8> = (0..55).collect();
    let mut alloy_encoded = Vec::new();
    items_55.encode(&mut alloy_encoded);
    assert_eq!(alloy_encoded[0], 0xc0 + 55); // Should be short list encoding

    let mut rlp_decoder = Rlp::new(&alloy_encoded);
    let mut list = rlp_decoder.list().unwrap();
    for expected in 0..55 {
        let actual = list.u8().unwrap();
        assert_eq!(actual, expected);
    }
    assert!(list.is_empty());

    // 56-byte list payload (first long list)
    let items_56: Vec<u8> = (0..56).collect();
    let mut alloy_encoded = Vec::new();
    items_56.encode(&mut alloy_encoded);
    assert_eq!(alloy_encoded[0], 0xf8); // Should be 0xf7 + 1 (long list encoding)
    assert_eq!(alloy_encoded[1], 56); // Length byte

    let mut rlp_decoder = Rlp::new(&alloy_encoded);
    let mut list = rlp_decoder.list().unwrap();
    for expected in 0..56 {
        let actual = list.u8().unwrap();
        assert_eq!(actual, expected);
    }
    assert!(list.is_empty());
}

#[test]
fn test_rlp_integer_overflow_cases() {
    // Test position overflow in take_exact
    let data = [0x80]; // Empty string
    let mut rlp = Rlp::new(&data);
    // This should work
    assert!(rlp.bytes().is_ok());

    // Test with usize::MAX - should fail gracefully
    let mut rlp = Rlp::new(&[0xbf, 0xFF, 0xFF, 0xFF, 0xFF]); // Claims max length
    assert!(rlp.bytes().is_err());

    // Test extremely large length values
    let mut malformed = vec![0xbb]; // Long string with 4-byte length
    malformed.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]); // Max u32 length
    let mut rlp = Rlp::new(&malformed);
    assert!(rlp.bytes().is_err());

    // Test length field overflow for lists
    let mut malformed = vec![0xfb]; // Long list with 4-byte length
    malformed.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]); // Max u32 length
    let mut rlp = Rlp::new(&malformed);
    assert!(rlp.list().is_err());
}

#[test]
fn test_rlp_type_mismatch_comprehensive() {
    // Try to decode list as bytes
    let list_data = [0xc2, 0x01, 0x02]; // List with two items
    let mut rlp = Rlp::new(&list_data);
    assert!(rlp.bytes().is_err());

    // Try to decode bytes as list
    let string_data = [0x83, 0x64, 0x6f, 0x67]; // "dog"
    let mut rlp = Rlp::new(&string_data);
    assert!(rlp.list().is_err());

    // Try to decode single byte as multi-byte number
    let single_byte = [0x42]; // Just 'B' = 66
    let mut rlp = Rlp::new(&single_byte);
    assert_eq!(rlp.u64().unwrap(), 0x42); // Single bytes decode as their value

    // Try to decode empty as number
    let empty = [0x80]; // Empty string
    let mut rlp = Rlp::new(&empty);
    assert_eq!(rlp.u8().unwrap(), 0); // Empty should decode as 0

    let mut rlp = Rlp::new(&empty);
    assert_eq!(rlp.u64().unwrap(), 0); // Empty should decode as 0

    // Try to decode very large number as smaller type
    let large_num = [0x89, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09]; // 9 bytes
    let mut rlp = Rlp::new(&large_num);
    assert!(rlp.u64().is_err()); // Too big for u64
}

#[test]
fn test_rlp_decoder_edge_cases() {
    // Bool decoder with invalid values
    let invalid_bool = [0x82, 0x01, 0x01]; // Two bytes: [1, 1]
    let mut rlp = Rlp::new(&invalid_bool);
    assert!(rlp.bool().is_err());

    let invalid_bool2 = [0x02]; // Single byte value 2
    let mut rlp = Rlp::new(&invalid_bool2);
    assert!(rlp.bool().is_err());

    // u8 decoder with multi-byte values
    let multi_byte = [0x82, 0x01, 0x00]; // Two bytes: [1, 0] = 256
    let mut rlp = Rlp::new(&multi_byte);
    assert!(rlp.u8().is_err());

    // Fixed-length decoder with wrong sizes
    let wrong_size_addr = vec![
        0x93, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11,
        0x11, 0x11, 0x11, 0x11, 0x11,
    ]; // 19 bytes instead of 20
    assert!(B160::decode_from_fixed(&wrong_size_addr).is_err());

    // Wrong prefix for B160
    let wrong_prefix = vec![0x95]; // Wrong prefix
    let mut wrong_addr = wrong_prefix;
    wrong_addr.extend_from_slice(&[0x11; 20]);
    assert!(B160::decode_from_fixed(&wrong_addr).is_err());
}

#[test]
fn test_rlp_list_structure_violations() {
    // Homogeneous list with inconsistent types
    let mut inconsistent = vec![0xc0 + 42]; // List with calculated payload size
    inconsistent.push(0x94); // Valid B160 header
    inconsistent.extend_from_slice(&[0x11; 20]); // 20 bytes
    inconsistent.push(0x95); // Invalid B160 header (wrong size claim)
    inconsistent.extend_from_slice(&[0x22; 20]); // 20 bytes

    // Should fail validation in HomList with validation enabled
    assert!(HomList::<B160, true>::decode_list_full(&inconsistent).is_err());

    // Partial list consumption
    let list_data = [0xc4, 0x01, 0x02, 0x03, 0x04]; // List with 4 items
    let mut rlp = Rlp::new(&list_data);
    let mut list = rlp.list().unwrap();
    let _ = list.u8().unwrap(); // Consume only first item
    let _ = list.u8().unwrap(); // Consume second item
                                // Leave items unconsumed - this is allowed but worth testing
    assert!(!list.is_empty());

    // List claiming to contain more items than available
    let truncated_list = [0xc3, 0x01, 0x02]; // Claims 3 bytes but only has 2
    let mut rlp = Rlp::new(&truncated_list);
    assert!(rlp.list().is_err());
}

#[test]
fn test_rlp_invalid_prefix_values() {
    // Test with prefix that would require more than 4 length bytes
    let malformed = [0xbc, 0x01, 0x02, 0x03, 0x04, 0x05]; // 5 length bytes
    let mut rlp = Rlp::new(&malformed);
    assert!(rlp.item().is_err());

    // Test list with too many length bytes
    let malformed = [0xfc, 0x01, 0x02, 0x03, 0x04, 0x05]; // List with 5 length bytes
    let mut rlp = Rlp::new(&malformed);
    assert!(rlp.item().is_err());

    // Test invalid string length claim
    let malformed = [0x85]; // String claiming 5 bytes
    let mut rlp = Rlp::new(&malformed); // But no data follows
    assert!(rlp.bytes().is_err());

    // Test with byte that should be encoded as single but isn't
    let malformed = [0x81, 0x01]; // Encoding for single byte 1 (should just be 0x01)
    let mut rlp = Rlp::new(&malformed);
    let result = rlp.bytes().unwrap();
    assert_eq!(result, &[0x01]); // Should still work but is non-canonical

    // Test invalid list length claim
    let malformed = [0xc5]; // List claiming 5 bytes
    let mut rlp = Rlp::new(&malformed); // But no data follows
    assert!(rlp.list().is_err());
}

#[test]
fn test_rlp_memory_exhaustion_scenarios() {
    // Test extremely large claimed string length
    let mut exhaustion = vec![0xba]; // Long string with 3-byte length
    exhaustion.extend_from_slice(&[0x10, 0x00, 0x00]); // ~1MB claimed
    let mut rlp = Rlp::new(&exhaustion);
    assert!(rlp.bytes().is_err()); // Should fail due to insufficient data

    // Test extremely large claimed list length
    let mut exhaustion = vec![0xfa]; // Long list with 3-byte length
    exhaustion.extend_from_slice(&[0x10, 0x00, 0x00]); // ~1MB claimed
    let mut rlp = Rlp::new(&exhaustion);
    assert!(rlp.list().is_err()); // Should fail due to insufficient data

    // Test with reasonable but large actual data (to ensure we don't just check claimed size)
    let large_string = "x".repeat(1000);
    let mut alloy_encoded = Vec::new();
    large_string.as_bytes().encode(&mut alloy_encoded);
    let mut rlp = Rlp::new(&alloy_encoded);
    let decoded = rlp.bytes().unwrap();
    assert_eq!(decoded, large_string.as_bytes()); // Should work for legitimate large data
}

#[test]
fn test_rlp_boundary_condition_errors() {
    // Test edge cases around specific byte values

    // Test maximum valid single byte (0x7f)
    let max_single = [0x7f];
    let mut rlp = Rlp::new(&max_single);
    assert_eq!(rlp.bytes().unwrap(), &[0x7f]);

    // Test minimum short string (0x80 = empty)
    let min_short = [0x80];
    let mut rlp = Rlp::new(&min_short);
    assert_eq!(rlp.bytes().unwrap(), &[] as &[u8]);

    // Test maximum short string length (0xb7 = 55 bytes)
    let mut max_short = vec![0xb7];
    max_short.extend_from_slice(&vec![0x42; 55]);
    let mut rlp = Rlp::new(&max_short);
    assert_eq!(rlp.bytes().unwrap(), &vec![0x42; 55]);

    // Test minimum long string (0xb8 = long with 1 length byte)
    let min_long = vec![0xb8, 0x38, 0x42]; // 56 bytes claimed but only 1 byte data
    let mut rlp = Rlp::new(&min_long);
    assert!(rlp.bytes().is_err()); // Should fail - insufficient data

    // Test corrupted length bytes
    let mut corrupted = vec![0xb9]; // Claims 2 length bytes
    corrupted.push(0x00); // But only provides 1
    let mut rlp = Rlp::new(&corrupted);
    assert!(rlp.bytes().is_err());
}
