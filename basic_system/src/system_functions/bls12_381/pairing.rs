use super::*;
use alloc::vec::Vec;
use crypto::ark_ec::AffineRepr;
use crypto::{ark_ec::pairing::Pairing, bls12_381::curves::Bls12_381};
use zk_ee::{
    out_of_return_memory,
    system::{Ergs, Resources, SystemFunction},
};

pub const BLS12_381_PAIRING_FIXED_GAS: u64 = 37700;
pub const BLS12_381_PAIRING_PER_PAIR_GAS: u64 = 32600;

pub const BLS12_381_PAIR_LEN: usize = G1_SERIALIZATION_LEN + G2_SERIALIZATION_LEN;

pub struct Bls12381PairingCheckPrecompile;

impl<R: Resources> SystemFunction<R, Bls12PrecompileErrors> for Bls12381PairingCheckPrecompile {
    fn execute<
        D: zk_ee::common_traits::TryExtend<u8> + ?Sized,
        A: core::alloc::Allocator + Clone,
    >(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), zk_ee::system::errors::subsystem::SubsystemError<Bls12PrecompileErrors>> {
        cycle_marker::wrap_with_resources!("bls12_381_pairing", resources, {
            bls12_381_pairing_as_system_function_inner(input, output, resources, allocator)
        })
    }
}

fn bls12_381_pairing_as_system_function_inner<
    D: zk_ee::common_traits::TryExtend<u8> + ?Sized,
    R: Resources,
    A: core::alloc::Allocator + Clone,
