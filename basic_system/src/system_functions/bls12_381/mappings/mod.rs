use crate::system_functions::curve_hints;
use core::cell::RefCell;
use crypto::ark_ff::Zero;
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
    O: IOOracle,
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

    let field_element = crypto::bls12_381::eip2537::parse_fq_bytes(input.try_into().unwrap())
        .ok_or_else(|| interface_error!(Bls12PrecompileInterfaceError::InvalidFieldElement))?;
    let invalid_field = || interface_error!(Bls12PrecompileInterfaceError::InvalidFieldElement);
    let result = match oracle {
        Some(oracle) => {
            // The callbacks run sequentially; both need access to the same oracle.
            let oracle = RefCell::new(oracle);
            let point = crypto::bls12_381::eip2537::map_fp_to_g1_with_hints(
                field_element,
                |value| curve_hints::bls12_381_fq_inverse(*oracle.borrow_mut(), value),
                |value, z| {
                    let mut oracle = oracle.borrow_mut();
                    // Match SWU's Legendre convention: zero takes the non-residue branch.
                    if !value.is_zero() {
                        if let Some(root) = curve_hints::bls12_381_fq_sqrt(*oracle, value) {
                            return (root, true);
                        }
                    }
                    let root = curve_hints::bls12_381_fq_sqrt(*oracle, &(*z * value))
                        .expect("the SWU non-residue times a non-square is square");
                    (root, false)
                },
            );
            crypto::bls12_381::curves::g1::clear_cofactor_with_divider(
                &point,
                &mut curve_hints::Bls12381FqDivider::new(oracle.into_inner()),
            )
        }
        None => crypto::bls12_381::eip2537::map_fp_to_g1(field_element)
            .map_err(|_| invalid_field())?
            .clear_cofactor(),
    };

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
    O: IOOracle,
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

    let field_element = crypto::bls12_381::eip2537::parse_fq2_bytes(input.try_into().unwrap())
        .ok_or_else(|| interface_error!(Bls12PrecompileInterfaceError::InvalidFieldElement))?;

    let invalid_field = || interface_error!(Bls12PrecompileInterfaceError::InvalidFieldElement);
    let result = match oracle {
        Some(oracle) => {
            // The callbacks run sequentially; both need access to the same oracle.
            let oracle = RefCell::new(oracle);
            let point = crypto::bls12_381::eip2537::map_fp2_to_g2_with_hints(
                field_element,
                |value| curve_hints::bls12_381_fq2_inverse(*oracle.borrow_mut(), value),
                |value, z| {
                    let mut oracle = oracle.borrow_mut();
                    // Match SWU's Legendre convention: zero takes the non-residue branch.
                    if !value.is_zero() {
                        if let Some(root) = curve_hints::bls12_381_fq2_sqrt(*oracle, value) {
                            return (root, true);
                        }
                    }
                    let root = curve_hints::bls12_381_fq2_sqrt(*oracle, &(*z * value))
                        .expect("the SWU non-residue times a non-square is square");
                    (root, false)
                },
            );
            crypto::bls12_381::curves::g2::clear_cofactor_with_divider(
                &point,
                &mut curve_hints::Bls12381Fq2Divider::new(oracle.into_inner()),
            )
        }
        None => crypto::bls12_381::eip2537::map_fp2_to_g2(field_element)
            .map_err(|_| invalid_field())?
            .clear_cofactor(),
    };

    write_g2(result, output)?;

    Ok(())
}

#[cfg(test)]
mod tests;
