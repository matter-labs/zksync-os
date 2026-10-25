use crate::cost_constants::{POINT_EVALUATION_COST_GAS, POINT_EVALUATION_NATIVE_COST};
use crate::system_functions::field_ops::ZkOsHintOracle;
use crypto::hints::{HintOracle, NoHints};
use crypto::precompiles::{kzg_verify_proof, verify_kzg_proof_with_oracle, KzgError};
use zk_ee::common_traits::TryExtend;
use zk_ee::interface_error;
use zk_ee::oracle::IOOracle;
use zk_ee::out_of_return_memory;
use zk_ee::system::errors::subsystem::SubsystemError;
use zk_ee::system::logger::Logger;
use zk_ee::system::*;

pub use crypto::precompiles::{parse_g1_compressed, KzgScalar};

///
/// Point evaluation system function implementation.
/// With `USE_ADVICE`, the square roots of the point decompressions, the field divisions of the
/// curve arithmetic (in affine coordinates) and the inversion of the final exponentiation are
/// taken from oracle hints.
///
pub struct PointEvaluationImpl<const USE_ADVICE: bool>;

impl<R: Resources, const USE_ADVICE: bool> SystemFunctionExt<R, PointEvaluationErrors>
    for PointEvaluationImpl<USE_ADVICE>
{
    /// Returns `OutOfGas` if not enough resources provided, resources may be not touched.
    ///
    /// Returns `InvalidInputSize` error if `input_len` != 192,
    /// `InvalidPoint` if commitment or proof point encoded incorrectly,
    /// `InvalidScalar` if `z` or `y` scalars encoded incorrectly,
    /// `InvalidVersionedHash` if versioned hash doesn't correspond to the commitment,
    /// `PairingMismatch` if kzg proof pairing check failed.
    fn execute<
        O: IOOracle,
        L: Logger,
        D: TryExtend<u8> + ?Sized,
        A: core::alloc::Allocator + Clone,
    >(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        oracle: &mut O,
        _logger: &mut L,
        _allocator: A,
    ) -> Result<(), SubsystemError<PointEvaluationErrors>> {
        cycle_marker::wrap_with_resources!("point_evaluation", resources, {
            let oracle = if USE_ADVICE {
                Some(ZkOsHintOracle::from_mut(oracle))
            } else {
                None
            };
            point_evaluation_as_system_function_inner(input, output, resources, oracle)
        })
    }
}

pub const POINT_EVAL_PRECOMPILE_SUCCESS_RESPONSE: [u8; 64] = const {
    // u256_be(4096) || u256_be(BLS12-381 Fr characteristic)
    let Ok(res) = const_hex::const_decode_to_array(
        b"000000000000000000000000000000000000000000000000000000000000100073eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001"
    ) else {
        panic!()
    };

    res
};

pub fn versioned_hash_for_kzg(data: &[u8]) -> [u8; 32] {
    use crypto::sha256::Digest;
    let mut hash: [u8; 32] = crypto::sha256::Sha256::digest(data).into();
    hash[0] = VERSIONED_HASH_VERSION_KZG;

    hash
}

#[inline(always)]
pub fn verify_kzg_proof(
    commitment: crypto::bls12_381::G1Affine,
    proof: crypto::bls12_381::G1Affine,
    z: KzgScalar,
    y: KzgScalar,
) -> bool {
    verify_kzg_proof_with_oracle::<NoHints>(commitment, proof, z, y, None)
}

fn point_evaluation_as_system_function_inner<
    D: ?Sized + TryExtend<u8>,
    R: Resources,
    O: HintOracle,
