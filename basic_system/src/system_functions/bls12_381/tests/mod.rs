use super::*;
use std::alloc::Global;
use zk_ee::{
    reference_implementations::{BaseResources, DecreasingNative},
    system::{errors::subsystem::SubsystemError, Resource, SystemFunctionExt},
};

type TestResources = BaseResources<DecreasingNative>;

fn assert_invalid_input_size_and_preserves_resources<F>(input: Vec<u8>)
where
    F: SystemFunctionExt<TestResources, Bls12PrecompileErrors>,
{
    let mut output = Vec::new();
    let mut resources = TestResources::FORMAL_INFINITE;
    let initial_resources = resources.clone();

    let err = F::execute(
        &input,
        &mut output,
        &mut resources,
        &mut Unqueried,
        &mut zk_ee::system::logger::NullLogger,
        Global,
    )
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
    assert_invalid_input_size_and_preserves_resources::<Bls12381G1AdditionPrecompile<true>>(vec![
        0u8;
        G1_SERIALIZATION_LEN * 2 - 1
    ]);
}

#[test]
fn malformed_g2_addition_input_is_free() {
    assert_invalid_input_size_and_preserves_resources::<Bls12381G2AdditionPrecompile<true>>(vec![
        0u8;
        G2_SERIALIZATION_LEN * 2 + 1
    ]);
}

#[test]
fn malformed_g1_mapping_input_is_free() {
    assert_invalid_input_size_and_preserves_resources::<Bls12381G1MappingPrecompile<true>>(vec![
        0u8;
        FIELD_ELEMENT_SERIALIZATION_LEN - 1
    ]);
}

#[test]
fn malformed_g2_mapping_input_is_free() {
    assert_invalid_input_size_and_preserves_resources::<Bls12381G2MappingPrecompile<true>>(vec![
        0u8;
        FIELD_EXT_ELEMENT_SERIALIZATION_LEN + 1
    ]);
}

#[test]
fn malformed_g1_msm_input_is_free() {
    assert_invalid_input_size_and_preserves_resources::<Bls12381G1MSMPrecompile<true>>(vec![
        0u8;
        G1_MSM_PAIR_LEN
            + 1
    ]);
}

#[test]
fn malformed_g2_msm_input_is_free() {
    assert_invalid_input_size_and_preserves_resources::<Bls12381G2MSMPrecompile<true>>(vec![
        0u8;
        G2_MSM_PAIR_LEN
            + 1
    ]);
}

#[test]
fn malformed_pairing_input_is_free() {
    assert_invalid_input_size_and_preserves_resources::<Bls12381PairingCheckPrecompile<true>>(
        vec![0u8; BLS12_381_PAIR_LEN + 1],
    );
}

/// An oracle used to verify that early errors and the non-advised path make no queries.
pub(super) struct Unqueried;

impl zk_ee::oracle::memory_io::MemoryOracle for Unqueried {
    fn send_query(
        &mut self,
        query_id: u32,
        _: usize,
    ) -> Result<(), zk_ee::system::errors::internal::InternalError> {
        panic!("unexpected query {query_id}")
    }
}

impl zk_ee::oracle::IOOracle for Unqueried {
    type RawIterator<'a> = core::iter::Empty<usize>;
    fn raw_query<
        'a,
        I: zk_ee::oracle::usize_serialization::UsizeSerializable
            + zk_ee::oracle::usize_serialization::UsizeDeserializable,
    >(
        &'a mut self,
        query_id: u32,
        _: &I,
    ) -> Result<Self::RawIterator<'a>, zk_ee::system::errors::internal::InternalError> {
        panic!("unexpected query {query_id}")
    }
}

pub(super) fn honest_oracle() -> oracle_provider::ZkEENonDeterminismSource {
    let mut oracle = oracle_provider::ZkEENonDeterminismSource::default();
    oracle.add_external_processor(callable_oracles::field_hints::NativeFieldOpsQuery);
    oracle
}

pub(super) fn compare<
    F: SystemFunctionExt<TestResources, Bls12PrecompileErrors>,
    H: SystemFunctionExt<TestResources, Bls12PrecompileErrors>,
>(
    input: &[u8],
) -> Vec<u8> {
    let mut output = Vec::new();
    let mut hinted_output = Vec::new();
    let mut resources = TestResources::FORMAL_INFINITE;
    let mut hinted_resources = resources.clone();
    let result = F::execute(
        input,
        &mut output,
        &mut resources,
        &mut Unqueried,
        &mut zk_ee::system::logger::NullLogger,
        Global,
    );
    let hinted = H::execute(
        input,
        &mut hinted_output,
        &mut hinted_resources,
        &mut honest_oracle(),
        &mut zk_ee::system::logger::NullLogger,
        Global,
    );
    assert_eq!(format!("{result:?}"), format!("{hinted:?}"));
    assert_eq!(output, hinted_output);
    assert_eq!(resources, hinted_resources);
    result.expect("valid input");
    output
}
