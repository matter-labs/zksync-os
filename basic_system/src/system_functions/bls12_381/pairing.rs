use super::*;
use crate::system_functions::field_ops::ZkOsHintOracle;
use crypto::hints::HintOracle;
use crypto::precompiles::bls12_381_pairing_check;
use zk_ee::oracle::IOOracle;
use zk_ee::system::base_system_functions::SystemFunctionExt;
use zk_ee::system::logger::Logger;
use zk_ee::{out_of_return_memory, system::Resources};

pub const BLS12_381_PAIRING_FIXED_GAS: u64 = 37700;
pub const BLS12_381_PAIRING_PER_PAIR_GAS: u64 = 32600;

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
                if USE_ADVICE {
                    Some(ZkOsHintOracle::from_mut(oracle))
                } else {
                    None
                },
            )
        })
    }
}

fn bls12_381_pairing_as_system_function_inner<
    D: zk_ee::common_traits::TryExtend<u8> + ?Sized,
    R: Resources,
    A: core::alloc::Allocator + Clone,
    O: HintOracle,
>(
    input: &[u8],
    output: &mut D,
    resources: &mut R,
    allocator: A,
    oracle: Option<&mut O>,
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

    let is_identity =
        bls12_381_pairing_check(input.as_chunks::<BLS12_381_PAIR_LEN>().0, allocator, oracle)
            .map_err(interface_error_of)?;
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
