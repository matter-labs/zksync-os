use super::*;

// Named malicious cases: rejection is asserted for the reference and for the
// hardened implementation (`diff_run` checks the one without hardening against the reference).

/// (a) The exact index of an existing key is the index of another key.
#[test]
fn malicious_exact_index_of_another_key() {
    let (x, y) = (storage_key(1), storage_key(2));
    let tree = tree_of(&[flat_key_of(&x), flat_key_of(&y)]);
    let y_index = tree.get_index_for_existing(&flat_key_of(&y));
    let answer_y = tamper(move |_, id, _, response| {
        if id == EXACT {
            *response = u64_words(y_index);
        }
    });
    let entries = [entry(&tree, x, Access::Read)];
    diff_run(&tree, &entries, &answer_y).assert_rejected();
    let entries = [entry(&tree, x, Access::Write(to_be_bytes(7)))];
    diff_run(&tree, &entries, &answer_y).assert_rejected();
}

/// (c) The previous index of a new key: a leaf that is not adjacent to the key, a leaf above
/// it, and the next free slot.
#[test]
fn malicious_previous_index() {
    let z = storage_key(1);
    let flat_key = flat_key_of(&z);
    // at 2, 3 and 4
    let tree = tree_of(&[
        offset(&flat_key, -2),
        offset(&flat_key, -1),
        offset(&flat_key, 1),
    ]);
    let saved = tree.next_free_slot;
    for access in [Access::Read, Access::Write(to_be_bytes(7))] {
        let entries = [entry(&tree, z, access)];
        for answer in [2, 4, saved] {
            let runs = diff_run(
                &tree,
                &entries,
                tamper(move |_, id, _, response| {
                    if id == PREV {
                        *response = u64_words(answer);
                    }
                }),
            );
            runs.assert_rejected();
        }
    }
}

/// (d) A proof for another index than the one asked for.
#[test]
fn malicious_proven_index() {
    let x = storage_key(1);
    let tree = tree_of(&[flat_key_of(&x)]);
    let entries = [entry(&tree, x, Access::Read)];
    let runs = diff_run(
        &tree,
        &entries,
        tamper(|_, id, _, response| {
            if id == PROOF {
                response[0] ^= 1;
            }
        }),
    );
    runs.assert_rejected();
}

/// (e) A corrupted sibling: rejected at the root if the fold uses it, and accepted, with the
/// honest root, if it does not.
#[test]
fn malicious_sibling() {
    let (x, y) = (storage_key(1), storage_key(2));
    // siblings at 2 and 3
    let tree = tree_of(&[flat_key_of(&x), flat_key_of(&y)]);
    // the path of the left leaf: merged nodes go up with the path of their left origin
    let flip_level = |level: usize| {
        tamper(move |_, id, input, response| {
            if id == PROOF && *input == Input::Index(2) {
                response[PROOF_PATH + 8 * level] ^= 1;
            }
        })
    };
    let entries = [entry(&tree, x, Access::Read), entry(&tree, y, Access::Read)];
    diff_run(&tree, &entries, flip_level(63)).assert_rejected();
    // the fold hashes the two leaves together, not with the siblings of level 0
    let runs = diff_run(&tree, &entries, flip_level(0));
    assert_eq!(runs.accepted(), (*tree.root(), tree.next_free_slot));
    let entries = [
        entry(&tree, x, Access::Read),
        entry(&tree, y, Access::Write(to_be_bytes(9))),
    ];
    let honest = diff_run(&tree, &entries, no_tamper).accepted();
    assert_eq!(diff_run(&tree, &entries, flip_level(0)).accepted(), honest);
}

/// (f) A corrupted leaf value.
#[test]
fn malicious_leaf_value() {
    let x = storage_key(1);
    let tree = tree_of(&[flat_key_of(&x)]);
    let flip_value = tamper(|_, id, _, response| {
        if id == PROOF {
            response[PROOF_VALUE] ^= 1;
        }
    });
    let entries = [entry(&tree, x, Access::Read)];
    diff_run(&tree, &entries, &flip_value).assert_rejected();
    let entries = [entry(&tree, x, Access::Write(to_be_bytes(7)))];
    diff_run(&tree, &entries, &flip_value).assert_rejected();
}

/// (g) Two keys are given the same index: the second finds the leaf of the first.
#[test]
fn malicious_same_index_for_two_keys() {
    let (x, y) = (storage_key(1), storage_key(2));
    let tree = tree_of(&[flat_key_of(&x), flat_key_of(&y)]);
    let x_index = tree.get_index_for_existing(&flat_key_of(&x));
    let flat_key_y = flat_key_of(&y);
    let entries = [entry(&tree, x, Access::Read), entry(&tree, y, Access::Read)];
    let runs = diff_run(
        &tree,
        &entries,
        tamper(move |_, id, input, response| {
            if id == EXACT && *input == Input::Key(flat_key_y) {
                *response = u64_words(x_index);
            }
        }),
    );
    runs.assert_rejected();
    assert_eq!(runs.num_queries(PROOF), 1);
}

