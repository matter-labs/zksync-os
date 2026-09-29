use super::*;

#[test]
fn charge_failure_never_uses_or_inserts_cache() {
    type TestErgs = <TestResources as Resources>::Ergs;
    let a = valid_input();
    let mut caches = PrecompileCaches::default();
    let low_gas = TestResources::from_ergs_and_native(
        TestErgs::from_legacy_gas(ECRECOVER_COST_GAS - 1).unwrap(),
        DecreasingNative::FORMAL_INFINITE,
    );
    let low_native = TestResources::from_ergs_and_native(
        TestErgs::FORMAL_INFINITE,
        DecreasingNative::from_computational(ECRECOVER_NATIVE_COST - 1),
    );
    for resources in [low_gas.clone(), low_native.clone()] {
        let expected = call::<true, _>(&a, resources.clone(), &mut honest_oracle(), None);
        assert!(expected.0.is_err());
        assert_eq!(
            call::<true, _>(&a, resources, &mut honest_oracle(), Some(&mut caches)),
            expected
        );
        assert!(caches.ecrecover.get(&normalized(&a)).is_none());
    }
    let _ = run(&a, Some(&mut caches));
    for resources in [low_gas, low_native] {
        let expected = call::<true, _>(&a, resources.clone(), &mut honest_oracle(), None);
        assert_eq!(
            call::<true, _>(&a, resources, &mut honest_oracle(), Some(&mut caches)),
            expected
        );
        assert!(caches.ecrecover.get(&normalized(&a)).is_some());
    }
}

fn call_into_sink(
    input: &[u8],
    len: usize,
    caches: Option<&mut PrecompileCaches>,
) -> (
    Result<(), SubsystemError<Secp256k1ECRecoverErrors>>,
    Vec<u8>,
    usize,
    u64,
    u64,
) {
    use core::mem::MaybeUninit;
    use std::alloc::Global;
    use zk_ee::memory::slice_vec::SliceVec;
    use zk_ee::system::logger::NullLogger;
    let mut backing = vec![MaybeUninit::new(0xa5u8); len];
    let mut sink = SliceVec::new(&mut backing);
    let mut resources = TestResources::FORMAL_INFINITE;
    let mut oracle = honest_oracle();
    let result = match caches {
        Some(caches) => EcRecoverImpl::<true>::execute_with_precompile_caches(
            input,
            &mut sink,
            &mut resources,
            &mut oracle,
            caches,
            &mut NullLogger,
            Global,
        ),
        None => EcRecoverImpl::<true>::execute(
            input,
            &mut sink,
            &mut resources,
            &mut oracle,
            &mut NullLogger,
            Global,
        ),
    };
    let logical_len = sink.len();
    drop(sink);
    // SAFETY: every backing byte was initialized before the call.
    let bytes = backing
        .iter()
        .map(|byte| unsafe { byte.assume_init() })
        .collect();
    (
        result,
        bytes,
        logical_len,
        resources.legacy_gas(),
        resources.native().as_u64(),
    )
}

#[test]
fn output_allocation_failure_is_identical_on_hits() {
    use zk_ee::system::errors::root_cause::{GetRootCause, RootCause};
    use zk_ee::system::errors::runtime::{FatalRuntimeError, RuntimeError};
    let input = valid_input();
    let expected_output = run(&input, None).1;
    for len in [13, 31] {
        let expected = call_into_sink(&input, len, None);
        assert!(matches!(
            expected.0.as_ref().map_err(|error| error.root_cause()),
            Err(RootCause::Runtime(RuntimeError::FatalRuntimeError(
                FatalRuntimeError::OutOfReturnMemory(_)
            )))
        ));
        assert_eq!(expected.1, expected_output[..len]);
        assert_eq!(expected.2, 0);
        let mut caches = PrecompileCaches::default();
        assert_eq!(call_into_sink(&input, len, Some(&mut caches)), expected);
        assert!(caches.ecrecover.get(&normalized(&input)).is_some());
        assert_eq!(call_into_sink(&input, len, Some(&mut caches)), expected);
        let success = call_into_sink(&input, 32, Some(&mut caches));
        assert_eq!(success.0, Ok(()));
        assert_eq!(success.1, expected_output);
        assert_eq!(success.2, 32);
    }
}
