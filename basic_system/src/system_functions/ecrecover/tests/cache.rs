use super::*;

fn cached_matches_uncached_for<const USE_ADVICE: bool>() {
    for (name, input) in vectors() {
        let reference = call::<USE_ADVICE, _>(
            &input,
            TestResources::FORMAL_INFINITE,
            &mut honest_oracle(),
            None,
        );
        let mut caches = PrecompileCaches::default();
        for repeat in 0..2 {
            let outcome = call::<USE_ADVICE, _>(
                &input,
                TestResources::FORMAL_INFINITE,
                &mut honest_oracle(),
                Some(&mut caches),
            );
            assert_eq!(outcome, reference, "{name}, call {repeat}");
        }
        assert_eq!(
            caches.ecrecover.get(&normalized(&input)).is_some(),
            name != "bad v",
            "{name}"
        );
    }
}

#[test]
fn cached_matches_uncached() {
    cached_matches_uncached_for::<false>();
    cached_matches_uncached_for::<true>();
}

#[test]
fn normalization_and_full_key() {
    let valid = valid_input();
    let mut caches = PrecompileCaches::default();
    let short = &valid[..126];
    let padded = [short, &[0, 0]].concat();
    let _ = run(short, Some(&mut caches));
    assert!(caches.ecrecover.get(&normalized(&padded)).is_some());
    let tape_hit = record_tape(&[&padded], Some(&mut caches)).0;
    assert!(tape_hit.is_empty());

    let mut caches = PrecompileCaches::default();
    let long = [valid.clone(), vec![0xff]].concat();
    let _ = run(&long, Some(&mut caches));
    assert!(caches.ecrecover.get(&normalized(&valid)).is_some());
    assert!(record_tape(&[&valid], Some(&mut caches)).0.is_empty());

    for index in [0, 32, 64, 96, 127] {
        let mut changed = valid.clone();
        changed[index] ^= 1;
        assert!(normalized(&valid) != normalized(&changed), "byte {index}");
        let reference = run(&changed, None);
        let actual = run(&changed, Some(&mut caches));
        assert_eq!(actual, reference, "byte {index}");
    }
}

#[test]
fn invalid_v_and_failed_recovery_cache_policy() {
    let mut caches = PrecompileCaches::default();
    let mut bad_v = valid_input();
    bad_v[63] = 29;
    let _ = run(&bad_v, Some(&mut caches));
    assert!(caches.ecrecover.get(&normalized(&bad_v)).is_none());
    for input in [no_sqrt_input(), infinity_input()] {
        let outcome = run(&input, Some(&mut caches));
        assert_eq!(outcome.1, Vec::<u8>::new());
        assert_eq!(caches.ecrecover.get(&normalized(&input)), Some(&None));
        assert_eq!(
            record_tape(&[&input], Some(&mut caches)).0,
            Vec::<u32>::new()
        );
    }
}
