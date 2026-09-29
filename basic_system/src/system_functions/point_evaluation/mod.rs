use crate::cost_constants::{POINT_EVALUATION_COST_GAS, POINT_EVALUATION_NATIVE_COST};
use crate::system_functions::curve_hints;
use crypto::ark_ec::pairing::Pairing;
use crypto::ark_ec::{AffineRepr, CurveGroup};
use crypto::ark_ff::{Field, PrimeField};
use zk_ee::common_structs::precompile_caches::{
    PointEvaluationCache, PrecompileCaches, POINT_EVALUATION_INPUT_LEN,
};
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
            let oracle = if USE_ADVICE { Some(oracle) } else { None };
            point_evaluation_as_system_function_inner(input, output, resources, oracle, None)
        })
    }

    /// Same as `execute`, but an input that verified before in this `System` is not verified again
    fn execute_with_precompile_caches<
        O: IOOracle,
        L: Logger,
        D: TryExtend<u8> + ?Sized,
        A: core::alloc::Allocator + Clone,
    >(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        oracle: &mut O,
        precompile_caches: &mut PrecompileCaches,
        _logger: &mut L,
        _allocator: A,
    ) -> Result<(), SubsystemError<PointEvaluationErrors>> {
        cycle_marker::wrap_with_resources!("point_evaluation", resources, {
            let oracle = if USE_ADVICE { Some(oracle) } else { None };
            point_evaluation_as_system_function_inner(
                input,
                output,
                resources,
                oracle,
                Some(&mut precompile_caches.point_evaluation),
            )
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
/// hint, and the subgroup membership test in affine coordinates with hinted divisions, when an
/// oracle is given
fn parse_g1_compressed_with_oracle<O: IOOracle>(
    input: &[u8],
    oracle: Option<&mut O>,
) -> Result<crypto::bls12_381::G1Affine, ()> {
    match oracle {
        Some(oracle) => {
            // the two steps query the oracle one after the other
            let oracle = core::cell::RefCell::new(oracle);
            crypto::bls12_381::g1_from_compressed_with_hints(
                input,
                |y_squared| curve_hints::bls12_381_fq_sqrt(*oracle.borrow_mut(), y_squared),
                |point| {
                    crypto::bls12_381::g1::is_in_subgroup_with_divider(
                        point,
                        &mut curve_hints::Bls12381FqDivider::new(*oracle.borrow_mut()),
                    )
                },
            )
            .map_err(|_| ())
        }
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

/// With an oracle, the curve arithmetic works in affine coordinates with its field divisions
/// from checked hints, and the inversion of the final exponentiation comes from one too.
pub fn verify_kzg_proof_with_oracle<O: IOOracle>(
    commitment: crypto::bls12_381::G1Affine,
    proof: crypto::bls12_381::G1Affine,
    z: KzgScalar,
    y: KzgScalar,
    mut oracle: Option<&mut O>,
) -> bool {
    use crypto::bls12_381::curves::{g1, Bls12_381};
    // Original check:
    // e(yG1 - commitment, G2) * e(proof, tauG2 - zG2) == 1.
    //
    // Move the z multiplication from G2 to G1:
    // e(yG1 - commitment - z*proof, G2) * e(proof, tauG2) == 1.
    // y G - z proof as one joint double-and-add (the doublings are shared), then minus the
    // commitment; the scalars were parsed below the group order
    let scalar = |repr: &KzgScalar| {
        crypto::bls12_381::Fr::from_bigint(*repr).expect("the scalar is below the group order")
    };
    let generator = crypto::bls12_381::G1Affine::generator();
    let left_g1 = match oracle.as_deref_mut() {
        Some(oracle) => {
            let mut divider = curve_hints::Bls12381FqDivider::new(oracle);
            let sum = g1::mul_two_affine_with_divider(
                &generator,
                scalar(&y),
                &proof,
                -scalar(&z),
                &mut divider,
            );
            g1::add_affine_with_divider(&sum, &-commitment, &mut divider)
        }
        None => {
            let mut left_g1 = g1::mul_two(
                &generator.into_group(),
                scalar(&y),
                &proof.into_group(),
                -scalar(&z),
            );
            left_g1 -= &commitment;
            left_g1.into_affine()
        }
    };
    // both G2 points are fixed, so their Miller-loop line coefficients are constants
    // (normalized to a first coefficient of one), read in place
    let g2 = [
        &crypto::bls12_381::consts::PREPARED_G2_GENERATOR,
        &crypto::bls12_381::consts::PREPARED_G2_BY_TAU,
    ];
    // the G1 points of the product, which the oracle reads where they are
    let g1 = [left_g1, proof];
    // the Miller loop over both pairs at once, as the prover computes it for its witness
    let pairs = || g1.iter().zip(g2);
    // the exact final exponentiation of the loop output, conjugated for the negative seed as
    // `multi_miller_loop` conjugates it (the prover's inverse is of the conjugate)
    let exact = |inverse: &dyn Fn(&crypto::bls12_381::Fq12) -> Option<crypto::bls12_381::Fq12>| {
        let mut miller_loop = Bls12_381::multi_miller_loop_shared(None, pairs());
        miller_loop.conjugate_in_place();
        // the Miller loop of curve points never evaluates to zero
        let gt_el = Bls12_381::final_exponentiation_with_inverse(&miller_loop, inverse)
            .expect("the Miller loop output is invertible");
        gt_el == <Bls12_381 as Pairing>::TargetField::ONE
    };
    let Some(oracle) = oracle else {
        return exact(&Field::inverse);
    };
    // The residue witness check in place of the final exponentiation (Novakovic, Eagen, "On
    // Proving Pairings", https://eprint.iacr.org/2024/640; soundness in the documentation of
    // `crypto::residue_witness`): the Miller loop started at `d` gives `d^|u| f`, the rest is
    // one Frobenius map. The prover claims the outcome; it cannot choose it: a claimed
    // identity must come with a witness that passes the check (a failed check is a broken
    // prover and panics), and a claimed non-identity is settled by the exact final
    // exponentiation, which finds an identity all the same.
    match curve_hints::bls12_381_kzg_residue_witness(oracle, &g1) {
        curve_hints::PairingClaim::Identity { c: _, d, s } => {
            let l = Bls12_381::multi_miller_loop_shared(Some(&d), pairs());
            assert!(
                crypto::residue_witness::bls12_381::check(&l, &d, &s),
                "the residue witness of the KZG proof claimed to be valid is wrong"
            );
            true
        }
        curve_hints::PairingClaim::NotIdentity { f_inverse } => {
            exact(&|f| curve_hints::checked_inverse(f, f_inverse))
        }
    }
}

/// An oracle type for the calls that use none
mod oracle_provider_stub {
    use zk_ee::oracle::usize_serialization::{UsizeDeserializable, UsizeSerializable};
    use zk_ee::oracle::IOOracle;
    use zk_ee::system::errors::internal::InternalError;

    pub enum NoOracle {}

    impl zk_ee::oracle::memory_io::MemoryOracle for NoOracle {}

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
    verified_inputs: Option<&mut PointEvaluationCache>,
) -> Result<(), SubsystemError<PointEvaluationErrors>> {
    resources
        .charge_legacy_gas_and_native(POINT_EVALUATION_COST_GAS, POINT_EVALUATION_NATIVE_COST)?;

    let Ok(input) = <&[u8; POINT_EVALUATION_INPUT_LEN]>::try_from(input) else {
        return Err(interface_error!(
            PointEvaluationInterfaceError::InvalidInputSize
        ));
    };

    // The outcome is a function of the input (and of the fixed trusted setup) only, and only
    // inputs that passed all the checks below are cached, so a cached input passes them again
    if !verified_inputs
        .as_deref()
        .is_some_and(|cache| cache.get(input).is_some())
    {
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
        let Ok(commitment_point) =
            parse_g1_compressed_with_oracle(commitment, oracle.as_deref_mut())
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

        if !verify_kzg_proof_with_oracle(commitment_point, proof, z, y, oracle) {
            return Err(interface_error!(
                PointEvaluationInterfaceError::PairingMismatch
            ));
        }
        if let Some(cache) = verified_inputs {
            cache.insert(input, ());
        }
    }

    dst.try_extend(POINT_EVAL_PRECOMPILE_SUCCESS_RESPONSE)
        .map_err(|_| out_of_return_memory!())?;
    Ok(())
}

#[cfg(test)]
mod tests;
