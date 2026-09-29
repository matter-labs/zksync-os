use super::*;

#[test]
fn verifying_small_batch_proof() {
    let key_0 = WarmStorageKey {
        address: B160::ZERO,
        key: Bytes32::zero(),
    };
    let key_1 = WarmStorageKey {
        address: B160::default(),
        key: Bytes32::from_byte_fill(1),
    };
    let key_2 = WarmStorageKey {
        address: B160::default(),
        key: Bytes32::from_byte_fill(2),
    };
    let key_f = WarmStorageKey {
        address: B160::default(),
        key: Bytes32::from_byte_fill(0x0f),
    };

    let mut tree = TestingTree::new_in(Global);

    // Only missing reads
    test_verifying_batch_proof(&mut tree, &[(key_0, None), (key_1, None), (key_2, None)]);

    // Only inserts
    test_verifying_batch_proof(
        &mut tree,
        &[
            (key_0, Some(to_be_bytes(1))),
            (key_1, Some(to_be_bytes(2))),
            (key_2, Some(to_be_bytes(3))),
        ],
    );

    // Updates and reads
    test_verifying_batch_proof(
        &mut tree,
        &[
            (key_1, Some(to_be_bytes(123456))),
            (key_0, None), // existing read
            (key_2, Some(to_be_bytes(654321))),
            (key_f, None), // missing read
        ],
    );

    // Updates and inserts
    test_verifying_batch_proof(
        &mut tree,
        &[
            (key_1, Some(to_be_bytes(123456))),
            (key_f, Some(to_be_bytes(u64::MAX))),
            (key_0, Some(to_be_bytes(777))),
        ],
    );
}

