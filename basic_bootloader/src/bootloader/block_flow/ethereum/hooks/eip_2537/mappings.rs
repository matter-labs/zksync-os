use zk_ee::system::{Ergs, Resources, SystemFunction};

use super::*;

pub const BLS12_381_FIELD_TO_G1_GAS: u64 = 5500;
pub const BLS12_381_FIELD_EXT_TO_G2_GAS: u64 = 23800;

pub struct Bls12381G1MappingPrecompile;

impl<R: Resources> SystemFunction<R, Bls12PrecompileErrors> for Bls12381G1MappingPrecompile {
    fn execute<
        D: zk_ee::common_traits::TryExtend<u8> + ?Sized,
        A: core::alloc::Allocator + Clone,
    >(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        _allocator: A,
    ) -> Result<(), zk_ee::system::errors::subsystem::SubsystemError<Bls12PrecompileErrors>> {
        if input.len() == 0 {
            return Err(interface_error!(
                Bls12PrecompileInterfaceError::InvalidInputSize
            ));
        }
        let cost_ergs = Ergs(BLS12_381_FIELD_TO_G1_GAS * ERGS_PER_GAS);
        let cost_native = 0;
        resources.charge(&R::from_ergs_and_native(
            cost_ergs,
            <R::Native as zk_ee::system::Computational>::from_computational(cost_native),
        ))?;
        if input.len() != FIELD_ELEMENT_SERIALIZATION_LEN {
            return Err(interface_error!(
                Bls12PrecompileInterfaceError::InvalidInputSize
            ));
        }

        let field_element = crypto::bls12_381::eip2537::parse_fq_bytes(input.try_into().unwrap())
            .ok_or_else(|| {
            interface_error!(Bls12PrecompileInterfaceError::InvalidFieldElement)
        })?;
        use crypto::ark_ec::hashing::map_to_curve_hasher::MapToCurve;
        let Ok(result) =
            crypto::ark_ec::hashing::curve_maps::wb::WBMap::map_to_curve(field_element)
        else {
            return Err(interface_error!(
                Bls12PrecompileInterfaceError::InvalidFieldElement
            ));
        };
        let result: G1Affine = result;
        let result = result.clear_cofactor();

        write_g1(result, output);

        Ok(())
    }
}

pub struct Bls12381G2MappingPrecompile;

impl<R: Resources> SystemFunction<R, Bls12PrecompileErrors> for Bls12381G2MappingPrecompile {
    fn execute<
        D: zk_ee::common_traits::TryExtend<u8> + ?Sized,
        A: core::alloc::Allocator + Clone,
    >(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        _allocator: A,
    ) -> Result<(), zk_ee::system::errors::subsystem::SubsystemError<Bls12PrecompileErrors>> {
        if input.len() == 0 {
            return Err(interface_error!(
                Bls12PrecompileInterfaceError::InvalidInputSize
            ));
        }
        let cost_ergs = Ergs(BLS12_381_FIELD_EXT_TO_G2_GAS * ERGS_PER_GAS);
        let cost_native = 0;
        resources.charge(&R::from_ergs_and_native(
            cost_ergs,
            <R::Native as zk_ee::system::Computational>::from_computational(cost_native),
        ))?;
        if input.len() != FIELD_EXT_ELEMENT_SERIALIZATION_LEN {
            return Err(interface_error!(
                Bls12PrecompileInterfaceError::InvalidInputSize
            ));
        }

        let field_element = crypto::bls12_381::eip2537::parse_fq2_bytes(input.try_into().unwrap())
            .ok_or_else(|| interface_error!(Bls12PrecompileInterfaceError::InvalidFieldElement))?;

        use crypto::ark_ec::hashing::map_to_curve_hasher::MapToCurve;
        let Ok(result) =
            crypto::ark_ec::hashing::curve_maps::wb::WBMap::map_to_curve(field_element)
        else {
            return Err(interface_error!(
                Bls12PrecompileInterfaceError::InvalidFieldElement
            ));
        };
        let result: G2Affine = result;
        let result = result.clear_cofactor();

        write_g2(result, output);

        Ok(())
    }
}
