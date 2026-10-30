use crypto::hints::MemoryOracle;
use crypto::precompiles::{bls12_381_map_fp2_to_g2, bls12_381_map_fp_to_g1};
use zk_ee::oracle::IOOracle;
use zk_ee::system::logger::Logger;
use zk_ee::system::{Resources, SystemFunctionExt};

use super::*;

pub const BLS12_381_FIELD_TO_G1_GAS: u64 = 5500;
pub const BLS12_381_FIELD_EXT_TO_G2_GAS: u64 = 23800;

pub struct Bls12381G1MappingPrecompile<const USE_ADVICE: bool>;

impl<R: Resources, const USE_ADVICE: bool> SystemFunctionExt<R, Bls12PrecompileErrors>
    for Bls12381G1MappingPrecompile<USE_ADVICE>
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
        _allocator: A,
    ) -> Result<(), zk_ee::system::errors::subsystem::SubsystemError<Bls12PrecompileErrors>> {
        cycle_marker::wrap_with_resources!("bls12_381_map_fp_to_g1", resources, {
            bls12_381_map_fp_to_g1_as_system_function_inner(
                input,
                output,
                resources,
                if USE_ADVICE { Some(oracle) } else { None },
            )
        })
    }
}

fn bls12_381_map_fp_to_g1_as_system_function_inner<
    D: zk_ee::common_traits::TryExtend<u8> + ?Sized,
    R: Resources,
    O: MemoryOracle,
>(
    input: &[u8],
    output: &mut D,
    resources: &mut R,
    oracle: Option<&mut O>,
) -> Result<(), zk_ee::system::errors::subsystem::SubsystemError<Bls12PrecompileErrors>> {
    if input.len() != FIELD_ELEMENT_SERIALIZATION_LEN {
        return Err(interface_error!(
            Bls12PrecompileInterfaceError::InvalidInputSize
        ));
    }

    let cost_gas = BLS12_381_FIELD_TO_G1_GAS;
    let cost_native = crate::cost_constants::BLS12_381_MAP_FP_TO_G1_NATIVE_COST;
    resources.charge_legacy_gas_and_native(cost_gas, cost_native)?;

    let result =
        bls12_381_map_fp_to_g1(input.try_into().unwrap(), oracle).map_err(interface_error_of)?;

    write_g1(result, output)?;

    Ok(())
}

pub struct Bls12381G2MappingPrecompile<const USE_ADVICE: bool>;

impl<R: Resources, const USE_ADVICE: bool> SystemFunctionExt<R, Bls12PrecompileErrors>
    for Bls12381G2MappingPrecompile<USE_ADVICE>
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
        _allocator: A,
    ) -> Result<(), zk_ee::system::errors::subsystem::SubsystemError<Bls12PrecompileErrors>> {
        cycle_marker::wrap_with_resources!("bls12_381_map_fp2_to_g2", resources, {
            bls12_381_map_fp2_to_g2_as_system_function_inner(
                input,
                output,
                resources,
                if USE_ADVICE { Some(oracle) } else { None },
            )
        })
    }
}

fn bls12_381_map_fp2_to_g2_as_system_function_inner<
    D: zk_ee::common_traits::TryExtend<u8> + ?Sized,
    R: Resources,
    O: MemoryOracle,
>(
    input: &[u8],
    output: &mut D,
    resources: &mut R,
    oracle: Option<&mut O>,
) -> Result<(), zk_ee::system::errors::subsystem::SubsystemError<Bls12PrecompileErrors>> {
    if input.len() != FIELD_EXT_ELEMENT_SERIALIZATION_LEN {
        return Err(interface_error!(
            Bls12PrecompileInterfaceError::InvalidInputSize
        ));
    }

    let cost_gas = BLS12_381_FIELD_EXT_TO_G2_GAS;
    let cost_native = crate::cost_constants::BLS12_381_MAP_FP2_TO_G2_NATIVE_COST;
    resources.charge_legacy_gas_and_native(cost_gas, cost_native)?;

    let result =
        bls12_381_map_fp2_to_g2(input.try_into().unwrap(), oracle).map_err(interface_error_of)?;

    write_g2(result, output)?;

    Ok(())
}

#[cfg(test)]
mod tests;
