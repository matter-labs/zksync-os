use super::*;
use crypto::ark_ff::AdditiveGroup;
use crypto::bls12_381::{Fq, Fq2};
use oracle_provider::ZkEENonDeterminismSource;
use std::alloc::Global;
use zk_ee::reference_implementations::{BaseResources, DecreasingNative};
use zk_ee::system::logger::NullLogger;
use zk_ee::system::Resource;

type R = BaseResources<DecreasingNative>;

fn call<F: SystemFunctionExt<R, Bls12PrecompileErrors>>(input: &[u8]) -> (Vec<u8>, R) {
    let mut oracle = ZkEENonDeterminismSource::default();
    oracle.add_external_processor(callable_oracles::field_hints::NativeFieldOpsQuery);
    let mut output = Vec::new();
    let mut resources = R::FORMAL_INFINITE;
    F::execute(
        input,
        &mut output,
        &mut resources,
        &mut oracle,
        &mut NullLogger,
        Global,
    )
    .unwrap();
    (output, resources)
}

#[test]
fn both_mapping_outputs_and_charges_match_without_hints() {
    for i in 0u64..=16 {
        let value = Fq::from(i * i + i);
        for value in [value, -value] {
            let mut encoded = [0u8; 64];
            crypto::bls12_381::eip2537::serialize_fq_bytes(value, &mut encoded);
            assert_eq!(
                call::<Bls12381G1MappingPrecompile<true>>(&encoded),
                call::<Bls12381G1MappingPrecompile<false>>(&encoded)
            );
            for value in [Fq2::new(value, Fq::ZERO), Fq2::new(value, Fq::from(i * 17))] {
                let mut encoded = [0u8; 128];
                crypto::bls12_381::eip2537::serialize_fq2_bytes(value, &mut encoded);
                assert_eq!(
                    call::<Bls12381G2MappingPrecompile<true>>(&encoded),
                    call::<Bls12381G2MappingPrecompile<false>>(&encoded)
                );
            }
        }
    }
}

#[test]
#[should_panic]
fn forged_mapping_square_root_cannot_change_output() {
    use crate::system_functions::field_ops::test_oracles::lying_oracle;
    use crypto::hints::FieldHintOp;
    let mut oracle = lying_oracle(&[FieldHintOp::Bls12381BaseFieldSqrt]);
    let mut encoded = [0u8; 64];
    crypto::bls12_381::eip2537::serialize_fq_bytes(Fq::from(7u64), &mut encoded);
    let mut resources = R::FORMAL_INFINITE;
    let _ = Bls12381G1MappingPrecompile::<true>::execute(
        &encoded,
        &mut Vec::new(),
        &mut resources,
        &mut oracle,
        &mut NullLogger,
        Global,
    );
}
