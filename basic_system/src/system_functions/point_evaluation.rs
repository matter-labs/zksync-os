use crate::cost_constants::{POINT_EVALUATION_COST_GAS, POINT_EVALUATION_NATIVE_COST};
use crate::system_functions::curve_hints;
use crypto::ark_ec::pairing::Pairing;
use crypto::ark_ec::{AffineRepr, CurveGroup};
use crypto::ark_ff::{Field, PrimeField};
use zk_ee::common_traits::TryExtend;
use zk_ee::interface_error;
use zk_ee::oracle::IOOracle;
use zk_ee::out_of_return_memory;
use zk_ee::system::errors::subsystem::SubsystemError;
use zk_ee::system::logger::Logger;
use zk_ee::system::*;

pub type KzgScalar = <crypto::bls12_381::Fr as PrimeField>::BigInt;

///
/// Point evaluation system function implementation.
/// With `USE_ADVICE`, the square roots of the point decompressions and the field inversions
/// (of the affine conversion and of the final exponentiation) are taken from oracle hints.
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
            let oracle = if USE_ADVICE { Some(oracle) } else { None };
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

// We do not need internal representation, just canonical scalar
fn parse_scalar(input: &[u8; 32]) -> Result<KzgScalar, ()> {
    // Arkworks has strange format for integer serialization, so we do manually
    let result = crypto::parse_u256_be(input);
    if result >= crypto::bls12_381::Fr::MODULUS {
        Err(())
    } else {
        Ok(result)
    }
}

pub fn parse_g1_compressed(input: &[u8]) -> Result<crypto::bls12_381::G1Affine, ()> {
    // format coincides with one defined in ZCash/Arkworks
    use crypto::ark_serialize::CanonicalDeserialize;
    crypto::bls12_381::G1Affine::deserialize_compressed(input).map_err(|_| ())
}

/// `parse_g1_compressed` with the square root of the decompression from a checked oracle
/// hint, when an oracle is given
fn parse_g1_compressed_with_oracle<O: IOOracle>(
    input: &[u8],
    oracle: Option<&mut O>,
) -> Result<crypto::bls12_381::G1Affine, ()> {
    match oracle {
        Some(oracle) => crypto::bls12_381::g1_from_compressed_with_sqrt(input, |y_squared| {
            curve_hints::bls12_381_fq_sqrt(oracle, y_squared)
        })
        .map_err(|_| ()),
        None => parse_g1_compressed(input),
    }
}

#[inline(always)]
pub fn verify_kzg_proof(
    commitment: crypto::bls12_381::G1Affine,
    proof: crypto::bls12_381::G1Affine,
    z: KzgScalar,
    y: KzgScalar,
) -> bool {
    verify_kzg_proof_with_oracle::<oracle_provider_stub::NoOracle>(commitment, proof, z, y, None)
}

/// With an oracle, the field inversions (of the affine conversion and of the final
/// exponentiation) come from checked hints.
pub fn verify_kzg_proof_with_oracle<O: IOOracle>(
    commitment: crypto::bls12_381::G1Affine,
    proof: crypto::bls12_381::G1Affine,
    z: KzgScalar,
    y: KzgScalar,
    mut oracle: Option<&mut O>,
) -> bool {
    use crypto::bls12_381::curves::Bls12_381;
    // Original check:
    // e(yG1 - commitment, G2) * e(proof, tauG2 - zG2) == 1.
    //
    // Move the z multiplication from G2 to G1:
    // e(yG1 - commitment - z*proof, G2) * e(proof, tauG2) == 1.
    let mut left_g1 = crypto::bls12_381::G1Affine::generator().mul_bigint(&y);
    left_g1 -= &commitment;
    left_g1 -= proof.mul_bigint(&z);

    let left_g1 = match oracle.as_deref_mut() {
        Some(oracle) => crypto::hinted_ops::to_affine_with_inverse(&left_g1, |z| {
            curve_hints::bls12_381_fq_inverse(oracle, z)
        }),
        None => left_g1.into_affine(),
    };
    // both G2 points are fixed, so their Miller-loop line coefficients are constants, read
    // in place
    let g2 = [
        &crypto::bls12_381::consts::PREPARED_G2_GENERATOR,
        &crypto::bls12_381::consts::PREPARED_G2_BY_TAU,
    ];
    let Some(oracle) = oracle else {
        let miller_loop = Bls12_381::multi_miller_loop_prepared([left_g1, proof], g2);
        // the Miller loop of curve points never evaluates to zero
        let gt_el = Bls12_381::final_exponentiation_with_inverse(&miller_loop, Field::inverse)
            .expect("the Miller loop output is invertible");
        return gt_el == <Bls12_381 as Pairing>::TargetField::ONE;
    };
    // The residue witness check in place of the final exponentiation (Novakovic, Eagen, "On
    // Proving Pairings", https://eprint.iacr.org/2024/640; soundness in the documentation of
    // `crypto::residue_witness`): the Miller loop started at `d` gives `d^|u| f`, the rest is
    // one Frobenius map. The prover claims the outcome; it cannot choose it: a claimed
    // identity must come with a witness that passes the check (a failed check is a broken
    // prover and panics), and a claimed non-identity is settled by the exact final
    // exponentiation, which finds an identity all the same.
    match curve_hints::bls12_381_kzg_residue_witness(oracle, &left_g1, &proof) {
        curve_hints::PairingClaim::Identity { c: _, d, s } => {
            let l = Bls12_381::multi_miller_loop_with_initial(&d, [left_g1, proof], g2);
            assert!(
                crypto::residue_witness::bls12_381::check(&l, &d, &s),
                "the residue witness of the KZG proof claimed to be valid is wrong"
            );
            true
        }
        curve_hints::PairingClaim::NotIdentity { f_inverse } => {
            let miller_loop = Bls12_381::multi_miller_loop_prepared([left_g1, proof], g2);
            let gt_el = Bls12_381::final_exponentiation_with_inverse(&miller_loop, |f| {
                curve_hints::checked_inverse(f, f_inverse)
            })
            .expect("the Miller loop output is invertible");
            gt_el == <Bls12_381 as Pairing>::TargetField::ONE
        }
    }
}

