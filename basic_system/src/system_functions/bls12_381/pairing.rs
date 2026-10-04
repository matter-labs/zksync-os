use super::*;
use crate::system_functions::curve_hints::{self, Bls12381Pair, PairingClaim};
use alloc::vec::Vec;
use crypto::{ark_ec::pairing::Pairing, ark_ff::One, bls12_381::curves::Bls12_381};
use zk_ee::oracle::IOOracle;
use zk_ee::system::base_system_functions::SystemFunctionExt;
use zk_ee::system::logger::Logger;
use zk_ee::{out_of_return_memory, system::Resources};

pub const BLS12_381_PAIRING_FIXED_GAS: u64 = 37700;
pub const BLS12_381_PAIRING_PER_PAIR_GAS: u64 = 32600;

pub const BLS12_381_PAIR_LEN: usize = G1_SERIALIZATION_LEN + G2_SERIALIZATION_LEN;

pub struct Bls12381PairingCheckPrecompile<const USE_ADVICE: bool>;

impl<R: Resources, const USE_ADVICE: bool> SystemFunctionExt<R, Bls12PrecompileErrors>
    for Bls12381PairingCheckPrecompile<USE_ADVICE>
{
    fn execute<
        O: IOOracle,
        L: Logger,
        D: zk_ee::common_traits::TryExtend<u8> + ?Sized,
        A: core::alloc::Allocator + Clone,
    >(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        oracle: &mut O,
        _logger: &mut L,
        allocator: A,
    ) -> Result<(), zk_ee::system::errors::subsystem::SubsystemError<Bls12PrecompileErrors>> {
        cycle_marker::wrap_with_resources!("bls12_381_pairing", resources, {
            bls12_381_pairing_as_system_function_inner(
                input,
                output,
                resources,
                allocator,
                if USE_ADVICE { Some(oracle) } else { None },
            )
        })
    }
}

fn bls12_381_pairing_as_system_function_inner<
    D: zk_ee::common_traits::TryExtend<u8> + ?Sized,
    R: Resources,
    A: core::alloc::Allocator + Clone,
    O: IOOracle,
>(
    input: &[u8],
    output: &mut D,
    resources: &mut R,
    allocator: A,
    mut oracle: Option<&mut O>,
) -> Result<(), zk_ee::system::errors::subsystem::SubsystemError<Bls12PrecompileErrors>> {
    if input.is_empty() || !input.len().is_multiple_of(BLS12_381_PAIR_LEN) {
        return Err(interface_error!(
            Bls12PrecompileInterfaceError::InvalidInputSize
        ));
    }

    let num_pairs = input.len() / BLS12_381_PAIR_LEN;
    let cost_gas =
        (num_pairs as u64) * BLS12_381_PAIRING_PER_PAIR_GAS + BLS12_381_PAIRING_FIXED_GAS;
    let cost_native = crate::cost_constants::BLS12_381_PAIRING_NATIVE_COST
        + crate::cost_constants::BLS12_381_PAIRING_PER_PAIR_NATIVE_COST
            .saturating_mul(num_pairs as u64);
    resources.charge_legacy_gas_and_native(cost_gas, cost_native)?;

    let mut pairs = Vec::with_capacity_in(num_pairs, allocator.clone());

    for pair_encoding in input.as_chunks::<BLS12_381_PAIR_LEN>().0.iter() {
        let g1 = parse_g1_with_subgroup_check_with_oracle(
            pair_encoding[0..G1_SERIALIZATION_LEN].try_into().unwrap(),
            oracle.as_deref_mut(),
        )?;
        let g2 = parse_g2_with_subgroup_check_with_oracle(
            pair_encoding[G1_SERIALIZATION_LEN..(G1_SERIALIZATION_LEN + G2_SERIALIZATION_LEN)]
                .try_into()
                .unwrap(),
            oracle.as_deref_mut(),
        )?;
        pairs.push(Bls12381Pair { g1, g2 });
    }

    let is_identity = pairing_product_is_identity(&pairs, oracle);
    output
        .try_extend([0u8; 31])
        .map_err(|_| out_of_return_memory!())?;

    if is_identity {
        output
            .try_extend([1u8])
            .map_err(|_| out_of_return_memory!())?;
    } else {
        output
            .try_extend([0u8])
            .map_err(|_| out_of_return_memory!())?;
    }

    Ok(())
}

/// The hint can save the final exponentiation, but cannot choose the result.
fn pairing_product_is_identity<O: IOOracle>(
    pairs: &[Bls12381Pair],
    oracle: Option<&mut O>,
) -> bool {
    let Some(oracle) = oracle else {
        return Bls12_381::multi_pairing(
            pairs.iter().map(|pair| pair.g1),
            pairs.iter().map(|pair| pair.g2),
        )
        .0
        .is_one();
    };
    // Prepare only the current bounded chunk, rather than retaining all of its large line tables.
    let g1 = || pairs.iter().map(|pair| pair.g1);
    let g2 = || pairs.iter().map(|pair| pair.g2);
    match curve_hints::bls12_381_pairing_residue_witness(oracle, pairs) {
        PairingClaim::Identity { d, s, .. } => {
            let l = Bls12_381::multi_miller_loop_with_initial_owned(&d, g1(), g2());
            assert!(
                crypto::residue_witness::bls12_381::check(&l, &d, &s),
                "the residue witness of the pairing claimed to be the identity is wrong"
            );
            true
        }
        PairingClaim::NotIdentity { f_inverse } => {
            let f = Bls12_381::multi_miller_loop(g1(), g2()).0;
            Bls12_381::final_exponentiation_with_inverse(&f, |f| {
                curve_hints::checked_inverse(f, f_inverse)
            })
            .expect("the Miller loop output is invertible")
            .is_one()
        }
    }
}
