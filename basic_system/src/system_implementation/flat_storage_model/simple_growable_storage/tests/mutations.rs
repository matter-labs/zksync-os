use super::*;

/// A replacement index answer.
#[derive(Clone, Copy, Debug)]
pub(super) enum Answer {
    /// An index the honest run opens
    Opened(Index),
    NextFreeSlot,
    AfterNextFreeSlot,
    Max,
    Zero,
    One,
}

/// A change of one response.
#[derive(Clone, Debug)]
pub(super) enum TamperKind {
    /// Flips a bit of a word: of the index or the leaf of a proof, or of any word
    Flip {
        index_or_leaf: bool,
        word: Index,
        bit: u8,
    },
    /// Replaces an index answer, or the index of a proof
    Answer(Answer),
    /// Replaces the leaf of a proof with the leaf of a proof of the honest run
    SwapLeaf(Index),
    /// Drops the words from a position on
    Truncate(Index),
    /// Appends a word
    Extend(u32),
}

pub(super) fn gen_tamper_kind() -> impl Strategy<Value = TamperKind> {
    let answer = prop_oneof![
        any::<Index>().prop_map(Answer::Opened),
        Just(Answer::NextFreeSlot),
        Just(Answer::AfterNextFreeSlot),
        Just(Answer::Max),
        Just(Answer::Zero),
        Just(Answer::One),
    ];
    prop_oneof![
        3 => (any::<bool>(), any::<Index>(), any::<u8>())
            .prop_map(|(index_or_leaf, word, bit)| TamperKind::Flip { index_or_leaf, word, bit }),
        3 => answer.prop_map(TamperKind::Answer),
        1 => any::<Index>().prop_map(TamperKind::SwapLeaf),
        1 => any::<Index>().prop_map(TamperKind::Truncate),
        1 => any::<u32>().prop_map(TamperKind::Extend),
    ]
}

pub(super) fn apply_tamper(
    kind: &TamperKind,
    query_id: u32,
    response: &mut Vec<u32>,
    honest: &[LoggedQuery],
    next_free_slot: u64,
) {
    match kind {
        TamperKind::Flip {
            index_or_leaf,
            word,
            bit,
        } => {
            let len = if query_id == PROOF && *index_or_leaf {
                PROOF_PATH
            } else {
                response.len()
            };
            response[word.index(len)] ^= 1 << (bit % 32);
        }
        TamperKind::Answer(answer) => {
            let opened: Vec<u64> = honest
                .iter()
                .filter_map(|(_, input, _)| match input {
                    Input::Index(index) => Some(*index),
                    Input::Key(_) => None,
                })
                .collect();
            let value = match answer {
                Answer::Opened(i) if !opened.is_empty() => *i.get(&opened),
                Answer::Opened(_) | Answer::Zero => 0,
                Answer::NextFreeSlot => next_free_slot,
                Answer::AfterNextFreeSlot => next_free_slot + 1,
                Answer::Max => u64::MAX,
                Answer::One => 1,
            };
            response[..2].copy_from_slice(&u64_words(value));
        }
        TamperKind::SwapLeaf(i) => {
            let proofs: Vec<&Vec<u32>> = honest
                .iter()
                .filter(|(id, ..)| *id == PROOF)
                .map(|(.., response)| response)
                .collect();
            if query_id == PROOF {
                let other = i.get(&proofs);
                response[PROOF_KEY..PROOF_PATH].copy_from_slice(&other[PROOF_KEY..PROOF_PATH]);
            }
        }
        TamperKind::Truncate(position) => {
            let len = response.len();
            response.truncate(position.index(len));
        }
        TamperKind::Extend(word) => response.push(*word),
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// A malicious oracle: one response of an honest run changed. The implementations agree
    /// on the outcome, rejection and the query log (some changes of siblings the
    /// fold does not use are accepted, by both, with the same root).
    #[test]
    fn single_tamper(
        slots in gen_previous_entries(0..24),
        batch in gen_batch(1..=24),
        query in any::<Index>(),
        kind in gen_tamper_kind(),
    ) {
        let tree = tree_with_slots(&slots);
        let existing: Vec<_> = slots.iter().map(|(key, _)| *key).collect();
        let entries = batch_entries(&tree, &existing, &batch);
        let honest = run(Implementation::Reference, &tree, &entries, None);
        prop_assume!(!honest.log.is_empty());
        let target = query.index(honest.log.len());
        let honest = std::rc::Rc::new(honest.log);
        let next_free_slot = tree.next_free_slot;
        diff_run(
            &tree,
            &entries,
            tamper(move |n, id, _, response| {
                if n == target {
                    apply_tamper(&kind, id, response, &honest, next_free_slot);
                }
            }),
        );
    }

    /// Combined changes can alter which later proofs are requested. Compare the full query
    /// log and outcome after two or three response changes at distinct query positions.
    #[test]
    fn multiple_tampers(
        slots in gen_previous_entries(0..12),
        batch in gen_batch(1..=12),
        changes in proptest::collection::vec((any::<Index>(), gen_tamper_kind()), 2..=3),
    ) {
        let tree = tree_with_slots(&slots);
        let existing: Vec<_> = slots.iter().map(|(key, _)| *key).collect();
        let entries = batch_entries(&tree, &existing, &batch);
        let honest = run(Implementation::Reference, &tree, &entries, None);
        prop_assume!(honest.log.len() >= 2);
        let mut seen = HashSet::new();
        let changes: Vec<_> = changes.into_iter()
            .map(|(position, kind)| (position.index(honest.log.len()), kind))
            .filter(|(position, _)| seen.insert(*position))
            .collect();
        prop_assume!(changes.len() >= 2);
        let honest = std::rc::Rc::new(honest.log);
        let next_free_slot = tree.next_free_slot;
        diff_run(
            &tree,
            &entries,
            tamper(move |n, id, _, response| {
                if let Some((_, kind)) = changes.iter().find(|(position, _)| *position == n) {
                    apply_tamper(kind, id, response, &honest, next_free_slot);
                }
            }),
        );
    }

    /// Every entry is a new slot with its own neighbours, and the rightmost leaf is not one of
    /// them: the proofs reach the bound the arenas are sized for, `items + new slots + 1`, and
    /// the keys theirs (every proof but the rightmost, and every new write).
    #[test]
    fn capacity_bound_is_reached(
        keys in proptest::collection::vec(uniform_full_key(), 1..40),
        writes in proptest::collection::vec(any::<bool>(), 40),
    ) {
        let mut keys = keys;
        keys.sort_by_key(flat_key_of);
        keys.dedup_by_key(|key| flat_key_of(key));
        let mut tree_keys = vec![offset(&Bytes32::MAX, -1)];
        for key in &keys {
            let flat_key = flat_key_of(key);
            prop_assume!(flat_key > Bytes32::from_byte_fill(0) && flat_key < offset(&Bytes32::MAX, -3));
            tree_keys.push(offset(&flat_key, -1));
            tree_keys.push(offset(&flat_key, 1));
        }
        let mut tree = tree_of(&tree_keys);
        let entries: Vec<_> = keys
            .iter()
            .zip(&writes)
            .enumerate()
            .map(|(i, (key, &write))| {
                let access = if write || i == 0 { Access::Write(to_be_bytes(i as u64 + 1)) } else { Access::Read };
                entry(&tree, *key, access)
            })
            .collect();
        let runs = check_batch_against_tree(&mut tree, &entries);
        prop_assert_eq!(runs.num_queries(PROOF), 2 * entries.len() + 1);
    }
}
