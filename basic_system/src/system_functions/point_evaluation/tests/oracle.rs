use super::*;

/// Records the prover input as in the host prover-input run.
fn recording_oracle() -> oracle_provider::ReadWitnessSource {
    let mut oracle =
        ZkEENonDeterminismSource::new(oracle_provider::RunMode::NativeRunSavingForRiscV);
    oracle.add_external_processor(NativeFieldOpsQuery);
    oracle_provider::ReadWitnessSource::new(oracle)
}

/// The words the host prover-input run records for the RISC-V guest over the calls of
/// `inputs` (through the caches when `caches` is given), and the outcomes of the calls
fn record_tape(
    inputs: &[&[u8]],
    mut caches: Option<&mut PrecompileCaches>,
) -> (Vec<u32>, Vec<Outcome>) {
    let mut oracle = recording_oracle();
    let read_items = oracle.get_read_items();
    let outcomes = inputs
        .iter()
        .map(|input| match caches.as_deref_mut() {
            Some(caches) => cached::<true, _>(input, infinite_resources(), &mut oracle, caches),
            None => uncached::<true, _>(input, infinite_resources(), &mut oracle),
        })
        .collect();
    let tape = read_items.borrow().clone();
    (tape, outcomes)
}

/// The tape of cached calls, and a check of their outcomes against uncached calls
fn cached_tape(inputs: &[&[u8]]) -> Vec<u32> {
    let (tape, outcomes) = record_tape(inputs, Some(&mut PrecompileCaches::default()));
    let (_, reference) = record_tape(inputs, None);
    assert_eq!(outcomes, reference);
    tape
}

#[test]
fn hits_issue_no_queries() {
    let (a, b, c, d) = (valid_a(), valid_b(), pairing_mismatch(), valid_degenerate());
    let (a, b, c, d) = (&a[..], &b[..], &c[..], &d[..]);
    let uncached_tape = |inputs: &[&[u8]]| record_tape(inputs, None).0;
    // a hit issues nothing
    let tape_a = uncached_tape(&[a]);
    assert!(!tape_a.is_empty());
    assert_eq!(cached_tape(&[a, a, a]), tape_a);
    assert_eq!(cached_tape(&[d, d]), uncached_tape(&[d]));
    // one entry: each valid input evicts the other, so every call is a miss
    assert_eq!(cached_tape(&[a, b, a, b]), uncached_tape(&[a, b, a, b]));
    assert_eq!(cached_tape(&[a, d, a]), uncached_tape(&[a, d, a]));
    // a failure is not cached, and does not evict
    assert_eq!(cached_tape(&[a, c, a]), uncached_tape(&[a, c]));
    assert_eq!(cached_tape(&[c, c]), uncached_tape(&[c, c]));
    assert_eq!(cached_tape(&[d, c, d]), uncached_tape(&[d, c]));
    // deterministic
    let sequence = [a, c, a, b, d, d, a];
    assert_eq!(cached_tape(&sequence), cached_tape(&sequence));
}

#[test]
fn exact_final_exponentiation_success_is_cached() {
    let a = valid_a();
    let reference = uncached::<true, _>(&a, infinite_resources(), &mut honest_oracle());
    let mut caches = PrecompileCaches::default();
    let claimed = cached::<true, _>(
        &a,
        infinite_resources(),
        &mut curve_hints::tests::non_identity_claiming_oracle(),
        &mut caches,
    );
    assert_eq!(claimed, reference);
    assert!(is_cached(&caches, &a));
    assert_eq!(
        cached::<true, _>(&a, infinite_resources(), &mut Unqueried, &mut caches),
        reference
    );
}
