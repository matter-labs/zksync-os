use super::*;
use crate::cost_constants::{
    BN254_PAIRING_BASE_NATIVE_COST, BN254_PAIRING_COST_PER_PAIR_GAS,
    BN254_PAIRING_PER_PAIR_NATIVE_COST, BN254_PAIRING_STATIC_COST_GAS,
};
use crate::system_functions::bytereverse;
use crate::system_functions::curve_hints;
use alloc::vec::Vec;
use crypto::ark_ff::Zero;
use crypto::ark_serialize::{CanonicalDeserialize, Valid};
use zk_ee::common_traits::TryExtend;
use zk_ee::oracle::IOOracle;
use zk_ee::system::base_system_functions::{
    Bn254PairingCheckErrors, Bn254PairingCheckInterfaceError, SystemFunctionExt,
};
use zk_ee::system::errors::subsystem::SubsystemError;
use zk_ee::system::logger::Logger;
use zk_ee::{interface_error, out_of_return_memory};

///
/// bn254 pairing check system function implementation.
/// With `USE_ADVICE`, the field inversion of the final exponentiation is taken from an oracle
/// hint.
///
pub struct Bn254PairingCheckImpl<const USE_ADVICE: bool>;

impl<R: Resources, const USE_ADVICE: bool> SystemFunctionExt<R, Bn254PairingCheckErrors>
    for Bn254PairingCheckImpl<USE_ADVICE>
{
    /// Returns `OutOfGas` if not enough resources provided.
    /// Returns `InvalidInput` error if the input size is not divisible by 192
    /// or failed to create affine points from inputs
    fn execute<
        O: IOOracle,
        L: Logger,
        D: TryExtend<u8> + ?Sized,
        A: core::alloc::Allocator + Clone,
    >(
        src: &[u8],
        dst: &mut D,
        resources: &mut R,
        oracle: &mut O,
        _logger: &mut L,
        allocator: A,
    ) -> Result<(), SubsystemError<Bn254PairingCheckErrors>> {
        cycle_marker::wrap_with_resources!("bn254_pairing", resources, {
            let oracle = if USE_ADVICE { Some(oracle) } else { None };
            let num_pairs = src.len() / 192;
            let gas_cost = BN254_PAIRING_STATIC_COST_GAS
                + BN254_PAIRING_COST_PER_PAIR_GAS * (num_pairs as u64);
            // Pairing has a large fixed cost (final exponentiation) charged once
            // when there is any pairing work, plus a per-pair Miller-loop cost.
            let native_cost = if num_pairs == 0 {
                0
            } else {
                BN254_PAIRING_BASE_NATIVE_COST
                    + (num_pairs as u64) * BN254_PAIRING_PER_PAIR_NATIVE_COST
            };

            resources.charge_legacy_gas_and_native(gas_cost, native_cost)?;

            if !src.len().is_multiple_of(192) {
                return Err(interface_error!(
                    Bn254PairingCheckInterfaceError::InvalidPairingSize
                ));
            }

            let success = if src.is_empty() {
                true
            } else {
                bn254_pairing_check_inner::<A, O>(num_pairs, src, allocator, oracle)
                    .map_err(|_| interface_error!(Bn254PairingCheckInterfaceError::InvalidPoint))?
            };

            dst.try_extend(core::iter::repeat_n(0, 31).chain(core::iter::once(success as u8)))
                .map_err(|_| out_of_return_memory!())?;

            Ok(())
        })
    }
}

