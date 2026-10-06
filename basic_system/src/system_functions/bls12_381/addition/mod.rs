use crate::system_functions::curve_hints;
use crypto::ark_ec::CurveGroup;
use zk_ee::oracle::IOOracle;
use zk_ee::system::logger::Logger;
use zk_ee::system::{Resources, SystemFunctionExt};

use super::*;

pub const BLS12_381_G1_ADDITION_GAS: u64 = 375;
pub const BLS12_381_G2_ADDITION_GAS: u64 = 600;

pub struct Bls12381G1AdditionPrecompile<const USE_ADVICE: bool>;

impl<R: Resources, const USE_ADVICE: bool> SystemFunctionExt<R, Bls12PrecompileErrors>
    for Bls12381G1AdditionPrecompile<USE_ADVICE>
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
        cycle_marker::wrap_with_resources!("bls12_381_g1_add", resources, {
            bls12_381_g1_add_as_system_function_inner(
                input,
                output,
                resources,
                if USE_ADVICE { Some(oracle) } else { None },
            )
        })
    }
}

fn bls12_381_g1_add_as_system_function_inner<
    D: zk_ee::common_traits::TryExtend<u8> + ?Sized,
    R: Resources,
    O: IOOracle,
>(
    input: &[u8],
    output: &mut D,
    resources: &mut R,
    oracle: Option<&mut O>,
) -> Result<(), zk_ee::system::errors::subsystem::SubsystemError<Bls12PrecompileErrors>> {
    if input.len() != G1_SERIALIZATION_LEN * 2 {
        return Err(interface_error!(
            Bls12PrecompileInterfaceError::InvalidInputSize
        ));
    }

    let cost_gas = BLS12_381_G1_ADDITION_GAS;
    let cost_native = crate::cost_constants::BLS12_381_G1ADD_NATIVE_COST;
    resources.charge_legacy_gas_and_native(cost_gas, cost_native)?;

    let p0 = parse_g1(input[0..G1_SERIALIZATION_LEN].try_into().unwrap())?;
    let p1 = parse_g1(
        input[G1_SERIALIZATION_LEN..(2 * G1_SERIALIZATION_LEN)]
            .try_into()
            .unwrap(),
    )?;

    let result = match oracle {
        Some(oracle) => {
            g1::add_affine_with_divider(&p0, &p1, &mut curve_hints::Bls12381FqDivider::new(oracle))
        }
        None => (p0 + p1).into_affine(),
    };

    write_g1(result, output)?;

    Ok(())
}

pub struct Bls12381G2AdditionPrecompile<const USE_ADVICE: bool>;

impl<R: Resources, const USE_ADVICE: bool> SystemFunctionExt<R, Bls12PrecompileErrors>
    for Bls12381G2AdditionPrecompile<USE_ADVICE>
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
        cycle_marker::wrap_with_resources!("bls12_381_g2_add", resources, {
            bls12_381_g2_add_as_system_function_inner(
                input,
                output,
                resources,
                if USE_ADVICE { Some(oracle) } else { None },
            )
        })
    }
}

fn bls12_381_g2_add_as_system_function_inner<
    D: zk_ee::common_traits::TryExtend<u8> + ?Sized,
    R: Resources,
    O: IOOracle,
>(
    input: &[u8],
    output: &mut D,
    resources: &mut R,
    oracle: Option<&mut O>,
) -> Result<(), zk_ee::system::errors::subsystem::SubsystemError<Bls12PrecompileErrors>> {
    if input.len() != G2_SERIALIZATION_LEN * 2 {
        return Err(interface_error!(
            Bls12PrecompileInterfaceError::InvalidInputSize
        ));
    }

    let cost_gas = BLS12_381_G2_ADDITION_GAS;
    let cost_native = crate::cost_constants::BLS12_381_G2ADD_NATIVE_COST;
    resources.charge_legacy_gas_and_native(cost_gas, cost_native)?;

    let p0 = parse_g2(input[0..G2_SERIALIZATION_LEN].try_into().unwrap())?;
    let p1 = parse_g2(
        input[G2_SERIALIZATION_LEN..(2 * G2_SERIALIZATION_LEN)]
            .try_into()
            .unwrap(),
    )?;

    let result = match oracle {
        Some(oracle) => {
            g2::add_affine_with_divider(&p0, &p1, &mut curve_hints::Bls12381Fq2Divider::new(oracle))
        }
        None => (p0 + p1).into_affine(),
    };

    write_g2(result, output)?;

    Ok(())
}

#[cfg(test)]
mod tests;
