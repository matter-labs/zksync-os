use super::*;

// The structures

/// Returns whether the component operation panicked.
pub(super) fn panics(f: impl FnOnce()) -> bool {
    catch_unwind(AssertUnwindSafe(f)).is_err()
}

/// Inserts keys into a `KeyIndex` and into a `BTreeMap`, and compares them: a key that is
/// already present must panic (the index is then rebuilt from the keys before), and at the
/// end the lengths, the lookups (also of `absent`) and the iteration must agree.
pub(super) fn check_key_index(keys: &[Bytes32], absent: &[Bytes32]) {
    let build = |keys: &[(Bytes32, KeyTarget)]| {
        let mut index = KeyIndex::with_max_len_in(keys.len() + 1, Global);
        for &(key, target) in keys {
            index.insert_new(key, target);
        }
        index
    };
    let mut inserted = Vec::new();
    let mut reference = BTreeMap::new();
    let mut index = build(&[]);
    for (i, &key) in keys.iter().enumerate() {
        let target = if i % 2 == 0 {
            KeyTarget::Opened(i)
        } else {
            KeyTarget::Inserted(i)
        };
        if reference.contains_key(&key) {
            assert!(
                panics(|| index.insert_new(key, target)),
                "duplicate {key:?}"
            );
            index = build(&inserted);
            continue;
        }
        index.insert_new(key, target);
        inserted.push((key, target));
        reference.insert(key, target);
        assert_eq!(index.len(), reference.len());
    }
    let index = build(&inserted);
    assert_eq!(index.len(), reference.len());
    for key in keys.iter().chain(absent) {
        assert_eq!(index.get(key), reference.get(key).copied(), "{key:?}");
    }
    let iterated: Vec<_> = index.iter().map(|(key, target)| (*key, target)).collect();
    let expected: Vec<_> = reference.into_iter().collect();
    assert_eq!(iterated, expected);
}

/// Keys under few prefixes, including 0 and `u64::MAX`, with the lowest and highest suffixes.
pub(super) fn prefix_group_keys() -> Vec<Bytes32> {
    let mut keys = vec![Bytes32::ZERO, Bytes32::MAX];
    for prefix in [0, 1, 0x8000_0000_0000_0000, u64::MAX - 1, u64::MAX] {
        keys.push(key_with_prefix(prefix, &[0; 24]));
        keys.push(key_with_prefix(prefix, &[0xff; 24]));
        for i in 1..4 {
            keys.push(key_with_prefix(prefix, &suffix_of(i)));
        }
    }
    keys.sort();
    keys.dedup();
    keys
}

#[test]
fn key_index_orders() {
    let keys = prefix_group_keys();
    let absent = [
        key_with_prefix(0, &suffix_of(9)),
        key_with_prefix(2, &[0; 24]),
        key_with_prefix(u64::MAX, &suffix_of(9)),
    ];
    // ascending, descending
    check_key_index(&keys, &absent);
    let descending: Vec<_> = keys[..].iter().rev().copied().collect();
    check_key_index(&descending, &absent);
    // interleaved across prefixes
    let (even, odd): (Vec<_>, Vec<_>) = keys[..].iter().enumerate().partition(|(i, _)| i % 2 == 0);
    let interleaved: Vec<_> = even.into_iter().chain(odd).map(|(_, key)| *key).collect();
    check_key_index(&interleaved, &absent);
    // a second key of a prefix, below or above the first, at every position
    let (first, second) = (
        key_with_prefix(7, &suffix_of(5)),
        key_with_prefix(7, &suffix_of(3)),
    );
    for position in 0..=keys.len() {
        for (a, b) in [(first, second), (second, first)] {
            let mut sequence = vec![a];
            sequence.extend_from_slice(&keys);
            sequence.insert(position + 1, b);
            check_key_index(&sequence, &absent);
        }
    }
}

#[test]
fn key_index_rejects_duplicates() {
    let one = key_with_prefix(5, &suffix_of(1));
    let many = key_with_prefix(5, &suffix_of(2));
    // under a `One` head
    check_key_index(&[one, one], &[]);
    // under a `Many` head
    check_key_index(&[one, many, one], &[]);
    check_key_index(&[one, many, many], &[]);
    check_key_index(
        &[Bytes32::ZERO, Bytes32::MAX, Bytes32::ZERO, Bytes32::MAX],
        &[],
    );
}

pub(super) fn gen_grouped_key() -> impl Strategy<Value = Bytes32> {
    let prefix = proptest::sample::select(vec![0, 1, 2, u64::MAX - 1, u64::MAX]);
    let suffix = prop_oneof![
        Just([0u8; 24]),
        Just([0xffu8; 24]),
        (0u64..8).prop_map(suffix_of),
        proptest::array::uniform24(proptest::num::u8::ANY),
    ];
    prop_oneof![
        8 => (prefix, suffix).prop_map(|(prefix, suffix)| key_with_prefix(prefix, &suffix)),
        1 => uniform_bytes(),
    ]
}

proptest! {
    #[test]
    fn key_index_matches_btree_map(
        keys in proptest::collection::vec(gen_grouped_key(), 0..80),
        absent in proptest::collection::vec(gen_grouped_key(), 0..20),
    ) {
        check_key_index(&keys, &absent);
    }

    #[test]
    fn chunked_vec_matches_vec(
        values in proptest::collection::vec(any::<u64>(), 0..40),
        extra in 0usize..6,
    ) {
        let max_len = values.len() + extra;
        let mut chunked = ChunkedVec::<u64, 4, Global>::with_max_len_in(max_len, Global);
        for (i, &value) in values.iter().enumerate() {
            if i % 2 == 0 {
                chunked.push(value);
            } else {
                chunked.next_slot().write(value);
                // SAFETY: just initialized
                unsafe { chunked.commit_next() };
            }
            prop_assert_eq!(chunked.len(), i + 1);
        }
        for (i, value) in values.iter().enumerate() {
            prop_assert_eq!(chunked.get(i), value);
            let incremented = chunked.get(i).wrapping_add(1);
            *chunked.get_mut(i) = incremented;
        }
        let incremented: Vec<_> = values.iter().map(|value| value.wrapping_add(1)).collect();
        prop_assert_eq!(chunked.iter().copied().collect::<Vec<_>>(), incremented);
        prop_assert_eq!(chunked.chunks.capacity(), max_len.div_ceil(4));
        // an uncommitted slot is not an element
        let uncommitted = panics(|| {
            chunked.get(values.len());
        });
        prop_assert!(uncommitted);
        // filling up to the bound, and past it
        for _ in values.len()..max_len {
            chunked.push(0);
        }
        let past_the_bound = panics(|| chunked.push(0));
        // within the last chunk's room, a push does not need a new chunk
        prop_assert_eq!(past_the_bound, max_len % 4 == 0);
    }

    #[test]
    fn key_prefix_is_the_big_endian_start(key in uniform_bytes()) {
        let bytes = key.as_u8_array_ref();
        let expected = u64::from_be_bytes(bytes[..8].try_into().unwrap());
        prop_assert_eq!(key_prefix(&key), expected);
        let first = u32::from_le_bytes(bytes[..4].try_into().unwrap());
        let second = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        prop_assert_eq!(prefix_of_words(first, second), expected);
    }
}

#[test]
fn push_within_capacity_does_not_grow() {
    let mut vec = Vec::with_capacity_in(2, Global);
    push_within_capacity(&mut vec, 1);
    push_within_capacity(&mut vec, 2);
    assert!(panics(|| push_within_capacity(&mut vec, 3)));
    assert_eq!(vec.capacity(), 2);
}
