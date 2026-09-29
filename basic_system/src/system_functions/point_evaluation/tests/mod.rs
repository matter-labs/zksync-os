use super::*;
use callable_oracles::field_hints::NativeFieldOpsQuery;
use oracle_provider::ZkEENonDeterminismSource;
use std::alloc::Global;
use zk_ee::reference_implementations::BaseResources;
use zk_ee::reference_implementations::DecreasingNative;
use zk_ee::system::logger::NullLogger;
use zk_ee::system::Resource;

/// Runs the precompile without and with the oracle hints; both must agree
fn execute(
    input: &[u8],
    output: &mut Vec<u8>,
    resources: &mut TestResources,
) -> Result<(), SubsystemError<PointEvaluationErrors>> {
    let mut oracle = ZkEENonDeterminismSource::default();
    oracle.add_external_processor(NativeFieldOpsQuery);
    let mut hinted_output = Vec::new();
    let mut hinted_resources = infinite_resources();
    let hinted = PointEvaluationImpl::<true>::execute(
        input,
        &mut hinted_output,
        &mut hinted_resources,
        &mut oracle,
        &mut NullLogger,
        Global,
    );
    let result = PointEvaluationImpl::<false>::execute(
        input,
        output,
        resources,
        &mut oracle,
        &mut NullLogger,
        Global,
    );
    assert_eq!(result.is_ok(), hinted.is_ok(), "{result:?} vs {hinted:?}");
    assert_eq!(*output, hinted_output);
    assert_eq!(resources.legacy_gas(), hinted_resources.legacy_gas());
    // a prover claiming an invalid proof (with a correct inverse) does not change the answer
    let mut claiming = curve_hints::tests::non_identity_claiming_oracle();
    let mut claiming_output = Vec::new();
    let claimed = PointEvaluationImpl::<true>::execute(
        input,
        &mut claiming_output,
        &mut infinite_resources(),
        &mut claiming,
        &mut NullLogger,
        Global,
    );
    assert_eq!(result.is_ok(), claimed.is_ok(), "{result:?} vs {claimed:?}");
    assert_eq!(*output, claiming_output);
    result
}

use alloy_primitives::hex;

type TestResources = BaseResources<DecreasingNative>;

fn infinite_resources() -> TestResources {
    TestResources::FORMAL_INFINITE
}

/// One call of the precompile: its result, its output and the legacy gas and native it
/// left
type Outcome = (
    Result<(), SubsystemError<PointEvaluationErrors>>,
    Vec<u8>,
    u64,
    u64,
);

fn outcome(
    result: Result<(), SubsystemError<PointEvaluationErrors>>,
    output: Vec<u8>,
    resources: &TestResources,
) -> Outcome {
    (
        result,
        output,
        resources.legacy_gas(),
        resources.native().as_u64(),
    )
}

fn uncached<const USE_ADVICE: bool, O: IOOracle>(
    input: &[u8],
    mut resources: TestResources,
    oracle: &mut O,
) -> Outcome {
    let mut output = Vec::new();
    let result = PointEvaluationImpl::<USE_ADVICE>::execute(
        input,
        &mut output,
        &mut resources,
        oracle,
        &mut NullLogger,
        Global,
    );
    outcome(result, output, &resources)
}

fn cached<const USE_ADVICE: bool, O: IOOracle>(
    input: &[u8],
    mut resources: TestResources,
    oracle: &mut O,
    caches: &mut PrecompileCaches,
) -> Outcome {
    let mut output = Vec::new();
    let result = PointEvaluationImpl::<USE_ADVICE>::execute_with_precompile_caches(
        input,
        &mut output,
        &mut resources,
        oracle,
        caches,
        &mut NullLogger,
        Global,
    );
    outcome(result, output, &resources)
}

fn honest_oracle() -> ZkEENonDeterminismSource {
    let mut oracle = ZkEENonDeterminismSource::default();
    oracle.add_external_processor(NativeFieldOpsQuery);
    oracle
}

/// An oracle that must not be queried at all.
struct Unqueried;

impl zk_ee::oracle::memory_io::MemoryOracle for Unqueried {
    fn send_query(
        &mut self,
        query_id: u32,
        _input_word: usize,
    ) -> Result<(), zk_ee::system::errors::internal::InternalError> {
        panic!("unexpected query 0x{query_id:08x}")
    }
}

impl IOOracle for Unqueried {
    type RawIterator<'a> = core::iter::Empty<usize>;

    fn raw_query<
        'a,
        I: zk_ee::oracle::usize_serialization::UsizeSerializable
            + zk_ee::oracle::usize_serialization::UsizeDeserializable,
    >(
        &'a mut self,
        query_type: u32,
        _input: &I,
    ) -> Result<Self::RawIterator<'a>, zk_ee::system::errors::internal::InternalError> {
        panic!("unexpected query 0x{query_type:08x}")
    }
}

fn is_cached(caches: &PrecompileCaches, input: &[u8]) -> bool {
    <&[u8; POINT_EVALUATION_INPUT_LEN]>::try_from(input)
        .is_ok_and(|input| caches.point_evaluation.get(input).is_some())
}

mod cache;
mod fixtures;
mod oracle;
mod parsing;
mod resources;
mod verification;

use fixtures::*;