>(
    input: &[u8],
    dst: &mut D,
    resources: &mut R,
    oracle: Option<&mut O>,
) -> Result<(), SubsystemError<PointEvaluationErrors>> {
    resources
        .charge_legacy_gas_and_native(POINT_EVALUATION_COST_GAS, POINT_EVALUATION_NATIVE_COST)?;

    if input.len() != 192 {
        return Err(interface_error!(
            PointEvaluationInterfaceError::InvalidInputSize
        ));
    }

    // Each check without any parsing
    let versioned_hash = &input[..32];
    let commitment = &input[96..144];

    // so far it's just one version
    if versioned_hash_for_kzg(commitment) != versioned_hash {
        return Err(interface_error!(
            PointEvaluationInterfaceError::InvalidVersionedHash
        ));
    }

    // Parse the commitment and proof, then the scalars, and check the proof
    let proof = &input[144..192];
    let z = input[32..64].try_into().unwrap();
    let y = input[64..96].try_into().unwrap();
    match kzg_verify_proof(commitment, proof, z, y, oracle) {
        Ok(true) => {
            dst.try_extend(POINT_EVAL_PRECOMPILE_SUCCESS_RESPONSE)
                .map_err(|_| out_of_return_memory!())?;
            Ok(())
        }
        Ok(false) => Err(interface_error!(
            PointEvaluationInterfaceError::PairingMismatch
        )),
        Err(KzgError::InvalidPoint) => Err(interface_error!(
            PointEvaluationInterfaceError::InvalidPoint
        )),
        Err(KzgError::InvalidScalar) => Err(interface_error!(
            PointEvaluationInterfaceError::InvalidScalar
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use callable_oracles::field_hints::NativeFieldOpsQuery;
    use oracle_provider::ZkEENonDeterminismSource;
    use std::alloc::Global;
    use zk_ee::reference_implementations::BaseResources;
    use zk_ee::reference_implementations::DecreasingNative;
    use zk_ee::system::logger::NullLogger;
    use zk_ee::system::Resource;

    /// Runs the precompile without and with the oracle hints; both must agree
    fn execute(
        input: &[u8],
        output: &mut Vec<u8>,
        resources: &mut TestResources,
    ) -> Result<(), SubsystemError<PointEvaluationErrors>> {
        let mut oracle = ZkEENonDeterminismSource::default();
        oracle.add_external_processor(NativeFieldOpsQuery);
        let mut hinted_output = Vec::new();
        let mut hinted_resources = infinite_resources();
        let hinted = PointEvaluationImpl::<true>::execute(
            input,
            &mut hinted_output,
            &mut hinted_resources,
            &mut oracle,
            &mut NullLogger,
            Global,
        );
        let result = PointEvaluationImpl::<false>::execute(
            input,
            output,
            resources,
            &mut oracle,
            &mut NullLogger,
            Global,
        );
        assert_eq!(result.is_ok(), hinted.is_ok(), "{result:?} vs {hinted:?}");
        assert_eq!(*output, hinted_output);
        assert_eq!(resources.legacy_gas(), hinted_resources.legacy_gas());
        // a prover claiming an invalid proof (with a correct inverse) does not change the answer
        let mut claiming =
            crate::system_functions::field_ops::test_oracles::non_identity_claiming_oracle();
        let mut claiming_output = Vec::new();
        let claimed = PointEvaluationImpl::<true>::execute(
            input,
            &mut claiming_output,
            &mut infinite_resources(),
            &mut claiming,
            &mut NullLogger,
            Global,
        );
        assert_eq!(result.is_ok(), claimed.is_ok(), "{result:?} vs {claimed:?}");
        assert_eq!(*output, claiming_output);
        result
    }

    use alloy_primitives::hex;

    type TestResources = BaseResources<DecreasingNative>;

    fn infinite_resources() -> TestResources {
        TestResources::FORMAL_INFINITE
    }

    #[test]
    fn basic_test() {
        // Test data from: https://github.com/ethereum/c-kzg-4844/blob/main/tests/verify_kzg_proof/kzg-mainnet/verify_kzg_proof_case_correct_proof_4_4/data.yaml

        let commitment = hex!("8f59a8d2a1a625a17f3fea0fe5eb8c896db3764f3185481bc22f91b4aaffcca25f26936857bc3a7c2539ea8ec3a952b7").to_vec();

        use crypto::sha256::*;
        let mut hasher = Sha256::new();
        hasher.update(commitment.clone());
        let mut versioned_hash = hasher.finalize().to_vec();
        versioned_hash[0] = VERSIONED_HASH_VERSION_KZG;

        let z = hex!("73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000000").to_vec();
        let y = hex!("1522a4a7f34e1ea350ae07c29c96c7e79655aa926122e95fe69fcbd932ca49e9").to_vec();
        let proof = hex!("a62ad71d14c5719385c0686f1871430475bf3a00f0aa3f7b8dd99a9abc2160744faf0070725e00b60ad9a026a15b1a8c").to_vec();

        let input = [versioned_hash, z, y, commitment, proof].concat();

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
        let commitment =
            hex!("8f59a8d2a1a625a17f3fea0fe5eb8c896db3764f3185481bc22f91b4aaffcca25f26936857bc3a7c2539ea8ec3a952b7");
        let proof =
            hex!("a62ad71d14c5719385c0686f1871430475bf3a00f0aa3f7b8dd99a9abc2160744faf0070725e00b60ad9a026a15b1a8c");
        let z = hex!("73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000000");
        let y = hex!("1522a4a7f34e1ea350ae07c29c96c7e79655aa926122e95fe69fcbd932ca49e9");

        // the scalars of the test are canonical
        let parse_scalar =
            |bytes: &[u8; 32]| -> Result<KzgScalar, ()> { Ok(crypto::parse_u256_be(bytes)) };
        let commitment = parse_g1_compressed(&commitment).unwrap();
        let proof = parse_g1_compressed(&proof).unwrap();
        let z = parse_scalar(&z).unwrap();
        let y = parse_scalar(&y).unwrap();

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
        let commitment = hex!("8f59a8d2a1a625a17f3fea0fe5eb8c896db3764f3185481bc22f91b4aaffcca25f26936857bc3a7c2539ea8ec3a952b7").to_vec();
        let versioned_hash = versioned_hash_for_kzg(&commitment).to_vec();
        let z = hex!("73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000000").to_vec();
        let y = hex!("1522a4a7f34e1ea350ae07c29c96c7e79655aa926122e95fe69fcbd932ca49e9").to_vec();
        let proof = hex!("a62ad71d14c5719385c0686f1871430475bf3a00f0aa3f7b8dd99a9abc2160744faf0070725e00b60ad9a026a15b1a8c").to_vec();
        let input = [versioned_hash, z, y, commitment, proof].concat();
        let mut lying = crate::system_functions::field_ops::test_oracles::lying_oracle(&[
            crypto::hints::FieldHintOp::Bls12381KzgResidueWitness,
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
    fn test_invalid_input() {
        let commitment = hex!("c00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").to_vec();

        use crypto::sha256::*;
        let mut hasher = Sha256::new();
        hasher.update(commitment.clone());
        let mut versioned_hash = hasher.finalize().to_vec();
        versioned_hash[0] = VERSIONED_HASH_VERSION_KZG;

        let z = hex!("0000000000000000000000000000000000000000000000000000000000000000").to_vec();
        let y = hex!("73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000001").to_vec();
        let proof = hex!("c00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").to_vec();

        let input = [versioned_hash, z, y, commitment, proof].concat();

        let mut output = Vec::new();
        let mut resources = infinite_resources();

        let result = execute(&input, &mut output, &mut resources);
        assert!(result.is_err(), "Result: {:?}", result);
    }

    /// Test invalid input size - too short
    #[test]
    fn test_point_evaluation_invalid_input_size_short() {
        let input = vec![0u8; 191]; // One byte short
        let mut output = Vec::new();
        let mut resources = infinite_resources();

        let result = execute(&input, &mut output, &mut resources);

        assert!(result.is_err());
        if let Err(SubsystemError::LeafUsage(err)) = result {
            if let PointEvaluationInterfaceError::InvalidInputSize = err.0 {
                // Expected error
            } else {
                panic!("Expected InvalidInputSize error, got: {:?}", err);
            }
        } else {
            panic!("Expected InvalidInputSize error, got: {:?}", result);
        }
    }

    /// Test invalid input size - too long
    #[test]
    fn test_point_evaluation_invalid_input_size_long() {
        let input = vec![0u8; 193]; // One byte too long
        let mut output = Vec::new();
        let mut resources = infinite_resources();

        let result = execute(&input, &mut output, &mut resources);

        assert!(result.is_err());
        if let Err(SubsystemError::LeafUsage(err)) = result {
            if let PointEvaluationInterfaceError::InvalidInputSize = err.0 {
                // Expected error
            } else {
                panic!("Expected InvalidInputSize error, got: {:?}", err);
            }
        } else {
            panic!("Expected InvalidInputSize error, got: {:?}", result);
        }
    }

    /// Test invalid scalar - z >= field modulus
    #[test]
    fn test_point_evaluation_invalid_scalar_z() {
        let commitment = hex!("8f59a8d2a1a625a17f3fea0fe5eb8c896db3764f3185481bc22f91b4aaffcca25f26936857bc3a7c2539ea8ec3a952b7").to_vec();

        use crypto::sha256::*;
        let mut hasher = Sha256::new();
        hasher.update(commitment.clone());
        let mut versioned_hash = hasher.finalize().to_vec();
        versioned_hash[0] = VERSIONED_HASH_VERSION_KZG;

        // Set z to field modulus (invalid)
        let invalid_z = [
            0x73, 0xed, 0xa7, 0x53, 0x29, 0x9d, 0x7d, 0x48, 0x33, 0x39, 0xd8, 0x08, 0x09, 0xa1,
            0xd8, 0x05, 0x53, 0xbd, 0xa4, 0x02, 0xff, 0xfe, 0x5b, 0xfe, 0xff, 0xff, 0xff, 0xff,
            0x00, 0x00, 0x00, 0x01,
        ]
        .to_vec();
        let y = hex!("1522a4a7f34e1ea350ae07c29c96c7e79655aa926122e95fe69fcbd932ca49e9").to_vec();
        let proof = hex!("a62ad71d14c5719385c0686f1871430475bf3a00f0aa3f7b8dd99a9abc2160744faf0070725e00b60ad9a026a15b1a8c").to_vec();

        let input = [versioned_hash, invalid_z, y, commitment, proof].concat();

        let mut output = Vec::new();
        let mut resources = infinite_resources();

        let result = execute(&input, &mut output, &mut resources);

        assert!(result.is_err());
        if let Err(SubsystemError::LeafUsage(err)) = result {
            if let PointEvaluationInterfaceError::InvalidScalar = err.0 {
                // Expected error
            } else {
                panic!("Expected InvalidScalar error, got: {:?}", err);
            }
        } else {
            panic!("Expected InvalidScalar error, got: {:?}", result);
        }
    }

    /// Test invalid scalar - y >= field modulus
    #[test]
    fn test_point_evaluation_invalid_scalar_y() {
        let commitment = hex!("8f59a8d2a1a625a17f3fea0fe5eb8c896db3764f3185481bc22f91b4aaffcca25f26936857bc3a7c2539ea8ec3a952b7").to_vec();

        use crypto::sha256::*;
        let mut hasher = Sha256::new();
        hasher.update(commitment.clone());
        let mut versioned_hash = hasher.finalize().to_vec();
        versioned_hash[0] = VERSIONED_HASH_VERSION_KZG;

        let z = hex!("73eda753299d7d483339d80809a1d80553bda402fffe5bfeffffffff00000000").to_vec();
        // Set y to field modulus (invalid)
        let invalid_y = [
            0x73, 0xed, 0xa7, 0x53, 0x29, 0x9d, 0x7d, 0x48, 0x33, 0x39, 0xd8, 0x08, 0x09, 0xa1,
            0xd8, 0x05, 0x53, 0xbd, 0xa4, 0x02, 0xff, 0xfe, 0x5b, 0xfe, 0xff, 0xff, 0xff, 0xff,
            0x00, 0x00, 0x00, 0x01,
        ]
        .to_vec();
        let proof = hex!("a62ad71d14c5719385c0686f1871430475bf3a00f0aa3f7b8dd99a9abc2160744faf0070725e00b60ad9a026a15b1a8c").to_vec();

        let input = [versioned_hash, z, invalid_y, commitment, proof].concat();

        let mut output = Vec::new();
        let mut resources = infinite_resources();

        let result = execute(&input, &mut output, &mut resources);

        assert!(result.is_err());
        if let Err(SubsystemError::LeafUsage(err)) = result {
            if let PointEvaluationInterfaceError::InvalidScalar = err.0 {
                // Expected error
            } else {
                panic!("Expected InvalidScalar error, got: {:?}", err);
            }
        } else {
            panic!("Expected InvalidScalar error, got: {:?}", result);
        }
    }

    /// Test versioned hash computation function
    #[test]
    fn test_versioned_hash_for_kzg() {
        let commitment = [0u8; 48]; // Identity commitment
        let hash = versioned_hash_for_kzg(&commitment);

        assert_eq!(hash[0], VERSIONED_HASH_VERSION_KZG);

        let expected_hash = [
            1, 176, 118, 31, 135, 176, 129, 213, 207, 16, 117, 124, 204, 137, 241, 43, 227, 85,
            199, 14, 46, 41, 223, 40, 139, 101, 179, 7, 16, 220, 188, 209,
        ];
        assert_eq!(hash, expected_hash);
    }
}
