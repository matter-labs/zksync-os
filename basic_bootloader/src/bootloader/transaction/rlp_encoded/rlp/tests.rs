use crate::bootloader::transaction::rlp_encoded::rlp::minimal_rlp_parser::{Rlp, RlpFixedItem};

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

        let alloy_decoded: u64 =
            match AlloyRlp::new(&alloy_encoded).and_then(|mut decoder| decoder.get_next::<u64>()) {
                Ok(Some(val)) => val,
                _ => continue, // Skip if Alloy can't decode this value
            };
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

        let alloy_decoded: Bytes = match AlloyRlp::new(&alloy_encoded)
            .and_then(|mut decoder| decoder.get_next::<Bytes>())
        {
            Ok(Some(val)) => val,
            _ => continue, // Skip if Alloy can't decode this value
        };
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

        let alloy_decoded: alloy_primitives::U256 = match AlloyRlp::new(&alloy_encoded)
            .and_then(|mut decoder| decoder.get_next::<alloy_primitives::U256>())
        {
            Ok(Some(val)) => val,
            _ => continue, // Skip if Alloy can't decode this value
        };
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
fn test_alloy_edge_case_truncated_data() {
    // Test various truncated data scenarios
    let test_cases = [
        // Truncated short string
        vec![0x83, 0x64, 0x6f], // Claims 3 bytes, has 2
        // Truncated long string header
        vec![0xb8],       // Claims long string but no length
        vec![0xb9, 0x00], // Claims 2-byte length but incomplete
        // Truncated list
        vec![0xc3, 0x01, 0x02], // Claims 3 bytes, has 2
        // Truncated long list header
        vec![0xf8],       // Claims long list but no length
        vec![0xf9, 0x00], // Claims 2-byte length but incomplete
    ];

    for test_data in test_cases.iter() {
        // Test with our parser
        let mut rlp_parser = Rlp::new(test_data);
        let our_bytes_result = rlp_parser.bytes();

        let mut rlp_parser_list = Rlp::new(test_data);
        let our_list_result = rlp_parser_list.list();

        // Test with Alloy parser
        let alloy_bytes_result =
            AlloyRlp::new(test_data).and_then(|mut decoder| decoder.get_next::<Bytes>());
        let alloy_list_result =
            AlloyRlp::new(test_data).and_then(|mut decoder| decoder.get_next::<Vec<u8>>());

        // Both should fail for truncated data
        assert!(our_bytes_result.is_err() && alloy_bytes_result.is_err());
        assert!(our_list_result.is_err() && alloy_list_result.is_err());
    }
}

#[test]
fn test_alloy_edge_case_non_minimal_encoding() {
    // Test non-minimal encodings that technically work but are non-canonical
    let test_cases = [
        // Single byte encoded as short string (non-minimal)
        vec![0x81, 0x00], // Should be just [0x00]
        vec![0x81, 0x01], // Should be just [0x01]
        vec![0x81, 0x7f], // Should be just [0x7f]
        // Short string that could be single byte
        vec![0x81, 0x80], // This is actually correct encoding for byte 0x80
        // Zero-length long string (should use short form 0x80)
        vec![0xb8, 0x00], // Should be just [0x80]
        // One-byte string with long encoding (should use short form)
        vec![0xb8, 0x01, 0x41], // Should be [0x81, 0x41]
    ];

    for (i, test_data) in test_cases.iter().enumerate() {
        // Test with our parser
        let mut rlp_parser = Rlp::new(test_data);
        let our_result = rlp_parser.bytes();

        // Test with Alloy parser
        let alloy_result: Result<Bytes, alloy_rlp::Error> =
            alloy_rlp::Decodable::decode(&mut &test_data[..]);

        // Both parsers should handle non-minimal encodings similarly
        // (They might accept them or reject them, but should be consistent)
        match (our_result, alloy_result) {
            (Ok(our_bytes), Ok(alloy_bytes)) => {
                assert_eq!(
                    our_bytes, alloy_bytes.0,
                    "Case {}: Non-minimal encoding gave different results",
                    i
                );
            }
            (Err(_), Err(_)) => {
                // Both rejected - consistent behavior
            }
            (our_res, alloy_res) => {
                panic!(
                    "Case {}: Divergence - our={:?}, alloy={:?}",
                    i, our_res, alloy_res
                );
            }
        }
    }
}

