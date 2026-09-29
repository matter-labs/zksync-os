use super::*;
use zk_ee::oracle::usize_serialization::{UsizeDeserializable, UsizeSerializable};
use zk_ee::oracle::IOOracle;
use zk_ee::system::errors::internal::InternalError;

use zk_ee::reference_implementations::{BaseResources, DecreasingNative};
use zk_ee::system::{Computational, ErgsResource, Resource};

type TestResources = BaseResources<DecreasingNative>;
type Outcome = (
    Result<(), SubsystemError<Secp256k1ECRecoverErrors>>,
    Vec<u8>,
    u64,
    u64,
);

fn normalized(input: &[u8]) -> EcRecoverInput {
    let mut key = EcRecoverInput([0; ECRECOVER_INPUT_LEN]);
    for (dst, src) in key.0.iter_mut().zip(input) {
        *dst = *src;
    }
    key
}

fn honest_oracle() -> oracle_provider::ZkEENonDeterminismSource {
    let mut oracle = oracle_provider::ZkEENonDeterminismSource::default();
    oracle.add_external_processor(callable_oracles::field_hints::NativeFieldOpsQuery);
    oracle
}

fn call<const USE_ADVICE: bool, O: IOOracle>(
    input: &[u8],
    mut resources: TestResources,
    oracle: &mut O,
    caches: Option<&mut PrecompileCaches>,
) -> Outcome {
    use std::alloc::Global;
    use zk_ee::system::logger::NullLogger;
    let mut output = Vec::new();
    let result = match caches {
        Some(caches) => EcRecoverImpl::<USE_ADVICE>::execute_with_precompile_caches(
            input,
            &mut output,
            &mut resources,
            oracle,
            caches,
            &mut NullLogger,
            Global,
        ),
        None => EcRecoverImpl::<USE_ADVICE>::execute(
            input,
            &mut output,
            &mut resources,
            oracle,
            &mut NullLogger,
            Global,
        ),
    };
    (
        result,
        output,
        resources.legacy_gas(),
        resources.native().as_u64(),
    )
}

// The common hinted call with enough resources; resource-boundary tests use `call` directly.
fn run(input: &[u8], caches: Option<&mut PrecompileCaches>) -> Outcome {
    call::<true, _>(
        input,
        TestResources::FORMAL_INFINITE,
        &mut honest_oracle(),
        caches,
    )
}

mod cache;
mod fixtures;
mod oracle;
mod recovery;
mod resources;

use fixtures::*;
use oracle::record_tape;
