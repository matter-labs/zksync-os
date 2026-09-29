use super::*;

fn recording_oracle() -> oracle_provider::ReadWitnessSource {
    let mut oracle = oracle_provider::ZkEENonDeterminismSource::new(
        oracle_provider::RunMode::NativeRunSavingForRiscV,
    );
    oracle.add_external_processor(callable_oracles::field_hints::NativeFieldOpsQuery);
    oracle_provider::ReadWitnessSource::new(oracle)
}

pub(super) fn record_tape(
    inputs: &[&[u8]],
    mut caches: Option<&mut PrecompileCaches>,
) -> (Vec<u32>, Vec<Outcome>) {
    let mut oracle = recording_oracle();
    let items = oracle.get_read_items();
    let outcomes = inputs
        .iter()
        .map(|input| {
            call::<true, _>(
                input,
                TestResources::FORMAL_INFINITE,
                &mut oracle,
                caches.as_deref_mut(),
            )
        })
        .collect();
    let tape = items.borrow().clone();
    (tape, outcomes)
}

#[test]
fn hits_skip_queries_and_eviction_requeries() {
    let a = valid_input();
    let mut b = a.clone();
    b[0] ^= 1;
    let failed = no_sqrt_input();
    let infinity = infinity_input();
    let repeated = [
        &a[..],
        &b[..],
        &a,
        &b,
        &failed,
        &failed,
        &infinity,
        &infinity,
    ];
    let (cached_tape, outcomes) = record_tape(&repeated, Some(&mut PrecompileCaches::default()));
    let (expected_tape, _) = record_tape(&[&a, &b, &failed, &infinity], None);
    assert!(!expected_tape.is_empty());
    assert_eq!(cached_tape, expected_tape);
    assert_eq!(outcomes, record_tape(&repeated, None).1);

    let distinct = (0..64)
        .map(|index| {
            let mut input = a.clone();
            input[0] = index + 0x80;
            input
        })
        .collect::<Vec<_>>();
    let mut sequence = vec![&a[..]];
    sequence.extend(distinct.iter().map(Vec::as_slice));
    sequence.push(&a);
    let cached = record_tape(&sequence, Some(&mut PrecompileCaches::default()));
    let uncached = record_tape(&sequence, None);
    assert_eq!(cached, uncached);
    assert_eq!(
        cached,
        record_tape(&sequence, Some(&mut PrecompileCaches::default()))
    );
}

#[test]
fn intrinsic_execute_does_not_read_precompile_cache() {
    use std::alloc::Global;
    use zk_ee::system::logger::NullLogger;
    let input = valid_input();
    let mut caches = PrecompileCaches::default();
    let _ = run(&input, Some(&mut caches));
    let (expected_tape, expected) = record_tape(&[&input], None);
    assert!(!expected_tape.is_empty());
    let mut oracle = recording_oracle();
    let items = oracle.get_read_items();
    let mut output = Vec::new();
    let mut resources = TestResources::FORMAL_INFINITE;
    let result = <NoStdSystemFunctions<true> as zk_ee::system::SystemFunctionsExt<
        TestResources,
    >>::secp256k1_ec_recover(
        &input,
        &mut output,
        &mut resources,
        &mut oracle,
        &mut NullLogger,
        Global,
    );
    let actual = (
        result,
        output,
        resources.legacy_gas(),
        resources.native().as_u64(),
    );
    assert_eq!(actual, expected[0]);
    assert_eq!(*items.borrow(), expected_tape);
    assert!(caches.ecrecover.get(&normalized(&input)).is_some());
}

#[test]
fn oracle_error_does_not_enter_cache() {
    struct FailingOracle;

    impl zk_ee::oracle::memory_io::MemoryOracle for FailingOracle {
        fn send_query(&mut self, _query_id: u32, _input_word: usize) -> Result<(), InternalError> {
            Err(zk_ee::internal_error!("deliberate oracle failure"))
        }
    }

    impl IOOracle for FailingOracle {
        type RawIterator<'a> = core::iter::Empty<usize>;

        fn raw_query<'a, I: UsizeSerializable + UsizeDeserializable>(
            &'a mut self,
            _query_type: u32,
            _input: &I,
        ) -> Result<Self::RawIterator<'a>, InternalError> {
            Err(zk_ee::internal_error!("deliberate oracle failure"))
        }
    }

    let input = valid_input();
    let mut caches = PrecompileCaches::default();
    let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        call::<true, _>(
            &input,
            TestResources::FORMAL_INFINITE,
            &mut FailingOracle,
            Some(&mut caches),
        )
    }));
    assert!(failure.is_err());
    assert!(caches.ecrecover.get(&normalized(&input)).is_none());
}