/// With an oracle, the field inversion of the final exponentiation comes from a checked hint.
fn bn254_pairing_check_inner<A: Allocator + Clone, O: IOOracle>(
    num_pairs: usize,
    src: &[u8],
    allocator: A,
    oracle: Option<&mut O>,
) -> Result<bool, ()> {
    use crypto::ark_ec::pairing::Pairing;
    use crypto::ark_ff::{One, PrimeField};
    use crypto::bn254::curves::{Bn254, G1Affine, G2Affine};
    use crypto::bn254::fields::{Fq, Fq2};

    if num_pairs == 0 {
        return Ok(true);
    }

    let mut pairs = Vec::with_capacity_in(num_pairs, allocator.clone());
    let mut src_iter = src.iter();

    for _ in 0..num_pairs {
        let mut buffer = [0u8; 192];
        for (dst, src) in buffer.iter_mut().zip(&mut src_iter) {
            *dst = *src;
        }
        let mut it = buffer.as_chunks::<32>().0.iter();
        unsafe {
            let mut g1_x = *it.next().unwrap_unchecked();
            let mut g1_y = *it.next().unwrap_unchecked();

            // NOTE: Ethereum serialization is strange
            let mut g2_x_c1 = *it.next().unwrap_unchecked();
            let mut g2_x_c0 = *it.next().unwrap_unchecked();
            let mut g2_y_c1 = *it.next().unwrap_unchecked();
            let mut g2_y_c0 = *it.next().unwrap_unchecked();

            bytereverse(&mut g1_x);
            bytereverse(&mut g1_y);

            let g1_x =
                <Fq as PrimeField>::BigInt::deserialize_uncompressed(&g1_x[..]).map_err(|_| ())?;
            let g1_y =
                <Fq as PrimeField>::BigInt::deserialize_uncompressed(&g1_y[..]).map_err(|_| ())?;
            let g1_x = Fq::from_bigint(g1_x).ok_or(())?;
            let g1_y = Fq::from_bigint(g1_y).ok_or(())?;

            // the point at infinity is encoded as (0, 0); every other encoding must be a
            // point of the curve (and, for G2, of the subgroup)
            let g1_is_zero = g1_x.is_zero() && g1_y.is_zero();
            let g1_point = G1Affine::new_unchecked(g1_x, g1_y);
            if !g1_is_zero {
                g1_point.check().map_err(|_| ())?;
            }

            bytereverse(&mut g2_x_c0);
            bytereverse(&mut g2_x_c1);
            bytereverse(&mut g2_y_c0);
            bytereverse(&mut g2_y_c1);

            let g2_x_c0 = <Fq as PrimeField>::BigInt::deserialize_uncompressed(&g2_x_c0[..])
                .map_err(|_| ())?;
            let g2_x_c1 = <Fq as PrimeField>::BigInt::deserialize_uncompressed(&g2_x_c1[..])
                .map_err(|_| ())?;
            let g2_x_c0 = Fq::from_bigint(g2_x_c0).ok_or(())?;
            let g2_x_c1 = Fq::from_bigint(g2_x_c1).ok_or(())?;

            let g2_x = Fq2::new(g2_x_c0, g2_x_c1);

            let g2_y_c0 = <Fq as PrimeField>::BigInt::deserialize_uncompressed(&g2_y_c0[..])
                .map_err(|_| ())?;
            let g2_y_c1 = <Fq as PrimeField>::BigInt::deserialize_uncompressed(&g2_y_c1[..])
                .map_err(|_| ())?;
            let g2_y_c0 = Fq::from_bigint(g2_y_c0).ok_or(())?;
            let g2_y_c1 = Fq::from_bigint(g2_y_c1).ok_or(())?;

            let g2_y = Fq2::new(g2_y_c0, g2_y_c1);

            let g2_is_zero = g2_x.is_zero() && g2_y.is_zero();
            let g2_point = G2Affine::new_unchecked(g2_x, g2_y);
            if !g2_is_zero {
                g2_point.check().map_err(|_| ())?;
            }

            // e(O, Q) = e(P, O) = 1: a degenerate pair does not change the product, and
            // skipping it (after the validation above) saves its Miller loop preparation
            if g1_is_zero || g2_is_zero {
                continue;
            }

            pairs.push((g1_point, g2_point));
        }
    }

    if pairs.is_empty() {
        // the empty product is the identity of the target group
        return Ok(true);
    }

    let g1_iter = || pairs.iter().map(|(g1, _)| g1);
    let g2_iter = || pairs.iter().map(|(_, g2)| g2);
    let Some(oracle) = oracle else {
        let miller_loop = Bn254::multi_miller_loop(g1_iter(), g2_iter());
        // the Miller loop of curve points never evaluates to zero
        let result =
            Bn254::final_exponentiation(miller_loop).expect("the Miller loop output is invertible");
        return Ok(result.0.is_one());
    };
    // The residue witness check in place of the final exponentiation (Novakovic, Eagen, "On
    // Proving Pairings", https://eprint.iacr.org/2024/640; soundness in the documentation of
    // `crypto::residue_witness`): the Miller loop started at `d` gives `d^(6x+2) f`, the rest
    // is a few Frobenius maps. The prover claims the outcome; it cannot choose it: a claimed
    // identity must come with a witness that passes the check (a failed check is a broken
    // prover and panics), and a claimed non-identity is settled by the exact final
    // exponentiation, which finds an identity all the same.
    match curve_hints::bn254_pairing_residue_witness(oracle, &pairs, allocator) {
        curve_hints::PairingClaim::Identity { c, d, s } => {
            let l = Bn254::multi_miller_loop_with_initial(&d, &c, g1_iter(), g2_iter());
            assert!(
                crypto::residue_witness::bn254::check(&l, &d, &s),
                "the residue witness of the pairing claimed to be the identity is wrong"
            );
            Ok(true)
        }
        curve_hints::PairingClaim::NotIdentity { f_inverse } => {
            let miller_loop = Bn254::multi_miller_loop(g1_iter(), g2_iter());
            let result = Bn254::final_exponentiation_with_inverse(&miller_loop.0, |f| {
                curve_hints::checked_inverse(f, f_inverse)
            })
            .expect("the Miller loop output is invertible");
            Ok(result.is_one())
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::system_functions::field_ops::FieldHintOp;
    use callable_oracles::field_hints::NativeFieldOpsQuery;
    use oracle_provider::ZkEENonDeterminismSource;
    use zk_ee::reference_implementations::BaseResources;
    use zk_ee::reference_implementations::DecreasingNative;
    use zk_ee::system::logger::NullLogger;
    use zk_ee::system::Resource;

    #[test]
    fn test_pairing_inner() {
        let allocator = std::alloc::Global;

        let src = hex::decode(
            "\
            1c76476f4def4bb94541d57ebba1193381ffa7aa76ada664dd31c16024c43f59\
            3034dd2920f673e204fee2811c678745fc819b55d3e9d294e45c9b03a76aef41\
            209dd15ebff5d46c4bd888e51a93cf99a7329636c63514396b4a452003a35bf7\
            04bf11ca01483bfa8b34b43561848d28905960114c8ac04049af4b6315a41678\
            2bb8324af6cfc93537a2ad1a445cfd0ca2a71acd7ac41fadbf933c2a51be344d\
            120a2a4cf30c1bf9845f20c6fe39e07ea2cce61f0c9bb048165fe5e4de877550\
            111e129f1cf1097710d41c4ac70fcdfa5ba2023c6ff1cbeac322de49d1b6df7c\
            2032c61a830e3c17286de9462bf242fca2883585b93870a73853face6a6bf411\
            198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2\
            1800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed\
            090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b\
            12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa",
        )
        .unwrap();

        assert!(bn254_pairing_check_inner::<_, ZkEENonDeterminismSource>(
            2,
            src.as_slice(),
            allocator,
            None
        )
        .unwrap());
        let mut oracle = ZkEENonDeterminismSource::default();
        oracle.add_external_processor(NativeFieldOpsQuery);
        assert!(
            bn254_pairing_check_inner(2, src.as_slice(), allocator, Some(&mut oracle)).unwrap()
        );
    }

    #[test]
    #[should_panic(expected = "claimed to be the identity is wrong")]
    fn test_pairing_wrong_identity_witness_panics() {
        let src = hex::decode(
            "\
            1c76476f4def4bb94541d57ebba1193381ffa7aa76ada664dd31c16024c43f59\
            3034dd2920f673e204fee2811c678745fc819b55d3e9d294e45c9b03a76aef41\
            209dd15ebff5d46c4bd888e51a93cf99a7329636c63514396b4a452003a35bf7\
            04bf11ca01483bfa8b34b43561848d28905960114c8ac04049af4b6315a41678\
            2bb8324af6cfc93537a2ad1a445cfd0ca2a71acd7ac41fadbf933c2a51be344d\
            120a2a4cf30c1bf9845f20c6fe39e07ea2cce61f0c9bb048165fe5e4de877550\
            111e129f1cf1097710d41c4ac70fcdfa5ba2023c6ff1cbeac322de49d1b6df7c\
            2032c61a830e3c17286de9462bf242fca2883585b93870a73853face6a6bf411\
            198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2\
            1800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed\
            090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b\
            12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa",
        )
        .unwrap();
        // a corrupted `c` with the identity claim fails the inverse pair check
        let mut lying =
            curve_hints::tests::lying_oracle(&[FieldHintOp::Bn254PairingResidueWitness]);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            bn254_pairing_check_inner(2, src.as_slice(), std::alloc::Global, Some(&mut lying))
        }));
        let message = result.unwrap_err();
        let message = message
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| message.downcast_ref::<String>().map(String::as_str))
            .unwrap_or("");
        assert!(message.contains("not an inverse pair"), "{message}");
        // a corrupted scaling factor with the identity claim fails the residue check
        let mut lying =
            curve_hints::tests::lying_oracle_last_word(&[FieldHintOp::Bn254PairingResidueWitness]);
        let _ = bn254_pairing_check_inner(2, src.as_slice(), std::alloc::Global, Some(&mut lying));
    }

    #[test]
    fn test_pairing_external() {
        let allocator = std::alloc::Global;

        let mut resource = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;

        let src: &[u8] = &hex::decode(
            "\
            1c76476f4def4bb94541d57ebba1193381ffa7aa76ada664dd31c16024c43f59\
            3034dd2920f673e204fee2811c678745fc819b55d3e9d294e45c9b03a76aef41\
            209dd15ebff5d46c4bd888e51a93cf99a7329636c63514396b4a452003a35bf7\
            04bf11ca01483bfa8b34b43561848d28905960114c8ac04049af4b6315a41678\
            2bb8324af6cfc93537a2ad1a445cfd0ca2a71acd7ac41fadbf933c2a51be344d\
            120a2a4cf30c1bf9845f20c6fe39e07ea2cce61f0c9bb048165fe5e4de877550\
            111e129f1cf1097710d41c4ac70fcdfa5ba2023c6ff1cbeac322de49d1b6df7c\
            2032c61a830e3c17286de9462bf242fca2883585b93870a73853face6a6bf411\
            198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2\
            1800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed\
            090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b\
            12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa",
        )
        .unwrap();

        let expected =
            hex::decode("0000000000000000000000000000000000000000000000000000000000000001")
                .unwrap();
        let mut dst = vec![];

        let mut oracle = ZkEENonDeterminismSource::default();
        oracle.add_external_processor(NativeFieldOpsQuery);
        Bn254PairingCheckImpl::<false>::execute(
            src,
            &mut dst,
            &mut resource,
            &mut oracle,
            &mut NullLogger,
            allocator,
        )
        .unwrap();

        assert_eq!(expected, dst.as_slice());

        let mut dst = vec![];

        Bn254PairingCheckImpl::<true>::execute(
            src,
            &mut dst,
            &mut resource,
            &mut oracle,
            &mut NullLogger,
            allocator,
        )
        .unwrap();

        assert_eq!(expected, dst.as_slice());
    }
}
