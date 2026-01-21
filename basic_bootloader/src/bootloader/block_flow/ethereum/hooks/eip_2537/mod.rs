use crypto::ark_ec::AffineRepr;
use system_hooks::add_precompile;
use zk_ee::common_structs::system_hooks::HooksStorage;
use zk_ee::interface_error;

define_subsystem!(Bls12Precompile,
  interface Bls12PrecompileInterfaceError
  {
      InvalidFieldElement,
      InvalidG1Point,
      InvalidG2Point,
      InvalidInputSize,
      PointNotInSubgroup,
  }
);

use evm_interpreter::ERGS_PER_GAS;

use crypto::ark_ff::PrimeField;
use crypto::bls12_381::*;
use zk_ee::define_subsystem;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::system::{EthereumLikeTypes, IOSubsystemExt};

mod addition;
mod addresses;
mod mappings;
mod msm;
mod pairing;

pub use self::addition::{Bls12381G1AdditionPrecompile, Bls12381G2AdditionPrecompile};
pub use self::addresses::*;
pub use self::mappings::{Bls12381G1MappingPrecompile, Bls12381G2MappingPrecompile};
pub use self::msm::{Bls12381G1MSMPrecompile, Bls12381G2MSMPrecompile};
pub use self::pairing::Bls12381PairingCheckPrecompile;

pub fn initialize_eip_2537<S: EthereumLikeTypes>(
    hooks: &mut HooksStorage<S, S::Allocator>,
) -> Result<(), InternalError>
where
    S::IO: IOSubsystemExt,
{
    add_precompile::<S, S::Allocator, Bls12381G1AdditionPrecompile, Bls12PrecompileErrors>(
        hooks,
        BLS12_G1ADD,
    )?;
    add_precompile::<S, S::Allocator, Bls12381G2AdditionPrecompile, Bls12PrecompileErrors>(
        hooks,
        BLS12_G2ADD,
    )?;
    add_precompile::<S, S::Allocator, Bls12381G1MSMPrecompile, Bls12PrecompileErrors>(
        hooks,
        BLS12_G1MSM,
    )?;
    add_precompile::<S, S::Allocator, Bls12381G2MSMPrecompile, Bls12PrecompileErrors>(
        hooks,
        BLS12_G2MSM,
    )?;
    add_precompile::<S, S::Allocator, Bls12381PairingCheckPrecompile, Bls12PrecompileErrors>(
        hooks,
        BLS12_PAIRING_CHECK,
    )?;
    add_precompile::<S, S::Allocator, Bls12381G1MappingPrecompile, Bls12PrecompileErrors>(
        hooks,
        BLS12_MAP_FP_TO_G1,
    )?;
    add_precompile::<S, S::Allocator, Bls12381G2MappingPrecompile, Bls12PrecompileErrors>(
        hooks,
        BLS12_MAP_FP2_TO_G2,
    )?;
    Ok(())
}

const SCALAR_SERIALIZATION_LEN: usize = 32;
const FIELD_ELEMENT_SERIALIZATION_LEN: usize = 64;
const FIELD_EXT_ELEMENT_SERIALIZATION_LEN: usize = FIELD_ELEMENT_SERIALIZATION_LEN * 2;
const G1_SERIALIZATION_LEN: usize = FIELD_ELEMENT_SERIALIZATION_LEN * 2;
const G2_SERIALIZATION_LEN: usize = FIELD_EXT_ELEMENT_SERIALIZATION_LEN * 2;

// infallible, as scalars are no required to be canonical
fn parse_integer(input: &[u8; SCALAR_SERIALIZATION_LEN]) -> <Fr as PrimeField>::BigInt {
    let mut repr = [0u64; 4];
    for (dst, src) in repr.iter_mut().zip(input.as_rchunks::<8>().1.iter().rev()) {
        *dst = u64::from_be_bytes(*src);
    }
    crypto::BigInt::new(repr)
}

// Parse functions without Fq/G1/G2 types in signatures
fn parse_g1(input: &[u8; G1_SERIALIZATION_LEN]) -> Result<G1Affine, Bls12PrecompileSubsystemError> {
    crypto::bls12_381::eip2537::parse_g1_bytes(input)
        .map(|(point, _)| point)
        .ok_or_else(|| interface_error!(Bls12PrecompileInterfaceError::InvalidG1Point))
}

fn parse_g2(input: &[u8; G2_SERIALIZATION_LEN]) -> Result<G2Affine, Bls12PrecompileSubsystemError> {
    crypto::bls12_381::eip2537::parse_g2_bytes(input)
        .map(|(point, _)| point)
        .ok_or_else(|| interface_error!(Bls12PrecompileInterfaceError::InvalidG1Point))
}

fn parse_g1_with_subgroup_check(
    input: &[u8; G1_SERIALIZATION_LEN],
) -> Result<G1Affine, Bls12PrecompileSubsystemError> {
    let point = parse_g1(input)?;
    if point.is_zero() || point.is_in_correct_subgroup_assuming_on_curve() {
        Ok(point)
    } else {
        Err(interface_error!(
            Bls12PrecompileInterfaceError::PointNotInSubgroup
        ))
    }
}

fn parse_g2_with_subgroup_check(
    input: &[u8; G2_SERIALIZATION_LEN],
) -> Result<G2Affine, Bls12PrecompileSubsystemError> {
    let point = parse_g2(input)?;
    if point.is_zero() || point.is_in_correct_subgroup_assuming_on_curve() {
        Ok(point)
    } else {
        Err(interface_error!(
            Bls12PrecompileInterfaceError::PointNotInSubgroup
        ))
    }
}

fn write_g1<D: zk_ee::common_traits::TryExtend<u8> + ?Sized>(el: G1Affine, output: &mut D) {
    let mut buffer = [0u8; G1_SERIALIZATION_LEN];
    crypto::bls12_381::eip2537::serialize_g1_bytes(el, &mut buffer);
    output.try_extend(buffer).map_err(|_| ()).unwrap();
}

fn write_g2<D: zk_ee::common_traits::TryExtend<u8> + ?Sized>(el: G2Affine, output: &mut D) {
    let mut buffer = [0u8; G2_SERIALIZATION_LEN];
    crypto::bls12_381::eip2537::serialize_g2_bytes(el, &mut buffer);
    output.try_extend(buffer).map_err(|_| ()).unwrap();
}