proptest! {
    #[test]
    fn verifying_larger_batch_proof_for_empty_tree(entries in gen_entries()) {
        let mut tree = TestingTree::new_in(Global);
        test_verifying_batch_proof(&mut tree, &entries);
    }

    #[test]
    fn verifying_larger_batch_proof(
        prev_entries in gen_previous_entries(0..100),
        new_entries in gen_entries(),
    ) {
        let mut tree = TestingTree::new_in(Global);
        for (key, value) in &prev_entries {
            tree.insert(&derive_flat_storage_key(&key.address, &key.key), value);
        }

        test_verifying_batch_proof(&mut tree, &new_entries);
    }

    #[test]
    fn verifying_larger_batch_proof_with_updates(
        prev_entries in gen_previous_entries(1..100), // We need non-empty prev entries to select reads / updates
        new_entries in gen_entries(),
        reads_and_updates in gen_reads_and_updates(),
    ) {
        let mut tree = TestingTree::new_in(Global);
        for (key, value) in &prev_entries {
            tree.insert(&derive_flat_storage_key(&key.address, &key.key), value);
        }

        // Should be deduplicated to maintain the batch verification contract
        let reads_and_updates: HashMap<_, _> = reads_and_updates
            .into_iter()
            .map(|(idx, value)| {
                let &(key, _) = idx.get(&prev_entries);
                (key, value)
            })
            .collect();
        let mut all_entries = new_entries;
        all_entries.extend(reads_and_updates);
        test_verifying_batch_proof(&mut tree, &all_entries);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// Small trees: new keys share neighbours, several new writes fall between the same
    /// leaves, and existing writes hit leaves opened as neighbours.
    #[test]
    fn differential_small_trees(
        slots in gen_previous_entries(0..21),
        batch in gen_batch(0..=40),
    ) {
        let mut tree = tree_with_slots(&slots);
        let existing: Vec<_> = slots.iter().map(|(key, _)| *key).collect();
        let entries = batch_entries(&tree, &existing, &batch);
        check_batch_against_tree(&mut tree, &entries);
    }

    /// Next free slots at and around powers of two: new writes start subtrees with empty
    /// siblings and merge with the rightmost path.
    #[test]
    fn differential_next_free_slot_grid(
        saved in proptest::sample::select(vec![2usize, 3, 4, 5, 8, 9, 16, 17, 64, 65, 128]),
        slots in gen_previous_entries(126..127),
        batch in gen_batch(1..=30),
    ) {
        let slots = &slots[..saved - 2];
        let mut tree = tree_of(&slots.iter().map(|(key, _)| flat_key_of(key)).collect::<Vec<_>>());
        prop_assert_eq!(tree.next_free_slot, saved as u64);
        let existing: Vec<_> = slots.iter().map(|(key, _)| *key).collect();
        let entries = batch_entries(&tree, &existing, &batch);
        check_batch_against_tree(&mut tree, &entries);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Leaves at random positions: a sparse tree with deep unmerged paths and non-contiguous
    /// indices (against the reference only: the tree inserts at random positions too).
    #[test]
    fn differential_randomized_positions(
        slots in gen_previous_entries(0..60),
        batch in gen_batch(0..=40),
    ) {
        let mut leaves = Vec::new_in(Global);
        for (key, value) in &slots {
            leaves.push((flat_key_of(key), *value));
        }
        let tree = TestingTree::<true>::new_in_with_leaves(Global, leaves);
        let existing: Vec<_> = slots.iter().map(|(key, _)| *key).collect();
        let entries = batch_entries(&tree, &existing, &batch);
        diff_run(&tree, &entries, no_tamper).accepted();
    }
}

// Edge cases

#[test]
fn empty_batch() {
    let tree = tree_of(&[flat_key_of(&storage_key(1))]);
    let runs = diff_run(&tree, &[], no_tamper);
    assert_eq!(runs.accepted(), (*tree.root(), tree.next_free_slot));
    assert!(runs.reference.log.is_empty());
}

#[test]
fn only_existing_reads_and_writes() {
    let keys: Vec<_> = (0..8).map(storage_key).collect();
    let mut tree = tree_of(&keys.iter().map(flat_key_of).collect::<Vec<_>>());
    // reads: no relink, no rightmost proof, the root is unchanged
    let entries: Vec<_> = keys
        .iter()
        .map(|key| entry(&tree, *key, Access::Read))
        .collect();
    let runs = check_batch_against_tree(&mut tree, &entries);
    assert_eq!(runs.num_queries(PREV), 0);
    assert_eq!(runs.num_queries(PROOF), keys.len());
    // writes: no relink, no rightmost proof
    let entries: Vec<_> = keys
        .iter()
        .enumerate()
        .map(|(i, key)| entry(&tree, *key, Access::Write(to_be_bytes(100 + i as u64))))
        .collect();
    let runs = check_batch_against_tree(&mut tree, &entries);
    assert_eq!(runs.num_queries(PREV), 0);
    assert_eq!(runs.num_queries(PROOF), keys.len());
}

#[test]
fn only_new_reads() {
    let mut tree = tree_of(
        &(0..4)
            .map(|i| flat_key_of(&storage_key(i)))
            .collect::<Vec<_>>(),
    );
    let root = *tree.root();
    let entries: Vec<_> = (10..20)
        .map(|i| entry(&tree, storage_key(i), Access::Read))
        .collect();
    check_batch_against_tree(&mut tree, &entries);
    assert_eq!(*tree.root(), root);
}

#[test]
fn single_new_write_into_empty_tree() {
    let mut tree = TestingTree::new_in(Global);
    let entries = [entry(&tree, storage_key(1), Access::Write(to_be_bytes(1)))];
    check_batch_against_tree(&mut tree, &entries);
}

#[test]
fn many_new_writes_between_the_same_leaves() {
    let mut tree = TestingTree::new_in(Global);
    let entries: Vec<_> = (0..40)
        .map(|i| entry(&tree, storage_key(i), Access::Write(to_be_bytes(i + 1))))
        .collect();
    let runs = check_batch_against_tree(&mut tree, &entries);
    // the guards, and nothing else, as neighbours; the rightmost leaf is the end guard
    assert_eq!(runs.num_queries(PROOF), 2);
}

#[test]
fn honest_neighbours_share_prefixes_with_new_read_and_write() {
    let write = storage_key(1);
    let read = (2..)
        .map(storage_key)
        .find(|key| key_prefix(&flat_key_of(key)) != key_prefix(&flat_key_of(&write)))
        .unwrap();
    let mut neighbours = Vec::new();
    for key in [write, read] {
        let flat_key = flat_key_of(&key);
        for delta in [-1, 1] {
            let neighbour = offset(&flat_key, delta);
            assert_eq!(key_prefix(&neighbour), key_prefix(&flat_key));
            neighbours.push(neighbour);
        }
    }
    neighbours.sort();
    let mut tree = tree_of(&neighbours);
    let entries = [
        entry(&tree, read, Access::Read),
        entry(&tree, write, Access::Write(to_be_bytes(7))),
    ];
    let runs = check_batch_against_tree(&mut tree, &entries);
    assert!(runs.hardened.counts[Counter::OverflowKeyCmp as usize] > 0);
}

#[test]
fn new_writes_next_to_the_guards() {
    let z = storage_key(1);
    let flat_key = flat_key_of(&z);
    for tree_keys in [
        // right after the start guard
        vec![offset(&flat_key, 1), offset(&flat_key, 2)],
        // right before the end guard
        vec![offset(&flat_key, -2), offset(&flat_key, -1)],
    ] {
        for access in [Access::Read, Access::Write(to_be_bytes(3))] {
            let mut tree = tree_of(&tree_keys);
            let entries = [entry(&tree, z, access)];
            check_batch_against_tree(&mut tree, &entries);
        }
    }
}

/// Batches whose number of proofs, keys or items is at a chunk boundary.
#[test]
fn chunk_boundaries() {
    let keys: Vec<_> = (0..260).map(storage_key).collect();
    let base = tree_of(&keys.iter().map(flat_key_of).collect::<Vec<_>>());
    for n in [31, 32, 33, 255, 256, 257] {
        // existing reads: `n` items, proofs and keys
        let mut tree = base.clone();
        let entries: Vec<_> = keys[..n]
            .iter()
            .map(|key| entry(&tree, *key, Access::Read))
            .collect();
        let runs = check_batch_against_tree(&mut tree, &entries);
        assert_eq!(runs.num_queries(PROOF), n);
        // new writes into the empty tree: `n` keys with the guards
        if n > 32 {
            let mut tree = TestingTree::new_in(Global);
            let entries: Vec<_> = (1000..1000 + n as u64 - 2)
                .map(|i| entry(&tree, storage_key(i), Access::Write(to_be_bytes(i))))
                .collect();
            check_batch_against_tree(&mut tree, &entries);
        }
    }
}

/// A source without an exact size hint is counted first.
#[test]
fn source_without_exact_size() {
    let keys: Vec<_> = (0..40).map(storage_key).collect();
    let tree = tree_of(&keys[..20].iter().map(flat_key_of).collect::<Vec<_>>());
    let entries: Vec<_> = keys
        .iter()
        .enumerate()
        .map(|(i, key)| {
            let access = if i % 3 == 0 {
                Access::Read
            } else {
                Access::Write(to_be_bytes(i as u64 + 1))
            };
            entry(&tree, *key, access)
        })
        .collect();
    let expected = diff_run(&tree, &entries, no_tamper).accepted();
    let mut commitment = FlatStorageCommitment::<TESTING_TREE_HEIGHT> {
        root: *tree.root(),
        next_free_slot: tree.next_free_slot,
    };
    let mut oracle = TreeOracle::new(tree.clone(), None);
    let source = entries.iter().copied().filter(|_| true);
    assert_ne!(source.size_hint().1, Some(source.size_hint().0));
    commitment
        .verify_and_apply_batch(&mut oracle, source, Global, &mut NullLogger)
        .unwrap();
    assert_eq!((commitment.root, commitment.next_free_slot), expected);
}