#[test]
fn test_alloy_edge_case_large_length_claims() {
    // Test extremely large length claims
    let test_cases = [
        // Large string claim with insufficient data
        vec![0xbb, 0x01, 0x00, 0x00, 0x00], // Claims 16MB, has 0 bytes
        vec![0xba, 0xff, 0xff, 0xff],       // Claims ~16MB with 3-byte length, has 0 bytes
        // Large list claim with insufficient data
        vec![0xfb, 0x01, 0x00, 0x00, 0x00], // Claims 16MB list, has 0 bytes
        vec![0xfa, 0xff, 0xff, 0xff],       // Claims ~16MB list with 3-byte length, has 0 bytes
        // Maximum length claims
        vec![0xbb, 0xff, 0xff, 0xff, 0xff], // Claims max u32 bytes
        vec![0xfb, 0xff, 0xff, 0xff, 0xff], // Claims max u32 list bytes
    ];

    for test_data in test_cases.iter() {
        // Test with our parser
        let mut rlp_parser = Rlp::new(test_data);
        let our_bytes_result = rlp_parser.bytes();

        let mut rlp_parser_list = Rlp::new(test_data);
        let our_list_result = rlp_parser_list.list();

        // Test with Alloy parser
        let alloy_bytes_result: Result<Bytes, alloy_rlp::Error> =
            alloy_rlp::Decodable::decode(&mut &test_data[..]);
        let alloy_list_result: Result<Vec<u8>, alloy_rlp::Error> =
            alloy_rlp::Decodable::decode(&mut &test_data[..]);

        // Both should fail gracefully (not panic or OOM)
        assert!(our_bytes_result.is_err() && alloy_bytes_result.is_err());
        assert!(our_list_result.is_err() && alloy_list_result.is_err());
    }
}

#[test]
fn test_alloy_edge_case_empty_and_zero_values_bytes() {
    // Test various representations of empty/zero values
    let test_cases = [
        // Empty string representations
        (vec![0x80], "empty string"),
        // Zero as different encodings
        (vec![0x00], "zero as single byte"),
    ];

    for (test_data, description) in test_cases.iter() {
        // Test string/bytes decoding
        let mut rlp_parser = Rlp::new(test_data);
        let our_bytes_result = rlp_parser.bytes();

        let alloy_bytes_result: Result<Bytes, alloy_rlp::Error> =
            alloy_rlp::Decodable::decode(&mut &test_data[..]);

        // Test list decoding
        let mut rlp_parser_list = Rlp::new(test_data);
        let our_list_result = rlp_parser_list.list();

        let alloy_list_result: Result<Vec<u8>, alloy_rlp::Error> =
            alloy_rlp::Decodable::decode(&mut &test_data[..]);

        assert!(our_list_result.is_err() && alloy_list_result.is_err());

        match (our_bytes_result, alloy_bytes_result) {
            (Ok(our_bytes), Ok(alloy_bytes)) => {
                assert_eq!(
                    our_bytes, alloy_bytes.0,
                    "{}: Byte decoding mismatch",
                    description
                );
            }
            (Err(_), Err(_)) => {
                // Both failed - acceptable
            }
            (our_res, alloc_res) => {
                panic!(
                    "Test '{}': Divergence - our={:?}, alloy={:?}",
                    description, our_res, alloc_res
                );
            }
        }
    }
}

#[test]
fn test_alloy_edge_case_empty_and_zero_values_lists() {
    // Test various representations of empty/zero values
    let test_cases = [
        // Empty list representations
        (vec![0xc0], "empty list"),
        (vec![0xf8, 0x00], "empty list (long form)"),
    ];

    for (test_data, description) in test_cases.iter() {
        // Test string/bytes decoding
        let mut rlp_parser = Rlp::new(test_data);
        let our_bytes_result = rlp_parser.bytes();

        let alloy_bytes_result: Result<Bytes, alloy_rlp::Error> =
            alloy_rlp::Decodable::decode(&mut &test_data[..]);

        // Test list decoding
        let mut rlp_parser_list = Rlp::new(test_data);
        let our_list_result = rlp_parser_list.list();

        let alloy_list_result: Result<Vec<u8>, alloy_rlp::Error> =
            alloy_rlp::Decodable::decode(&mut &test_data[..]);

        assert!(our_bytes_result.is_err() && alloy_bytes_result.is_err());

        // For valid encodings, results should match
        match (our_list_result, alloy_list_result) {
            (Ok(our_list), Ok(alloy_list)) => {
                assert_eq!(
                    alloy_list,
                    our_list.remaining(),
                    "{}: List decoding mismatch",
                    description
                );
            }
            (Err(_), Err(_)) => {
                // Both failed - acceptable
            }
            (our_res, alloc_res) => {
                panic!(
                    "Test '{}': Divergence - our={:?}, alloy={:?}",
                    description, our_res, alloc_res
                );
            }
        }
    }
}
