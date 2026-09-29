use super::*;

#[test]
fn cached_and_uncached_agree_in_both_modes() {
    for (name, input, expected) in vectors() {
        let reference = uncached::<true, _>(&input, infinite_resources(), &mut honest_oracle());
        assert_eq!(
            reference,
            uncached::<false, _>(&input, infinite_resources(), &mut Unqueried),
            "{name}"
        );
        match (&reference.0, &expected) {
            (Ok(()), None) => assert_eq!(
                reference.1, POINT_EVAL_PRECOMPILE_SUCCESS_RESPONSE,
                "{name}"
            ),
            (Err(SubsystemError::LeafUsage(error)), Some(expected)) => {
                assert_eq!(&error.0, expected, "{name}")
            }
            (result, _) => panic!("{name}: unexpected {result:?}"),
        }
        let (mut hinted, mut unhinted) = (PrecompileCaches::default(), PrecompileCaches::default());
        // A miss followed by a hit for valid inputs; invalid inputs must keep missing.
        for _ in 0..2 {
            let result = if is_cached(&hinted, &input) {
                cached::<true, _>(&input, infinite_resources(), &mut Unqueried, &mut hinted)
            } else {
                cached::<true, _>(
                    &input,
                    infinite_resources(),
                    &mut honest_oracle(),
                    &mut hinted,
                )
            };
            assert_eq!(result, reference, "{name}");
            assert_eq!(
                cached::<false, _>(&input, infinite_resources(), &mut Unqueried, &mut unhinted),
                reference,
                "{name}"
            );
            assert_eq!(is_cached(&hinted, &input), expected.is_none(), "{name}");
            assert_eq!(is_cached(&unhinted, &input), expected.is_none(), "{name}");
        }
    }
}

fn changed_inputs_miss_for<const USE_ADVICE: bool>() {
    let a = valid_a();
    let mut caches = PrecompileCaches::default();
    assert_eq!(
        cached::<USE_ADVICE, _>(&a, infinite_resources(), &mut honest_oracle(), &mut caches).0,
        Ok(())
    );
    let mut variants = vec![a[..191].to_vec(), [a.clone(), vec![0]].concat()];
    // Mutations at both ends of every field must miss, including the proof's final bytes.
    for (start, end) in [(0, 32), (32, 64), (64, 96), (96, 144), (144, 192)] {
        for (byte, bit) in [
            (start, 0x01),
            (start + 1, 0x80),
            (end - 1, 0x01),
            (end - 2, 0x40),
        ] {
            let mut input = a.clone();
            input[byte] ^= bit;
            variants.push(input);
        }
    }
    for input in variants {
        let reference =
            uncached::<USE_ADVICE, _>(&input, infinite_resources(), &mut honest_oracle());
        assert!(reference.0.is_err(), "{input:02x?}");
        assert_eq!(
            cached::<USE_ADVICE, _>(
                &input,
                infinite_resources(),
                &mut honest_oracle(),
                &mut caches
            ),
            reference
        );
        assert!(!is_cached(&caches, &input));
        assert!(
            is_cached(&caches, &a),
            "a failed verification must not evict the successful input"
        );
    }
}

#[test]
fn changed_inputs_miss_without_evicting() {
    changed_inputs_miss_for::<true>();
    changed_inputs_miss_for::<false>();
}
