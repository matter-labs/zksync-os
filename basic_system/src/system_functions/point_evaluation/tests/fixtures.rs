use super::*;

pub(super) fn with_versioned_hash(z: &[u8], y: &[u8], commitment: &[u8], proof: &[u8]) -> Vec<u8> {
    [
        &versioned_hash_for_kzg(commitment)[..],
        z,
        y,
        commitment,
        proof,
    ]
    .concat()
}

/// The valid input of `basic_test`
pub(super) fn valid_a() -> Vec<u8> {
    with_versioned_hash(
        &hex!("73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000000"),
        &hex!("1522a4a7f34e1ea350ae07c29c96c7e79655aa926122e95fe69fcbd932ca49e9"),
        &hex!("8f59a8d2a1a625a17f3fea0fe5eb8c896db3764f3185481bc22f91b4aaffcca25f26936857bc3a7c2539ea8ec3a952b7"),
        &hex!("a62ad71d14c5719385c0686f1871430475bf3a00f0aa3f7b8dd99a9abc2160744faf0070725e00b60ad9a026a15b1a8c"),
    )
}

/// The valid input of the "kzg regression test" of the precompile instance tests
pub(super) fn valid_b() -> Vec<u8> {
    hex!("016685e172a749f7426a45259a2f0ca6382072faf89dc058fba39ad2792242eb55db09df7484b85d7e2870dc3e526e87b12ef1716bc52ffcf82aa07743a5d4f512d1aa968cf5c88d44a2a5815242082a27f99d2fa816838338b9532bf3cee22d91c1ac59fdf344e2a9098eb2c3699b9652a9a8efc006c4bda8a1d764b4a0fada3c34e72a0cc4d151a6dd137dde534af29517ca718c450eed935603cd5985ee6bbcfbe2da87e24e64be8fa2d81733e0de9b8a11efd7c73f39898d158fd0c4867e").to_vec()
}

/// A valid degenerate input: the commitment and the proof at infinity, y = 0 (c-kzg's
/// `verify_kzg_proof_case_correct_proof_point_at_infinity`); no square roots, and no
/// inverse for the affine conversion
pub(super) fn valid_degenerate() -> Vec<u8> {
    let mut infinity = [0u8; 48];
    infinity[0] = 0xc0;
    with_versioned_hash(
        &hex!("0000000000000000000000000000000000000000000000000000000000000002"),
        &[0u8; 32],
        &infinity,
        &infinity,
    )
}

/// `valid_a` with a wrong evaluation
pub(super) fn pairing_mismatch() -> Vec<u8> {
    let mut input = valid_a();
    input[95] ^= 1;
    input
}

/// The modulus of the scalar field, the smallest invalid scalar
pub(super) const SCALAR_MODULUS: [u8; 32] =
    hex!("73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001");

/// A compressed point whose x has no y on the curve
pub(super) const NOT_ON_CURVE: [u8; 48] = hex!("800000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002");

/// Every kind of input, with its expected result (`None`: success)
pub(super) fn vectors() -> Vec<(&'static str, Vec<u8>, Option<PointEvaluationInterfaceError>)> {
    let a = valid_a();
    let mut invalid_commitment = a.clone();
    invalid_commitment[..32].copy_from_slice(&versioned_hash_for_kzg(&NOT_ON_CURVE));
    invalid_commitment[96..144].copy_from_slice(&NOT_ON_CURVE);
    let mut invalid_proof = a.clone();
    invalid_proof[144..192].copy_from_slice(&NOT_ON_CURVE);
    let mut invalid_z = a.clone();
    invalid_z[32..64].copy_from_slice(&SCALAR_MODULUS);
    let mut invalid_y = a.clone();
    invalid_y[64..96].copy_from_slice(&SCALAR_MODULUS);
    let mut invalid_versioned_hash = a.clone();
    invalid_versioned_hash[1] ^= 1;
    let longer = [a.clone(), vec![0]].concat();
    use PointEvaluationInterfaceError::*;
    vec![
        ("valid a", a.clone(), None),
        ("valid b", valid_b(), None),
        ("valid degenerate", valid_degenerate(), None),
        (
            "pairing mismatch",
            pairing_mismatch(),
            Some(PairingMismatch),
        ),
        ("invalid commitment", invalid_commitment, Some(InvalidPoint)),
        ("invalid proof", invalid_proof, Some(InvalidPoint)),
        ("invalid z", invalid_z, Some(InvalidScalar)),
        ("invalid y", invalid_y, Some(InvalidScalar)),
        (
            "invalid versioned hash",
            invalid_versioned_hash,
            Some(InvalidVersionedHash),
        ),
        ("191 bytes", a[..191].to_vec(), Some(InvalidInputSize)),
        ("193 bytes", longer, Some(InvalidInputSize)),
        ("empty", Vec::new(), Some(InvalidInputSize)),
    ]
}
