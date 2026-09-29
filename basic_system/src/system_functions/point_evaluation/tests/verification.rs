use super::*;

#[test]
fn basic_test() {
    let input = valid_a();

    let expected_output = hex!("000000000000000000000000000000000000000000000000000000000000100073eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001");
    let gas = 50000;

    let mut output = Vec::new();
    let mut resources = infinite_resources();
    let gas_before = resources.legacy_gas();

    let result = execute(&input, &mut output, &mut resources);
    assert!(result.is_ok(), "Result: {:?}", result);

    let gas_used = gas_before - resources.legacy_gas();

    assert_eq!(gas_used, gas);
    assert_eq!(output[..], expected_output);
}

#[test]
fn test_rearranged_kzg_verifier_matches_reference() {
    let input = valid_a();
    let commitment = parse_g1_compressed(&input[96..144]).unwrap();
    let proof = parse_g1_compressed(&input[144..192]).unwrap();
    let z = parse_scalar(input[32..64].try_into().unwrap()).unwrap();
    let y = parse_scalar(input[64..96].try_into().unwrap()).unwrap();

    let local = verify_kzg_proof(commitment, proof, z, y);
    let reference = crypto::bls12_381::verify_kzg_proof(commitment, proof, z, y);
    assert!(local, "valid proof should verify");
    assert_eq!(local, reference);

    let unrelated_128_bit_z =
        hex!("000000000000000000000000000000000123456789abcdef0123456789abcdef");
    let unrelated_128_bit_z = parse_scalar(&unrelated_128_bit_z).unwrap();
    assert_eq!(
        verify_kzg_proof(commitment, proof, unrelated_128_bit_z, y),
        crypto::bls12_381::verify_kzg_proof(commitment, proof, unrelated_128_bit_z, y)
    );

    let zero_y = parse_scalar(&[0u8; 32]).unwrap();
    assert_eq!(
        verify_kzg_proof(commitment, proof, z, zero_y),
        crypto::bls12_381::verify_kzg_proof(commitment, proof, z, zero_y)
    );
}

#[test]
#[should_panic(expected = "claimed to be valid is wrong")]
fn wrong_identity_witness_panics() {
    let input = valid_a();
    let mut lying = curve_hints::tests::lying_oracle(&[
        crate::system_functions::field_ops::FieldHintOp::Bls12381KzgResidueWitness,
    ]);
    let _ = PointEvaluationImpl::<true>::execute(
        &input,
        &mut Vec::new(),
        &mut infinite_resources(),
        &mut lying,
        &mut NullLogger,
        Global,
    );
}

#[test]
fn invalid_inputs_return_the_expected_error() {
    let mut invalid_z = valid_a();
    invalid_z[32..64].copy_from_slice(&SCALAR_MODULUS);
    let mut invalid_y = valid_a();
    invalid_y[64..96].copy_from_slice(&SCALAR_MODULUS);
    let mut invalid_degenerate = valid_degenerate();
    invalid_degenerate[64..96].copy_from_slice(&SCALAR_MODULUS);
    use PointEvaluationInterfaceError::{InvalidInputSize, InvalidScalar};
    for (input, expected) in [
        (vec![0; 191], InvalidInputSize),
        (vec![0; 193], InvalidInputSize),
        (invalid_z, InvalidScalar),
        (invalid_y, InvalidScalar),
        (invalid_degenerate, InvalidScalar),
    ] {
        let mut output = Vec::new();
        let result = execute(&input, &mut output, &mut infinite_resources());
        assert!(matches!(result, Err(SubsystemError::LeafUsage(error)) if error.0 == expected));
        assert!(output.is_empty());
    }
}
