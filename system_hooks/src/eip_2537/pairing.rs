use super::*;
use alloc::vec::Vec;
use crypto::{ark_ec::pairing::Pairing, bls12_381::curves::Bls12_381};

pub const BLS12_381_PAIRING_FIXED_GAS: u64 = 37700;
pub const BLS12_381_PAIRING_PER_PAIR_GAS: u64 = 32600;

pub const BLS12_381_PAIR_LEN: usize = G1_SERIALIZATION_LEN + G2_SERIALIZATION_LEN;

pub struct Bls12381PairingCheckPrecompile;

impl crate::PurePrecompileInvocation for Bls12381PairingCheckPrecompile {
    type Subsystem = Bls12PrecompileErrors;

    fn invoke<'a, R: Resources, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        _caller_ee: u8,
        output: &mut SliceVec<'a, u8>,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SubsystemError<Self::Subsystem>> {
        if input.is_empty() {
            return Err(Bls12PrecompileSubsystemError::LeafUsage(interface_error!(
                Bls12PrecompileInterfaceError::InvalidInputSize
            )));
        }
        let num_pairs = input.len() / BLS12_381_PAIR_LEN;
        let cost_ergs = Ergs(
            ((num_pairs as u64) * BLS12_381_PAIRING_PER_PAIR_GAS + BLS12_381_PAIRING_FIXED_GAS)
                * ERGS_PER_GAS,
        );
        let cost_native = 0;
        resources.charge(&R::from_ergs_and_native(
            cost_ergs,
            <R::Native as zk_ee::system::Computational>::from_computational(cost_native),
        ))?;

        if !input.len().is_multiple_of(BLS12_381_PAIR_LEN) {
            return Err(Bls12PrecompileSubsystemError::LeafUsage(interface_error!(
                Bls12PrecompileInterfaceError::InvalidInputSize
            )));
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
            g1_points.push(g1);
            g2_points.push(g2);
        }

        let pairing_result = <Bls12_381 as Pairing>::multi_pairing(g1_points, g2_points);
        output.extend_from_slice(&[0u8; 31]);

        use crypto::ark_ff::Field;
        if pairing_result.0 == <Bls12_381 as Pairing>::TargetField::ONE {
            output.extend([1u8]);
        } else {
            output.extend([0u8]);
        }

        Ok(())
    }
}
