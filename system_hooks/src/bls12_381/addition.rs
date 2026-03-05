use crypto::ark_ec::CurveGroup;
use zk_ee::system::{Ergs, Resources, SystemFunction};

use super::*;

pub const BLS12_381_G1_ADDITION_GAS: u64 = 375;
pub const BLS12_381_G2_ADDITION_GAS: u64 = 600;

pub struct Bls12381G1AdditionPrecompile;

impl<R: Resources> SystemFunction<R, Bls12PrecompileErrors> for Bls12381G1AdditionPrecompile {
    fn execute<
        D: zk_ee::common_traits::TryExtend<u8> + ?Sized,
        A: core::alloc::Allocator + Clone,
    >(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        _allocator: A,
    ) -> Result<(), zk_ee::system::errors::subsystem::SubsystemError<Bls12PrecompileErrors>> {
        let cost_ergs = Ergs(BLS12_381_G1_ADDITION_GAS * ERGS_PER_GAS);
        let cost_native = 0;
        resources.charge(&R::from_ergs_and_native(
            cost_ergs,
            <R::Native as zk_ee::system::Computational>::from_computational(cost_native),
        ))?;

        if input.len() != G1_SERIALIZATION_LEN * 2 {
            return Err(interface_error!(
                Bls12PrecompileInterfaceError::InvalidInputSize
            ));
        }

        let p0 = parse_g1(input[0..G1_SERIALIZATION_LEN].try_into().unwrap())?;
        let p1 = parse_g1(
            input[G1_SERIALIZATION_LEN..(2 * G1_SERIALIZATION_LEN)]
                .try_into()
                .unwrap(),
        )?;

        let result = p0 + p1;
        let result = result.into_affine();

        write_g1(result, output);

        Ok(())
    }
}

pub struct Bls12381G2AdditionPrecompile;

impl<R: Resources> SystemFunction<R, Bls12PrecompileErrors> for Bls12381G2AdditionPrecompile {
    fn execute<
        D: zk_ee::common_traits::TryExtend<u8> + ?Sized,
        A: core::alloc::Allocator + Clone,
    >(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        _allocator: A,
    ) -> Result<(), zk_ee::system::errors::subsystem::SubsystemError<Bls12PrecompileErrors>> {
        let cost_ergs = Ergs(BLS12_381_G2_ADDITION_GAS * ERGS_PER_GAS);
        let cost_native = 0;
        resources.charge(&R::from_ergs_and_native(
            cost_ergs,
            <R::Native as zk_ee::system::Computational>::from_computational(cost_native),
        ))?;

        if input.len() != G2_SERIALIZATION_LEN * 2 {
            return Err(interface_error!(
                Bls12PrecompileInterfaceError::InvalidInputSize
            ));
        }

        let p0 = parse_g2(input[0..G2_SERIALIZATION_LEN].try_into().unwrap())?;
        let p1 = parse_g2(
            input[G2_SERIALIZATION_LEN..(2 * G2_SERIALIZATION_LEN)]
                .try_into()
                .unwrap(),
        )?;

        let result = p0 + p1;
        let result = result.into_affine();

        write_g2(result, output);

        Ok(())
    }
}