>(
    input: &[u8],
    output: &mut D,
    resources: &mut R,
    allocator: A,
) -> Result<(), zk_ee::system::errors::subsystem::SubsystemError<Bls12PrecompileErrors>> {
    if input.len() == 0 {
        return Err(interface_error!(
            Bls12PrecompileInterfaceError::InvalidInputSize
        ));
    }
    let num_pairs = input.len() / BLS12_381_PAIR_LEN;
    let cost_ergs = Ergs(
        ((num_pairs as u64) * BLS12_381_PAIRING_PER_PAIR_GAS + BLS12_381_PAIRING_FIXED_GAS)
            * ERGS_PER_GAS,
    );
    // TODO(EVM-1237): add native model
    let cost_native = 0;
    resources.charge(&R::from_ergs_and_native(
        cost_ergs,
        <R::Native as zk_ee::system::Computational>::from_computational(cost_native),
    ))?;

    if !input.len().is_multiple_of(BLS12_381_PAIR_LEN) {
        return Err(interface_error!(
            Bls12PrecompileInterfaceError::InvalidInputSize
        ));
    }

    let mut g1_points = Vec::with_capacity_in(num_pairs, allocator.clone());
    let mut g2_points = Vec::with_capacity_in(num_pairs, allocator.clone());

    // arkworks MSM allocates inside, so we will do it our way, just parse here
    // G1Projective::msm_bigint(bases, bigints)

    // parse to use Peppinger algorithm
    for pair_encoding in input.as_chunks::<BLS12_381_PAIR_LEN>().0.iter() {
        let g1 = parse_g1_with_subgroup_check(
            pair_encoding[0..G1_SERIALIZATION_LEN].try_into().unwrap(),
        )?;
        let g2 = parse_g2_with_subgroup_check(
            pair_encoding[G1_SERIALIZATION_LEN..(G1_SERIALIZATION_LEN + G2_SERIALIZATION_LEN)]
                .try_into()
                .unwrap(),
        )?;
        // e(O, Q) = e(P, O) = 1 in the target field, so degenerate pairs do not
        // affect the multi-pairing product. Skip them after subgroup validation
        // to save the per-pair Miller-loop precomputation that dominates the
        // cost on Pectra degenerate inputs.
        if g1.is_zero() || g2.is_zero() {
            continue;
        }
        g1_points.push(g1);
        g2_points.push(g2);
    }

    output
        .try_extend([0u8; 31])
        .map_err(|_| out_of_return_memory!())?;

    use crypto::ark_ff::Field;
    let success = if g1_points.is_empty() {
        // Empty product equals the identity in the target field.
        true
    } else {
        let pairing_result = <Bls12_381 as Pairing>::multi_pairing(g1_points, g2_points);
        pairing_result.0 == <Bls12_381 as Pairing>::TargetField::ONE
    };

    output
        .try_extend([success as u8])
        .map_err(|_| out_of_return_memory!())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::ops::Neg;
    use crypto::bls12_381::eip2537::{serialize_g1_bytes, serialize_g2_bytes};
    use zk_ee::reference_implementations::{BaseResources, DecreasingNative};
    use zk_ee::system::Resource;

    fn encode_g1(point: G1Affine) -> [u8; G1_SERIALIZATION_LEN] {
        let mut buf = [0u8; G1_SERIALIZATION_LEN];
        serialize_g1_bytes(point, &mut buf);
        buf
    }

    fn encode_g2(point: G2Affine) -> [u8; G2_SERIALIZATION_LEN] {
        let mut buf = [0u8; G2_SERIALIZATION_LEN];
        serialize_g2_bytes(point, &mut buf);
        buf
    }

    fn encode_pair(g1: G1Affine, g2: G2Affine) -> [u8; BLS12_381_PAIR_LEN] {
        let mut buf = [0u8; BLS12_381_PAIR_LEN];
        buf[..G1_SERIALIZATION_LEN].copy_from_slice(&encode_g1(g1));
        buf[G1_SERIALIZATION_LEN..].copy_from_slice(&encode_g2(g2));
        buf
    }

    fn run(input: &[u8]) -> Vec<u8> {
        let allocator = std::alloc::Global;
        let mut resource = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;
        let mut dst: Vec<u8> = Vec::new();
        Bls12381PairingCheckPrecompile::execute(input, &mut dst, &mut resource, allocator)
            .expect("precompile should succeed on well-formed input");
        dst
    }

    fn expect_check(input: &[u8], expected_true: bool) {
        let dst = run(input);
        let mut expected = [0u8; 32];
        expected[31] = expected_true as u8;
        assert_eq!(dst.as_slice(), &expected[..]);
    }

    #[test]
    fn single_pair_both_infinity_returns_true() {
        let input = [0u8; BLS12_381_PAIR_LEN];
        expect_check(&input, true);
    }

    #[test]
    fn single_pair_g1_infinity_returns_true() {
        let mut input = [0u8; BLS12_381_PAIR_LEN];
        input[G1_SERIALIZATION_LEN..].copy_from_slice(&encode_g2(G2Affine::generator()));
        expect_check(&input, true);
    }

    #[test]
    fn single_pair_g2_infinity_returns_true() {
        let mut input = [0u8; BLS12_381_PAIR_LEN];
        input[..G1_SERIALIZATION_LEN].copy_from_slice(&encode_g1(G1Affine::generator()));
        expect_check(&input, true);
    }

    #[test]
    fn many_infinity_pairs_return_true() {
        let input = vec![0u8; 7 * BLS12_381_PAIR_LEN];
        expect_check(&input, true);
    }

    #[test]
    fn nontrivial_pair_returns_false_and_infinity_does_not_mask_it() {
        // e(G1, G2) is the BLS12-381 generator pairing, which is not 1.
        let nontrivial = encode_pair(G1Affine::generator(), G2Affine::generator());
        expect_check(&nontrivial, false);

        // Appending degenerate pairs must not flip the result to true.
        let mut with_inf = nontrivial.to_vec();
        with_inf.extend_from_slice(&[0u8; BLS12_381_PAIR_LEN]);
        expect_check(&with_inf, false);

        let mut prefixed = vec![0u8; BLS12_381_PAIR_LEN];
        prefixed.extend_from_slice(&nontrivial);
        expect_check(&prefixed, false);
    }

    #[test]
    fn balanced_pair_returns_true_with_or_without_infinity_padding() {
        // e(G1, G2) * e(-G1, G2) = e(G1, G2) * e(G1, G2)^{-1} = 1
        let g1 = G1Affine::generator();
        let g2 = G2Affine::generator();
        let balanced_a = encode_pair(g1, g2);
        let balanced_b = encode_pair(g1.neg(), g2);

        let mut balanced = balanced_a.to_vec();
        balanced.extend_from_slice(&balanced_b);
        expect_check(&balanced, true);

        // Interleaving degenerate pairs must keep the result true.
        let mut interleaved = vec![0u8; BLS12_381_PAIR_LEN];
        interleaved.extend_from_slice(&balanced_a);
        interleaved.extend_from_slice(&[0u8; BLS12_381_PAIR_LEN]);
        interleaved.extend_from_slice(&balanced_b);
        interleaved.extend_from_slice(&[0u8; BLS12_381_PAIR_LEN]);
        expect_check(&interleaved, true);
    }

    #[test]
    fn malformed_nonzero_encoding_is_still_rejected() {
        // A G1 input where the y-coordinate is forced to zero with a non-zero x
        // is not on the curve and must not be accepted as the point at infinity.
        // This guards against any future refactor that filters before parsing.
        let mut input = [0u8; BLS12_381_PAIR_LEN];
        // x = 1 in big-endian, padded to 48 bytes then to the 64-byte slot.
        input[G1_SERIALIZATION_LEN - 1] = 1;
        // y stays zero. G2 stays at infinity (irrelevant once G1 parse fails).
        let allocator = std::alloc::Global;
        let mut resource = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;
        let mut dst: Vec<u8> = Vec::new();
        let err =
            Bls12381PairingCheckPrecompile::execute(&input, &mut dst, &mut resource, allocator)
                .expect_err("invalid G1 encoding must be rejected");
        // Sanity: we got an error rather than silently treating it as infinity.
        let _ = err;
    }
}
