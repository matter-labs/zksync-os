use super::*;
use crypto::ark_ec::AffineRepr;
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

pub fn initialize_eip_2537<S: EthereumLikeTypes>(hooks_storage: &mut HooksStorage<S, S::Allocator>)
where
    S::IO: IOSubsystemExt,
{
    hooks_storage.add_precompile_from_pure_invocation::<Bls12381G1AdditionPrecompile>(BLS12_G1ADD);
    hooks_storage.add_precompile_from_pure_invocation::<Bls12381G2AdditionPrecompile>(BLS12_G2ADD);
    hooks_storage.add_precompile_from_pure_invocation::<Bls12381G1MSMPrecompile>(BLS12_G1MSM);
    hooks_storage.add_precompile_from_pure_invocation::<Bls12381G2MSMPrecompile>(BLS12_G2MSM);
    hooks_storage
        .add_precompile_from_pure_invocation::<Bls12381PairingCheckPrecompile>(BLS12_PAIRING_CHECK);
    hooks_storage
        .add_precompile_from_pure_invocation::<Bls12381G1MappingPrecompile>(BLS12_MAP_FP_TO_G1);
    hooks_storage
        .add_precompile_from_pure_invocation::<Bls12381G2MappingPrecompile>(BLS12_MAP_FP2_TO_G2);
}

const SCALAR_SERIALIZATION_LEN: usize = 32;
const FIELD_ELEMENT_SERIALIZATION_LEN: usize = 64;
const FIELD_EXT_ELEMENT_SERIALIZATION_LEN: usize = FIELD_ELEMENT_SERIALIZATION_LEN * 2;
const G1_SERIALIZATION_LEN: usize = FIELD_ELEMENT_SERIALIZATION_LEN * 2;
const G2_SERIALIZATION_LEN: usize = FIELD_EXT_ELEMENT_SERIALIZATION_LEN * 2;

// infalliable, as scalars are no required to be canonical
fn parse_integer(input: &[u8; SCALAR_SERIALIZATION_LEN]) -> <Fr as PrimeField>::BigInt {
    let mut repr = [0u64; 4];
    for (dst, src) in repr.iter_mut().zip(input.as_rchunks::<8>().1.iter().rev()) {
        *dst = u64::from_be_bytes(*src);
    }
    crypto::BigInt::new(repr)
}

fn parse_fq(
    input: &[u8; FIELD_ELEMENT_SERIALIZATION_LEN],
) -> Result<Fq, Bls12PrecompileSubsystemError> {
    if !input[..16].iter().all(|el| *el == 0) {
        return Err(Bls12PrecompileSubsystemError::LeafUsage(interface_error!(
            Bls12PrecompileInterfaceError::InvalidFieldElement
        )));
    }
    // account for potentially variable representations
    let mut repr = <Fq as PrimeField>::BigInt::zero().0;
    for (dst, src) in repr
        .iter_mut()
        .zip(input[16..].as_rchunks::<8>().1.iter().rev())
    {
        *dst = u64::from_be_bytes(*src);
    }
    let repr = crypto::BigInt::new(repr);
    if repr >= Fq::MODULUS {
        return Err(Bls12PrecompileSubsystemError::LeafUsage(interface_error!(
            Bls12PrecompileInterfaceError::InvalidFieldElement
        )));
    }

    Ok(Fq::new(repr))
}

fn parse_fq2(
    input: &[u8; FIELD_EXT_ELEMENT_SERIALIZATION_LEN],
) -> Result<Fq2, Bls12PrecompileSubsystemError> {
    let c0 = parse_fq(input[0..64].try_into().unwrap())?;
    let c1 = parse_fq(input[64..128].try_into().unwrap())?;

    Ok(Fq2 { c0, c1 })
}

// No subgroup check
fn parse_g1(input: &[u8; G1_SERIALIZATION_LEN]) -> Result<G1Affine, Bls12PrecompileSubsystemError> {
    if input.iter().all(|el| *el == 0) {
        Ok(G1Affine::identity())
    } else {
        let x = parse_fq(input[0..64].try_into().unwrap())?;
        let y = parse_fq(input[64..128].try_into().unwrap())?;
        let maybe_point = G1Affine::new_unchecked(x, y);

        if !maybe_point.is_on_curve() {
            return Err(Bls12PrecompileSubsystemError::LeafUsage(interface_error!(
                Bls12PrecompileInterfaceError::InvalidG1Point
            )));
        }

        Ok(maybe_point)
    }
}

// No subgroup check
fn parse_g2(input: &[u8; G2_SERIALIZATION_LEN]) -> Result<G2Affine, Bls12PrecompileSubsystemError> {
    if input.iter().all(|el| *el == 0) {
        Ok(G2Affine::identity())
    } else {
        let x = parse_fq2(input[0..128].try_into().unwrap())?;
        let y = parse_fq2(input[128..256].try_into().unwrap())?;
        let maybe_point = G2Affine::new_unchecked(x, y);

        if !maybe_point.is_on_curve() {
            return Err(Bls12PrecompileSubsystemError::LeafUsage(interface_error!(
                Bls12PrecompileInterfaceError::InvalidG1Point
            )));
        }

        Ok(maybe_point)
    }
}

fn parse_g1_with_subgroup_check(
    input: &[u8; G1_SERIALIZATION_LEN],
) -> Result<G1Affine, Bls12PrecompileSubsystemError> {
    let point = parse_g1(input)?;
    if point.is_zero() || point.is_in_correct_subgroup_assuming_on_curve() {
        Ok(point)
    } else {
        Err(Bls12PrecompileSubsystemError::LeafUsage(interface_error!(
            Bls12PrecompileInterfaceError::PointNotInSubgroup
        )))
    }
}

fn parse_g2_with_subgroup_check(
    input: &[u8; G2_SERIALIZATION_LEN],
) -> Result<G2Affine, Bls12PrecompileSubsystemError> {
    let point = parse_g2(input)?;
    if point.is_zero() || point.is_in_correct_subgroup_assuming_on_curve() {
        Ok(point)
    } else {
        Err(Bls12PrecompileSubsystemError::LeafUsage(interface_error!(
            Bls12PrecompileInterfaceError::PointNotInSubgroup
        )))
    }
}

fn write_fq(el: Fq, output: &mut SliceVec<'_, u8>) {
    output.extend_from_slice(&[0u8; 16]);
    // BE
    for word in el.into_bigint().0[..6].iter().rev() {
        output.extend_from_slice(&word.to_be_bytes());
    }
}

fn write_g1(el: G1Affine, output: &mut SliceVec<'_, u8>) {
    if let Some((x, y)) = el.xy() {
        write_fq(x, output);
        write_fq(y, output);
    } else {
        // all zeroes
        output.extend_from_slice(&[0u8; G1_SERIALIZATION_LEN]);
    }
}

fn write_g2(el: G2Affine, output: &mut SliceVec<'_, u8>) {
    if let Some((x, y)) = el.xy() {
        write_fq(x.c0, output);
        write_fq(x.c1, output);
        write_fq(y.c0, output);
        write_fq(y.c1, output);
    } else {
        // all zeroes
        output.extend_from_slice(&[0u8; G2_SERIALIZATION_LEN]);
    }
}
