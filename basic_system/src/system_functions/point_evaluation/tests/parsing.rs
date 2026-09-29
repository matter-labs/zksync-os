use super::*;

/// Test versioned hash computation function
#[test]
fn test_versioned_hash_for_kzg() {
    let commitment = [0u8; 48]; // Identity commitment
    let hash = versioned_hash_for_kzg(&commitment);

    assert_eq!(hash[0], VERSIONED_HASH_VERSION_KZG);

    let expected_hash = [
        1, 176, 118, 31, 135, 176, 129, 213, 207, 16, 117, 124, 204, 137, 241, 43, 227, 85, 199,
        14, 46, 41, 223, 40, 139, 101, 179, 7, 16, 220, 188, 209,
    ];
    assert_eq!(hash, expected_hash);
}

/// Test scalar parsing edge cases
#[test]
fn test_parse_scalar_edge_cases() {
    // Test maximum valid scalar (modulus - 1)
    let max_valid = [
        0x73, 0xed, 0xa7, 0x53, 0x29, 0x9d, 0x7d, 0x48, 0x33, 0x39, 0xd8, 0x08, 0x09, 0xa1, 0xd8,
        0x05, 0x53, 0xbd, 0xa4, 0x02, 0xff, 0xfe, 0x5b, 0xfe, 0xff, 0xff, 0xff, 0xff, 0x00, 0x00,
        0x00, 0x00,
    ];
    assert!(parse_scalar(&max_valid).is_ok());

    // Test minimum invalid scalar (modulus)
    let min_invalid = [
        0x73, 0xed, 0xa7, 0x53, 0x29, 0x9d, 0x7d, 0x48, 0x33, 0x39, 0xd8, 0x08, 0x09, 0xa1, 0xd8,
        0x05, 0x53, 0xbd, 0xa4, 0x02, 0xff, 0xfe, 0x5b, 0xfe, 0xff, 0xff, 0xff, 0xff, 0x00, 0x00,
        0x00, 0x01,
    ];
    assert!(parse_scalar(&min_invalid).is_err());

    // Test zero (always valid)
    let zero = [0u8; 32];
    assert!(parse_scalar(&zero).is_ok());
}

/// Test parse_g1_compressed edge cases
#[test]
fn test_parse_g1_compressed_edge_cases() {
    // Test valid identity element (point at infinity)
    let identity = [
        0xc0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00,
    ];
    assert!(
        parse_g1_compressed(&identity).is_ok(),
        "Identity point should be valid"
    );

    // Test wrong input size - too short
    let too_short = [0u8; 47]; // One byte short
    assert!(
        parse_g1_compressed(&too_short).is_err(),
        "Input too short should fail"
    );

    // Test wrong input size - too long
    let too_long = [0u8; 49]; // One byte too long
    assert!(
        parse_g1_compressed(&too_long).is_err(),
        "Input too long should fail"
    );

    // Test all zeros (not a valid compressed point)
    let all_zeros = [0u8; 48];
    assert!(
        parse_g1_compressed(&all_zeros).is_err(),
        "All zeros should be invalid"
    );

    // Test all ones (invalid field element)
    let all_ones = [0xffu8; 48];
    assert!(
        parse_g1_compressed(&all_ones).is_err(),
        "All ones should be invalid"
    );

    // Test invalid compression flag (neither compressed nor uncompressed)
    let invalid_flag = [
        0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00,
    ];
    assert!(
        parse_g1_compressed(&invalid_flag).is_err(),
        "Invalid compression flag should fail"
    );

    // Test x-coordinate >= field modulus (invalid field element)
    let invalid_x = [
        0x9a, 0x0d, 0x51, 0xcc, 0x7f, 0xa0, 0x52, 0xe0, 0xc9, 0x9d, 0x3e, 0xa2, 0x42, 0x78, 0x10,
        0x5b, 0xf0, 0x1c, 0x29, 0x94, 0x3d, 0xa1, 0x8e, 0xf2, 0x50, 0x51, 0x73, 0x37, 0x8a, 0x64,
        0xa2, 0x61, 0x05, 0x43, 0x48, 0x44, 0x31, 0x15, 0x66, 0x5b, 0x5e, 0x96, 0x4e, 0x9b, 0x4a,
        0x3c, 0x7c, 0x59,
    ];
    assert!(
        parse_g1_compressed(&invalid_x).is_err(),
        "X-coordinate >= field modulus should fail"
    );

    // Test point not on curve (valid x-coordinate but no corresponding y)
    let not_on_curve = [
        0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x02,
    ];
    assert!(
        parse_g1_compressed(&not_on_curve).is_err(),
        "Point not on curve should fail"
    );

    // Test identity with wrong infinity flag
    let wrong_infinity = [
        0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00,
    ];
    assert!(
        parse_g1_compressed(&wrong_infinity).is_err(),
        "Wrong infinity flag should fail"
    );
}

/// Test parse_g1_compressed with known valid points
#[test]
fn test_parse_g1_compressed_known_valid_points() {
    // Test with the actual BLS12-381 generator point (compressed)
    let generator_compressed = hex!("97f1d3a73197d7942695638c4fa9ac0fc3688c4f9774b905a14e3a3f171bac586c55e83ff97a1aeffb3af00adb22c6bb");
    let result = parse_g1_compressed(&generator_compressed);
    assert!(
        result.is_ok(),
        "BLS12-381 generator should parse successfully"
    );

    // Test a known valid point from test vectors
    let valid_point = hex!("8f59a8d2a1a625a17f3fea0fe5eb8c896db3764f3185481bc22f91b4aaffcca25f26936857bc3a7c2539ea8ec3a952b7");
    let result = parse_g1_compressed(&valid_point);
    assert!(
        result.is_ok(),
        "Known valid point should parse successfully"
    );

    // Test another known valid point with y-bit set
    let valid_point_y_bit = hex!("a62ad71d14c5719385c0686f1871430475bf3a00f0aa3f7b8dd99a9abc2160744faf0070725e00b60ad9a026a15b1a8c");
    let result = parse_g1_compressed(&valid_point_y_bit);
    assert!(
        result.is_ok(),
        "Known valid point with y-bit should parse successfully"
    );
}

/// Test parse_g1_compressed error conditions comprehensively
#[test]
fn test_parse_g1_compressed_comprehensive_errors() {
    // Test various invalid compression flag combinations
    let invalid_flags = [
        0x00, // No compression bit set
        0x20, // Reserved bit set
        0x60, // Multiple reserved bits
        0xe0, // All flag bits except infinity
    ];

    for &flag in &invalid_flags {
        let mut invalid_point = [0u8; 48];
        invalid_point[0] = flag;
        assert!(
            parse_g1_compressed(&invalid_point).is_err(),
            "Invalid flag 0x{:02x} should fail",
            flag
        );
    }

    // Test infinity point with non-zero coordinates (should fail)
    let mut invalid_infinity = [0u8; 48];
    invalid_infinity[0] = 0xc0; // Infinity flag
    invalid_infinity[47] = 0x01; // Non-zero coordinate
    assert!(
        parse_g1_compressed(&invalid_infinity).is_err(),
        "Infinity point with non-zero coordinates should fail"
    );

    // Test compressed point with both infinity and y-bit flags
    let mut invalid_mixed = [0u8; 48];
    invalid_mixed[0] = 0xf0; // Both infinity and y-bit flags
    assert!(
        parse_g1_compressed(&invalid_mixed).is_err(),
        "Point with both infinity and y-bit flags should fail"
    );
}