/// (h) An existing write that is not opened yet is answered with the index of a leaf opened as
/// a neighbour, and a forged leaf with the written key and initial value: the index is
/// rejected as opened twice.
#[test]
fn malicious_write_at_opened_neighbour() {
    let (w, z) = (storage_key(1), storage_key(2));
    let flat_key_z = flat_key_of(&z);
    let tree = tree_of(&[
        flat_key_of(&w),
        offset(&flat_key_z, -1),
        offset(&flat_key_z, 1),
    ]);
    let neighbour = tree.get_index_for_existing(&offset(&flat_key_z, -1));
    let entries = [
        entry(&tree, z, Access::Read),
        entry(&tree, w, Access::Write(to_be_bytes(7))),
    ];
    let forged = FlatStorageLeaf {
        key: flat_key_of(&w),
        value: entries[1].1.initial_value,
        next: 1,
    };
    // the reads pass asks for the previous index and two proofs, then the write for its index
    // and its proof
    let runs = diff_run(
        &tree,
        &entries,
        tamper(move |n, id, _, response| match (n, id) {
            (3, EXACT) => *response = u64_words(neighbour),
            (4, PROOF) => forge_leaf(response, &forged),
            _ => {}
        }),
    );
    assert_eq!(runs.reference.log[4].1, Input::Index(neighbour));
    runs.assert_rejected();
}

/// (i) A forged neighbour leaf whose key is the key of a leaf opened before.
#[test]
fn malicious_neighbour_key_opened_before() {
    let (mut z1, mut z2) = (storage_key(1), storage_key(2));
    if flat_key_of(&z2) > flat_key_of(&z1) {
        core::mem::swap(&mut z1, &mut z2);
    }
    let (f1, f2) = (flat_key_of(&z1), flat_key_of(&z2));
    let tree = tree_of(&[
        offset(&f1, -1),
        offset(&f1, 1),
        offset(&f2, -1),
        offset(&f2, 1),
    ]);
    let next_of_z2 = tree.get_index_for_existing(&offset(&f2, 1));
    let entries = [
        entry(&tree, z1, Access::Read),
        entry(&tree, z2, Access::Read),
    ];
    let runs = diff_run(
        &tree,
        &entries,
        tamper(move |_, id, input, response| {
            if id == PROOF && *input == Input::Index(next_of_z2) {
                let mut leaf = leaf_of_response(response);
                // the next leaf of `z1`, above `z2`
                leaf.key = offset(&f1, 1);
                forge_leaf(response, &leaf);
            }
        }),
    );
    runs.assert_rejected();
}

/// (j) A forged proof of the rightmost leaf.
#[test]
fn malicious_rightmost_proof() {
    let z = storage_key(1);
    let flat_key = flat_key_of(&z);
    let tree = tree_of(&[
        offset(&flat_key, -1),
        offset(&flat_key, 1),
        offset(&Bytes32::MAX, -1),
    ]);
    let rightmost = tree.next_free_slot - 1;
    let entries = [entry(&tree, z, Access::Write(to_be_bytes(7)))];
    // the rightmost leaf merges with the new write at level 0, and goes up alone at level 1
    for word in [PROOF_VALUE, PROOF_NEXT, PROOF_PATH + 8] {
        let runs = diff_run(
            &tree,
            &entries,
            tamper(move |_, id, input, response| {
                if id == PROOF && *input == Input::Index(rightmost) {
                    response[word] ^= 1;
                }
            }),
        );
        assert_eq!(
            runs.reference.log.last().unwrap().1,
            Input::Index(rightmost)
        );
        runs.assert_rejected();
    }
}

/// (k) The previous index of a new key is the start guard, with a forged non-zero key below the
/// new key: rejected by the start guard check of the relink, not by the root.
#[test]
fn malicious_start_guard_key() {
    let z = storage_key(1);
    let flat_key = flat_key_of(&z);
    let tree = tree_of(&[offset(&flat_key, -1), offset(&flat_key, 1)]);
    let next = tree.get_index_for_existing(&offset(&flat_key, 1));
    let forged = FlatStorageLeaf {
        key: offset(&flat_key, -5),
        value: Bytes32::ZERO,
        next,
    };
    let entries = [entry(&tree, z, Access::Read)];
    let runs = diff_run(
        &tree,
        &entries,
        tamper(move |_, id, input, response| match (id, input) {
            (PREV, _) => *response = u64_words(MIN_KEY_LEAF_MARKER_IDX),
            (PROOF, Input::Index(MIN_KEY_LEAF_MARKER_IDX)) => forge_leaf(response, &forged),
            _ => {}
        }),
    );
    runs.assert_rejected();
}