/// An oracle type for the calls that use none
mod oracle_provider_stub {
    use zk_ee::oracle::usize_serialization::{UsizeDeserializable, UsizeSerializable};
    use zk_ee::oracle::IOOracle;
    use zk_ee::system::errors::internal::InternalError;

    pub enum NoOracle {}

    impl IOOracle for NoOracle {
        type RawIterator<'a> = core::iter::Empty<usize>;

        fn raw_query<'a, I: UsizeSerializable + UsizeDeserializable>(
            &'a mut self,
            _query_type: u32,
            _input: &I,
        ) -> Result<Self::RawIterator<'a>, InternalError> {
            match *self {}
        }
    }
}

fn point_evaluation_as_system_function_inner<
    D: ?Sized + TryExtend<u8>,
    R: Resources,
    O: IOOracle,
>(
    input: &[u8],
    dst: &mut D,
    resources: &mut R,
    mut oracle: Option<&mut O>,
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

    // Parse the commitment and proof
    let Ok(commitment_point) = parse_g1_compressed_with_oracle(commitment, oracle.as_deref_mut())
    else {
        return Err(interface_error!(
            PointEvaluationInterfaceError::InvalidPoint
        ));
    };
    let proof = &input[144..192];
    let Ok(proof) = parse_g1_compressed_with_oracle(proof, oracle.as_deref_mut()) else {
        return Err(interface_error!(
            PointEvaluationInterfaceError::InvalidPoint
        ));
    };

    let Ok(z) = parse_scalar(input[32..64].try_into().unwrap()) else {
        return Err(interface_error!(
            PointEvaluationInterfaceError::InvalidScalar
        ));
    };

    let Ok(y) = parse_scalar(input[64..96].try_into().unwrap()) else {
        return Err(interface_error!(
            PointEvaluationInterfaceError::InvalidScalar
        ));
    };

    if verify_kzg_proof_with_oracle(commitment_point, proof, z, y, oracle) {
        dst.try_extend(POINT_EVAL_PRECOMPILE_SUCCESS_RESPONSE)
            .map_err(|_| out_of_return_memory!())?;
        Ok(())
    } else {
        Err(interface_error!(
            PointEvaluationInterfaceError::PairingMismatch
        ))
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
        let mut claiming = curve_hints::tests::non_identity_claiming_oracle();
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

    /// Test scalar parsing edge cases
    #[test]
    fn test_parse_scalar_edge_cases() {
        // Test maximum valid scalar (modulus - 1)
        let max_valid = [
            0x73, 0xed, 0xa7, 0x53, 0x29, 0x9d, 0x7d, 0x48, 0x33, 0x39, 0xd8, 0x08, 0x09, 0xa1,
            0xd8, 0x05, 0x53, 0xbd, 0xa4, 0x02, 0xff, 0xfe, 0x5b, 0xfe, 0xff, 0xff, 0xff, 0xff,
            0x00, 0x00, 0x00, 0x00,
        ];
        assert!(parse_scalar(&max_valid).is_ok());

        // Test minimum invalid scalar (modulus)
        let min_invalid = [
            0x73, 0xed, 0xa7, 0x53, 0x29, 0x9d, 0x7d, 0x48, 0x33, 0x39, 0xd8, 0x08, 0x09, 0xa1,
            0xd8, 0x05, 0x53, 0xbd, 0xa4, 0x02, 0xff, 0xfe, 0x5b, 0xfe, 0xff, 0xff, 0xff, 0xff,
            0x00, 0x00, 0x00, 0x01,
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
            0xc0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
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
            0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        assert!(
            parse_g1_compressed(&invalid_flag).is_err(),
            "Invalid compression flag should fail"
        );

        // Test x-coordinate >= field modulus (invalid field element)
        let invalid_x = [
            0x9a, 0x0d, 0x51, 0xcc, 0x7f, 0xa0, 0x52, 0xe0, 0xc9, 0x9d, 0x3e, 0xa2, 0x42, 0x78,
            0x10, 0x5b, 0xf0, 0x1c, 0x29, 0x94, 0x3d, 0xa1, 0x8e, 0xf2, 0x50, 0x51, 0x73, 0x37,
            0x8a, 0x64, 0xa2, 0x61, 0x05, 0x43, 0x48, 0x44, 0x31, 0x15, 0x66, 0x5b, 0x5e, 0x96,
            0x4e, 0x9b, 0x4a, 0x3c, 0x7c, 0x59,
        ];
        assert!(
            parse_g1_compressed(&invalid_x).is_err(),
            "X-coordinate >= field modulus should fail"
        );

        // Test point not on curve (valid x-coordinate but no corresponding y)
        let not_on_curve = [
            0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x02,
        ];
        assert!(
            parse_g1_compressed(&not_on_curve).is_err(),
            "Point not on curve should fail"
        );

        // Test identity with wrong infinity flag
        let wrong_infinity = [
            0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
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
}
