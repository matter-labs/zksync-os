use zk_ee::interface_error;
use zk_ee::out_of_return_memory;
use zk_ee::system::base_system_functions::{
    Bls12PrecompileErrors, Bls12PrecompileInterfaceError, Bls12PrecompileSubsystemError,
};

use crypto::bls12_381::*;
use crypto::precompiles::{
    Bls12Error, BLS12_381_PAIR_LEN, FIELD_ELEMENT_SERIALIZATION_LEN,
    FIELD_EXT_ELEMENT_SERIALIZATION_LEN, G1_MSM_PAIR_LEN, G1_SERIALIZATION_LEN, G2_MSM_PAIR_LEN,
    G2_SERIALIZATION_LEN,
};

mod addition;
mod mappings;
mod msm;
mod pairing;

pub use self::addition::{Bls12381G1AdditionPrecompile, Bls12381G2AdditionPrecompile};
pub use self::mappings::{Bls12381G1MappingPrecompile, Bls12381G2MappingPrecompile};
pub use self::msm::{Bls12381G1MSMPrecompile, Bls12381G2MSMPrecompile};
pub use self::pairing::Bls12381PairingCheckPrecompile;

/// The interface error of the precompiles for an error of the crypto half
/// (`crypto::precompiles`)
fn interface_error_of(error: Bls12Error) -> Bls12PrecompileSubsystemError {
    match error {
        Bls12Error::InvalidG1Point => {
            interface_error!(Bls12PrecompileInterfaceError::InvalidG1Point)
        }
        Bls12Error::InvalidG2Point => {
            interface_error!(Bls12PrecompileInterfaceError::InvalidG2Point)
        }
        Bls12Error::NotInSubgroup => {
            interface_error!(Bls12PrecompileInterfaceError::PointNotInSubgroup)
        }
        Bls12Error::InvalidFieldElement => {
            interface_error!(Bls12PrecompileInterfaceError::InvalidFieldElement)
        }
    }
}

fn write_g1<D: zk_ee::common_traits::TryExtend<u8> + ?Sized>(
    el: G1Affine,
    output: &mut D,
) -> Result<(), Bls12PrecompileSubsystemError> {
    let mut buffer = [0u8; G1_SERIALIZATION_LEN];
    crypto::bls12_381::eip2537::serialize_g1_bytes(el, &mut buffer);
    output
        .try_extend(buffer)
        .map_err(|_| out_of_return_memory!())?;
    Ok(())
}

fn write_g2<D: zk_ee::common_traits::TryExtend<u8> + ?Sized>(
    el: G2Affine,
    output: &mut D,
) -> Result<(), Bls12PrecompileSubsystemError> {
    let mut buffer = [0u8; G2_SERIALIZATION_LEN];
    crypto::bls12_381::eip2537::serialize_g2_bytes(el, &mut buffer);
    output
        .try_extend(buffer)
        .map_err(|_| out_of_return_memory!())?;
    Ok(())
}

// the decoding of the outputs, for the tests
#[cfg(test)]
const SCALAR_SERIALIZATION_LEN: usize = 32;

#[cfg(test)]
fn parse_g1(input: &[u8; G1_SERIALIZATION_LEN]) -> Option<G1Affine> {
    crypto::bls12_381::eip2537::parse_g1_bytes(input).map(|(point, _)| point)
}

#[cfg(test)]
fn parse_g2(input: &[u8; G2_SERIALIZATION_LEN]) -> Option<G2Affine> {
    crypto::bls12_381::eip2537::parse_g2_bytes(input).map(|(point, _)| point)
}

#[cfg(test)]
mod tests;
