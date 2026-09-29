use super::*;

/// (b) The exact index of an existing key is at or above the next free slot, where leaves are
/// empty. The existing-index check rejects the answer; the reference opens the empty leaf, and rejects it.
#[test]
fn existing_index_check_rejects_exact_index_at_next_free_slot() {
    let x = storage_key(1);
    let tree = tree_of(&[flat_key_of(&x)]);
    let saved = tree.next_free_slot;
    for answer in [saved, saved + 1, u64::MAX] {
        for access in [Access::Read, Access::Write(to_be_bytes(7))] {
            let entries = [entry(&tree, x, access)];
            let runs = diff_run(
                &tree,
                &entries,
                tamper(move |_, id, _, response| {
                    if id == EXACT {
                        *response = u64_words(answer);
                    }
                }),
            );
            runs.assert_rejected();
            runs.assert_rejected_before_proof(answer);
            assert_eq!(runs.hardened.log.len(), 1);
            assert_eq!(runs.reference.log.len(), 2);
            assert_eq!(runs.reference.log[1].1, Input::Index(answer));
        }
    }
}

/// The tree of the alias cases: one leaf `L` at index 2, so the saved next free slot is 3;
/// `L`, and new write keys above it, sorted.
pub(super) fn alias_fixture(
    num_writes: usize,
) -> (TestingTree<false>, WarmStorageKey, Vec<WarmStorageKey>) {
    let leaf = storage_key(1);
    let tree = tree_of(&[flat_key_of(&leaf)]);
    let mut writes: Vec<_> = (2..)
        .map(storage_key)
        .filter(|key| flat_key_of(key) > flat_key_of(&leaf))
        .take(num_writes)
        .collect();
    writes.sort_by_key(flat_key_of);
    (tree, leaf, writes)
}

/// The alias tamper: the proof of `L` links it to the saved next free slot, whose proof is a
/// forged leaf with `key`. The new writes, whose predecessor `L` is opened, then fetch that
/// successor, and the first of them is appended at the same index.
pub(super) fn alias_tamper(key: Bytes32) -> impl Fn() -> Option<Tamper> {
    tamper(move |_, id, input, response| match (id, input) {
        (PROOF, Input::Index(2)) => response[PROOF_NEXT..PROOF_PATH].copy_from_slice(&u64_words(3)),
        (PROOF, Input::Index(3)) => forge_leaf(
            response,
            &FlatStorageLeaf {
                key,
                value: Bytes32::ZERO,
                next: 1,
            },
        ),
        _ => {}
    })
}

/// (l) The new write aliases a forged successor at the saved next free slot: without
/// hardening, the relink resolves the new write to the opened successor, and the fold takes
/// the successor's path for it, before the fold rejects the duplicate index. The cached-successor check
/// rejects the link of the opened predecessor to the saved next free slot first.
#[test]
fn cached_successor_check_rejects_new_write_aliasing_opened_leaf() {
    let (tree, leaf, writes) = alias_fixture(1);
    let entries = [
        entry(&tree, leaf, Access::Read),
        entry(&tree, writes[0], Access::Write(to_be_bytes(7))),
    ];
    let runs = diff_run(
        &tree,
        &entries,
        alias_tamper(offset(&flat_key_of(&writes[0]), 1)),
    );
    runs.assert_rejected();
    runs.assert_rejected_before_proof(tree.next_free_slot);
    assert!(runs
        .reference
        .log
        .iter()
        .any(|(id, input, _)| *id == PROOF && *input == Input::Index(3)));
    let counts = runs.unhardened.counts;
    assert!(counts[Counter::NewWriteAliasResolved as usize] > 0);
    assert!(counts[Counter::NewWriteAliasPath as usize] > 0);
    let counts = runs.hardened.counts;
    assert_eq!(counts[Counter::NewWriteAliasResolved as usize], 0);
    assert_eq!(counts[Counter::NewWriteAliasPath as usize], 0);
}

/// (m) A new write key can never be first or last in key order: its neighbours are opened. So
/// the aliased case of the first and last leaves is an opened leaf at or above the saved next
/// free slot: here the forged successor is last, after two new writes, the first of which
/// aliases it.
#[test]
fn cached_successor_check_rejects_aliased_last_leaf() {
    let (tree, leaf, writes) = alias_fixture(2);
    let entries = [
        entry(&tree, leaf, Access::Read),
        entry(&tree, writes[0], Access::Write(to_be_bytes(7))),
        entry(&tree, writes[1], Access::Write(to_be_bytes(8))),
    ];
    let runs = diff_run(
        &tree,
        &entries,
        alias_tamper(offset(&flat_key_of(&writes[1]), 1)),
    );
    runs.assert_rejected();
    runs.assert_rejected_before_proof(tree.next_free_slot);
    assert!(runs.unhardened.counts[Counter::NewWriteAliasResolved as usize] > 0);
}

/// The cached-successor check in the reads pass: a new read whose opened predecessor links to the saved next free
/// slot. The reference opens the forged successor there, and rejects at the root.
#[test]
fn cached_successor_check_rejects_new_read_after_forged_link() {
    let (tree, leaf, reads) = alias_fixture(1);
    let entries = [
        entry(&tree, leaf, Access::Read),
        entry(&tree, reads[0], Access::Read),
    ];
    let runs = diff_run(
        &tree,
        &entries,
        alias_tamper(offset(&flat_key_of(&reads[0]), 1)),
    );
    runs.assert_rejected();
    runs.assert_rejected_before_proof(tree.next_free_slot);
    assert_eq!(runs.reference.log.len(), runs.hardened.log.len() + 1);
}
