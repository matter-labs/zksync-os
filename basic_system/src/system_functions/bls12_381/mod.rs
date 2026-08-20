use crypto::ark_ec::AffineRepr;
use zk_ee::interface_error;
use zk_ee::out_of_return_memory;
use zk_ee::system::base_system_functions::{
    Bls12PrecompileErrors, Bls12PrecompileInterfaceError, Bls12PrecompileSubsystemError,
};

use evm_interpreter::ERGS_PER_GAS;

use crypto::ark_ff::PrimeField;
use crypto::bls12_381::*;

mod addition;
mod mappings;
mod msm;
mod pairing;

pub use self::addition::{Bls12381G1AdditionPrecompile, Bls12381G2AdditionPrecompile};
pub use self::mappings::{Bls12381G1MappingPrecompile, Bls12381G2MappingPrecompile};
pub use self::msm::{Bls12381G1MSMPrecompile, Bls12381G2MSMPrecompile};
pub use self::pairing::Bls12381PairingCheckPrecompile;

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
        .ok_or_else(|| interface_error!(Bls12PrecompileInterfaceError::InvalidG2Point))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::alloc::Global;
    use zk_ee::{
        reference_implementations::{BaseResources, DecreasingNative},
        system::{errors::subsystem::SubsystemError, Resource, SystemFunction},
    };

    type TestResources = BaseResources<DecreasingNative>;

    fn assert_invalid_input_size_and_preserves_resources<F>(input: Vec<u8>)
    where
        F: SystemFunction<TestResources, Bls12PrecompileErrors>,
    {
        let mut output = Vec::new();
        let mut resources = TestResources::FORMAL_INFINITE;
        let initial_resources = resources.clone();

        let err = F::execute(&input, &mut output, &mut resources, Global)
            .expect_err("malformed input must be rejected");

        assert_eq!(resources, initial_resources);
        assert!(output.is_empty());
        assert!(matches!(
            err,
            SubsystemError::LeafUsage(err)
                if err.0 == Bls12PrecompileInterfaceError::InvalidInputSize
        ));
    }

    #[test]
    fn malformed_g1_addition_input_is_free() {
        assert_invalid_input_size_and_preserves_resources::<Bls12381G1AdditionPrecompile>(vec![
            0u8;
            G1_SERIALIZATION_LEN * 2 - 1
        ]);
    }

    #[test]
    fn malformed_g2_addition_input_is_free() {
        assert_invalid_input_size_and_preserves_resources::<Bls12381G2AdditionPrecompile>(vec![
            0u8;
            G2_SERIALIZATION_LEN * 2 + 1
        ]);
    }

    #[test]
    fn malformed_g1_mapping_input_is_free() {
        assert_invalid_input_size_and_preserves_resources::<Bls12381G1MappingPrecompile>(vec![
            0u8;
            FIELD_ELEMENT_SERIALIZATION_LEN - 1
        ]);
    }

    #[test]
    fn malformed_g2_mapping_input_is_free() {
        assert_invalid_input_size_and_preserves_resources::<Bls12381G2MappingPrecompile>(vec![
            0u8;
            FIELD_EXT_ELEMENT_SERIALIZATION_LEN + 1
        ]);
    }

    #[test]
    fn malformed_g1_msm_input_is_free() {
        assert_invalid_input_size_and_preserves_resources::<Bls12381G1MSMPrecompile>(vec![
            0u8;
            msm::G1_MSM_PAIR_LEN
                + 1
        ]);
    }

    #[test]
    fn malformed_g2_msm_input_is_free() {
        assert_invalid_input_size_and_preserves_resources::<Bls12381G2MSMPrecompile>(vec![
            0u8;
            msm::G2_MSM_PAIR_LEN
                + 1
        ]);
    }

    #[test]
    fn malformed_pairing_input_is_free() {
        assert_invalid_input_size_and_preserves_resources::<Bls12381PairingCheckPrecompile>(
            vec![0u8; pairing::BLS12_381_PAIR_LEN + 1],
        );
    }
}