/// (n) Forged neighbour keys that share two 8-byte prefixes: every key of the batch is in the
/// overflow map. The implementations agree, and the comparisons of overflow keys grow as
/// n log n, not quadratically.
#[test]
fn malicious_shared_prefix_neighbours() {
    let comparisons = |num_reads: u64| {
        let reads: Vec<_> = (0..num_reads).map(storage_key).collect();
        let tree_keys: Vec<_> = (0..2 * num_reads)
            .map(|i| flat_key_of(&storage_key(1_000_000 + i)))
            .collect();
        let tree = tree_of(&tree_keys);
        let low = |i: u64| key_with_prefix(1, &suffix_of(i));
        let high = |i: u64| key_with_prefix(u64::MAX - 1, &suffix_of(i));
        let read_of: std::rc::Rc<HashMap<Bytes32, u64>> = std::rc::Rc::new(
            reads
                .iter()
                .enumerate()
                .map(|(i, key)| {
                    let flat_key = flat_key_of(key);
                    assert!(low(i as u64) < flat_key && flat_key < high(i as u64));
                    (flat_key, i as u64)
                })
                .collect(),
        );
        let entries: Vec<_> = reads
            .iter()
            .map(|key| entry(&tree, *key, Access::Read))
            .collect();
        let runs = diff_run(
            &tree,
            &entries,
            tamper(move |_, id, input, response| match (id, *input) {
                (PREV, Input::Key(key)) => *response = u64_words(2 + 2 * read_of[&key]),
                (PROOF, Input::Index(index)) => {
                    let i = (index - 2) / 2;
                    let leaf = if index % 2 == 0 {
                        FlatStorageLeaf {
                            key: low(i),
                            value: Bytes32::ZERO,
                            next: index + 1,
                        }
                    } else {
                        FlatStorageLeaf {
                            key: high(i),
                            value: Bytes32::ZERO,
                            next: 1,
                        }
                    };
                    forge_leaf(response, &leaf);
                }
                _ => {}
            }),
        );
        runs.assert_rejected();
        runs.hardened.counts[Counter::OverflowKeyCmp as usize]
    };
    let (small, large) = (comparisons(128), comparisons(256));
    // n keys in overflow: at most ~n log2(n) comparisons per B-tree operation bound
    let n = 512u64;
    assert!(large <= 16 * n * n.ilog2() as u64, "{large} comparisons");
    assert!(
        large * 10 < small * 32,
        "{small} comparisons for 256 keys, {large} for 512"
    );
}

/// The checks of the source that do not depend on the oracle.
#[test]
fn inconsistent_source() {
    let x = storage_key(1);
    let tree = tree_of(&[flat_key_of(&x)]);
    let mut read = entry(&tree, x, Access::Read);
    read.1.initial_value_used = false;
    diff_run(&tree, &[read], no_tamper).assert_rejected();
    let mut new_read = entry(&tree, storage_key(2), Access::Read);
    new_read.1.initial_value = to_be_bytes(1);
    new_read.1.current_value = to_be_bytes(1);
    diff_run(&tree, &[new_read], no_tamper).assert_rejected();
    // the same slot twice
    let write = entry(&tree, x, Access::Write(to_be_bytes(5)));
    diff_run(&tree, &[read, write], no_tamper).assert_rejected();
    let read = entry(&tree, x, Access::Read);
    let mut other_initial = write;
    other_initial.1.initial_value = to_be_bytes(99);
    diff_run(&tree, &[read, other_initial], no_tamper).assert_rejected();
    // a read and a write of the same value: the write finds the leaf of the read
    diff_run(&tree, &[read, write], no_tamper).accepted();
    diff_run(&tree, &[write, write], no_tamper).assert_rejected();
    let new_write = entry(&tree, storage_key(2), Access::Write(to_be_bytes(5)));
    diff_run(&tree, &[new_write, new_write], no_tamper).assert_rejected();
}

#[test]
fn malformed_source_can_receive_scripted_exact_index() {
    let existing = storage_key(1);
    let absent = storage_key(2);
    let tree = tree_of(&[flat_key_of(&existing)]);
    let mut malformed = entry(&tree, absent, Access::Read);
    malformed.1.is_new_storage_slot = false;
    let index = tree.get_index_for_existing(&flat_key_of(&existing));
    let runs = diff_run_scripted(
        &tree,
        &[malformed],
        no_tamper,
        script(move |_, id, input| {
            (id == EXACT && *input == Input::Key(flat_key_of(&absent))).then(|| u64_words(index))
        }),
    );
    runs.assert_rejected();
    assert_eq!(runs.reference.log.len(), 2);
    assert_eq!(runs.reference.log[0].1, Input::Key(flat_key_of(&absent)));
}
