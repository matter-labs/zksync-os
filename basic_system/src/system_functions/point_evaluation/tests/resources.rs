use super::*;

fn charging_for<const USE_ADVICE: bool>() {
    let a = valid_a();
    type TestErgs = <TestResources as Resources>::Ergs;
    let short_of_gas = TestResources::from_ergs_and_native(
        TestErgs::from_legacy_gas(POINT_EVALUATION_COST_GAS - 1).unwrap(),
        DecreasingNative::FORMAL_INFINITE,
    );
    let short_of_native = TestResources::from_ergs_and_native(
        TestErgs::FORMAL_INFINITE,
        DecreasingNative::from_computational(POINT_EVALUATION_NATIVE_COST - 1),
    );
    let exact = TestResources::from_ergs_and_native(
        TestErgs::from_legacy_gas(POINT_EVALUATION_COST_GAS).unwrap(),
        DecreasingNative::from_computational(POINT_EVALUATION_NATIVE_COST),
    );
    // A failed charge must not populate the cache.
    let mut caches = PrecompileCaches::default();
    for resources in [short_of_gas.clone(), short_of_native.clone()] {
        let reference = uncached::<USE_ADVICE, _>(&a, resources.clone(), &mut honest_oracle());
        assert!(reference.0.is_err());
        let outcome = cached::<USE_ADVICE, _>(&a, resources, &mut honest_oracle(), &mut caches);
        assert_eq!(outcome, reference);
        assert!(!is_cached(&caches, &a));
    }
    // warm A, with exactly the resources the charge needs
    let reference = uncached::<USE_ADVICE, _>(&a, exact.clone(), &mut honest_oracle());
    assert_eq!(reference.0, Ok(()));
    assert_eq!((reference.2, reference.3), (0, 0));
    let warmed = cached::<USE_ADVICE, _>(&a, exact.clone(), &mut honest_oracle(), &mut caches);
    assert_eq!(warmed, reference);
    assert!(is_cached(&caches, &a));
    let hit = cached::<USE_ADVICE, _>(&a, exact, &mut Unqueried, &mut caches);
    assert_eq!(hit, reference);
    // (i) a failed charge on a cached input fails exactly as uncached, before the lookup
    for resources in [short_of_gas, short_of_native] {
        let reference = uncached::<USE_ADVICE, _>(&a, resources.clone(), &mut honest_oracle());
        let outcome = cached::<USE_ADVICE, _>(&a, resources, &mut Unqueried, &mut caches);
        assert_eq!(outcome, reference);
        assert!(is_cached(&caches, &a));
    }
}

#[test]
fn hits_and_misses_charge_identically() {
    charging_for::<true>();
    charging_for::<false>();
}

/// Calls the precompile with the output into a sink of `len` bytes: the result, the
/// backing bytes of the sink afterwards (all initialized up front) and its logical length
fn call_into_sink(
    input: &[u8],
    len: usize,
    caches: Option<&mut PrecompileCaches>,
) -> (
    Result<(), SubsystemError<PointEvaluationErrors>>,
    Vec<u8>,
    usize,
) {
    use core::mem::MaybeUninit;
    use zk_ee::memory::slice_vec::SliceVec;
    let mut memory = vec![MaybeUninit::new(0xa5u8); len];
    let mut sink = SliceVec::new(&mut memory);
    let mut resources = infinite_resources();
    let mut oracle = honest_oracle();
    let result = match caches {
        Some(caches) => PointEvaluationImpl::<true>::execute_with_precompile_caches(
            input,
            &mut sink,
            &mut resources,
            &mut oracle,
            caches,
            &mut NullLogger,
            Global,
        ),
        None => PointEvaluationImpl::<true>::execute(
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
    // SAFETY: every byte of the memory was initialized when it was created
    let bytes = memory
        .iter()
        .map(|byte| unsafe { byte.assume_init() })
        .collect();
    (result, bytes, logical_len)
}

#[test]
fn out_of_return_memory_same_on_hit_and_miss() {
    use zk_ee::system::errors::root_cause::{GetRootCause, RootCause};
    use zk_ee::system::errors::runtime::{FatalRuntimeError, RuntimeError};
    let a = valid_a();
    for len in [13, 31] {
        let reference = call_into_sink(&a, len, None);
        assert!(matches!(
            reference.0.as_ref().map_err(|error| error.root_cause()),
            Err(RootCause::Runtime(RuntimeError::FatalRuntimeError(
                FatalRuntimeError::OutOfReturnMemory(_)
            )))
        ));
        // the sink is written up to its end, and its length is left as it was
        assert_eq!(reference.1, POINT_EVAL_PRECOMPILE_SUCCESS_RESPONSE[..len]);
        assert_eq!(reference.2, 0);
        let mut caches = PrecompileCaches::default();
        let miss = call_into_sink(&a, len, Some(&mut caches));
        assert_eq!(miss, reference);
        // the input verified, whatever the sink
        assert!(is_cached(&caches, &a));
        let hit = call_into_sink(&a, len, Some(&mut caches));
        assert_eq!(hit, reference);
        let (result, bytes, logical_len) = call_into_sink(&a, 64, Some(&mut caches));
        assert_eq!(result, Ok(()));
        assert_eq!(bytes, POINT_EVAL_PRECOMPILE_SUCCESS_RESPONSE);
        assert_eq!(logical_len, 64);
    }
}
