use super::*;

impl<const N: usize> StateRootView<EthereumIOTypesConfig> for FlatStorageCommitment<N> {
    ///
    /// Vefifies all the state reads and applies writes.
    ///
    fn verify_and_apply_batch<'a, O: IOOracle, A: Allocator + Clone + Default>(
        &mut self,
        oracle: &mut O,
        source: impl Iterator<Item = (WarmStorageKey, WarmStorageValue)> + Clone,
        allocator: A,
        logger: &mut impl Logger,
    ) -> Result<(), InternalError> {
        // we know that for our storage model we have a luxury of amortizing all new writes,
        // so our strategy is to:
        // - verify all reads
        // - apply non-initial writes
        // - get a proof of the last stored item (by index)
        // - compute leaf hashes for a batch and extend tree
        // - perform updates of pointers in existing section of the tree
        // - carefully handle the case if we potentially have pointers to each other in the appended section

        // and then we can go even further, by checking early intersections, and reducing a number of accesses
        // if we have many of them (by trading control flow vs more hash functions)

        // One traversal of the source, which derives every flat key once. Derivation is pure and
        // issues no query, so doing it up front, in source order, is not observable.
        let (min_len, max_len) = source.size_hint();
        let num_items = if max_len == Some(min_len) {
            min_len
        } else {
            source.clone().count()
        };
        let mut items = ChunkedVec::<BatchItem, ITEM_CHUNK_LEN, A>::with_max_len_in(
            num_items,
            allocator.clone(),
        );
        let mut num_new_slots = 0;
        let mut num_new_writes = 0;
        let mut deriver = FlatStorageKeyDeriver::new();
        for (key, value) in source {
            let is_write = value.current_value != value.initial_value;
            match (is_write, value.is_new_storage_slot) {
                (false, true) => logger_log!(
                    logger,
                    "checking empty read for address = {:?}, key = {:?}\n",
                    &key.address,
                    &key.key,
                ),
                (false, false) => logger_log!(
                    logger,
                    "checking existing read for address = {:?}, key = {:?}, value = {:?}\n",
                    &key.address,
                    &key.key,
                    value.current_value,
                ),
                (true, true) => logger_log!(
                    logger,
                    "applying initial write for address = {:?}, key = {:?}, value {:?} -> {:?}\n",
                    &key.address,
                    &key.key,
                    &value.initial_value,
                    &value.current_value
                ),
                (true, false) => logger_log!(
                    logger,
                    "applying repeated write for address = {:?}, key = {:?}, value {:?} -> {:?}\n",
                    &key.address,
                    &key.key,
                    &value.initial_value,
                    &value.current_value
                ),
            }
            num_new_slots += value.is_new_storage_slot as usize;
            num_new_writes += (is_write && value.is_new_storage_slot) as usize;
            items.push(BatchItem {
                flat_key: deriver.derive(&key.address, &key.key),
                initial_value: value.initial_value,
                current_value: value.current_value,
                is_new: value.is_new_storage_slot,
                initial_value_used: value.initial_value_used,
                is_write,
            });
        }

        // If there was no IO, just return
        if items.len() == 0 {
            return Ok(());
        }

        // Bounds for every answer stream: an item opens at most one leaf (an existing slot) or two
        // (the neighbours of a new one), and the rightmost leaf is opened at most once more. Every
        // opened leaf but the rightmost, and every new write, adds a key.
        let max_opened = items.len() + num_new_slots + 1;
        let max_keys = max_opened - 1 + num_new_writes;
        // The opened leaves have paths at the same positions. The index map points to opened
        // leaves; the key map also points to new writes.
        let mut opened = ChunkedVec::<OpenedLeaf<N>, LEAF_CHUNK_LEN, A>::with_max_len_in(
            max_opened,
            allocator.clone(),
        );
        let mut paths = ChunkedVec::<[Bytes32; N], LEAF_CHUNK_LEN, A>::with_max_len_in(
            max_opened,
            allocator.clone(),
        );
        let mut by_index = BTreeMap::<u64, usize, A>::new_in(allocator.clone());
        let mut keys = KeyIndex::with_max_len_in(max_keys, allocator.clone());

        let mut new_writes = Vec::with_capacity_in(num_new_writes, allocator.clone());

        // save it for later
        let saved_next_free_slot = self.next_free_slot;

        let mut num_nonexisting_reads = 0;

        for item in items.iter().filter(|item| !item.is_write) {
            let flat_key = &item.flat_key;
            // reads
            assert!(item.initial_value_used);
            if item.is_new {
                assert_eq!(
                    item.initial_value,
                    Bytes32::ZERO,
                    "initial value of empty slot must be trivial"
                );
                num_nonexisting_reads += 1;

                let previous_idx = get_prev_index::<O>(oracle, flat_key);
                assert!(previous_idx < saved_next_free_slot);
                let next_idx;

                // Check if indexes are in cache,
                // otherwise get leaf and add to caches.
                match by_index.get(&previous_idx).copied() {
                    None => {
                        let pos = open_leaf(oracle, previous_idx, &mut opened, &mut paths);
                        let previous = &opened.get(pos).leaf;
                        assert!(previous.key < *flat_key);
                        next_idx = previous.next;
                        assert!(next_idx < saved_next_free_slot);
                        keys.insert_new(previous.key, KeyTarget::Opened(pos));
                        by_index.insert(previous_idx, pos);
                    }
                    Some(pos) => {
                        let previous = &opened.get(pos).leaf;
                        assert!(previous.key < *flat_key);
                        next_idx = previous.next;
                        // A cached predecessor must link to an existing leaf.
                        if hardening_enabled() {
                            assert!(next_idx < saved_next_free_slot);
                        }
                    }
                }

                match by_index.get(&next_idx).copied() {
                    None => {
                        let pos = open_leaf(oracle, next_idx, &mut opened, &mut paths);
                        let next = &opened.get(pos).leaf;
                        assert!(next.key > *flat_key);
                        keys.insert_new(next.key, KeyTarget::Opened(pos));
                        by_index.insert(next_idx, pos);
                    }
                    Some(pos) => {
                        assert!(opened.get(pos).leaf.key > *flat_key);
                    }
                }
                // now we will need to check merkle paths for that indexes
            } else {
                let index = get_index::<O>(oracle, flat_key);
                // An existing key must occupy a leaf below the next free slot.
                if hardening_enabled() {
                    assert!(index < saved_next_free_slot);
                }

                // Check if index is  in cache,
                // otherwise get leaf and add to caches.
                match by_index.get(&index).copied() {
                    None => {
                        let pos = open_leaf(oracle, index, &mut opened, &mut paths);
                        let leaf = &opened.get(pos).leaf;
                        assert_eq!(leaf.key, *flat_key);
                        assert_eq!(leaf.value, item.current_value);
                        keys.insert_new(leaf.key, KeyTarget::Opened(pos));
                        by_index.insert(index, pos);
                    }
                    Some(pos) => {
                        let leaf = &opened.get(pos).leaf;
                        assert_eq!(leaf.key, *flat_key);
                        assert_eq!(leaf.value, item.current_value);
                    }
                }
            }
        }

        // NOTE: we effectively build a linked list backed by the fixed capacity (merkle tree) array,
        // and our `keys` is just another representation of it

        let mut num_total_writes = 0;

        for item in items.iter().filter(|item| item.is_write) {
            num_total_writes += 1;
            let flat_key = &item.flat_key;
            // writes
            if item.is_new {
                // since it's new, we always ask for neighbours
                let previous_idx = get_prev_index::<O>(oracle, flat_key);
                assert!(previous_idx < saved_next_free_slot);
                let next_idx;
                // but we can still have two cases regarding touching such neighbours, if we insert something in the middle

                match by_index.get(&previous_idx).copied() {
                    Some(pos) => {
                        let previous = &opened.get(pos).leaf;
                        next_idx = previous.next;
                        assert!(previous.key < *flat_key);
                        // A cached predecessor must link to an existing leaf.
                        if hardening_enabled() {
                            assert!(next_idx < saved_next_free_slot);
                        }
                    }
                    None => {
                        let pos = open_leaf(oracle, previous_idx, &mut opened, &mut paths);
                        let previous = &opened.get(pos).leaf;
                        next_idx = previous.next;
                        assert!(next_idx < saved_next_free_slot);
                        assert!(previous.key < *flat_key);
                        // and we can insert
                        keys.insert_new(previous.key, KeyTarget::Opened(pos));
                        let existing = by_index.insert(previous_idx, pos);
                        assert!(existing.is_none());
                    }
                }
                // same for next
                match by_index.get(&next_idx).copied() {
                    Some(pos) => {
                        assert!(opened.get(pos).leaf.key > *flat_key);
                    }
                    None => {
                        let pos = open_leaf(oracle, next_idx, &mut opened, &mut paths);
                        let next = &opened.get(pos).leaf;
                        assert!(next.key > *flat_key);
                        // and we can insert
                        keys.insert_new(next.key, KeyTarget::Opened(pos));
                        let existing = by_index.insert(next_idx, pos);
                        assert!(existing.is_none());
                    }
                }

                // and we will append it to the tree
                let index = self.next_free_slot;
                self.next_free_slot += 1;
                // insert only in one of the caches
                keys.insert_new(*flat_key, KeyTarget::Inserted(new_writes.len()));
                push_within_capacity(
                    &mut new_writes,
                    (
                        index,
                        FlatStorageLeaf::<N> {
                            key: *flat_key,
                            value: item.current_value,
                            next: 0, // will be adjusted later
                        },
                    ),
                );
            } else {
                // here we branch because we COULD have requested this one as a read witness

                if let Some(target) = keys.get(flat_key) {
                    let pos = target
                        .resolve(&by_index, &opened, &new_writes)
                        .expect("must be in cache");
                    let OpenedLeaf {
                        leaf, modification, ..
                    } = opened.get_mut(pos);
                    assert_eq!(leaf.key, *flat_key);
                    assert_eq!(leaf.value, item.initial_value);
                    assert!(modification.is_none());
                    *modification = Some(LeafUpdateRecord {
                        value: Some(item.current_value),
                        next: None, // this value will be adjusted later
                    });
                } else {
                    let index = get_index::<O>(oracle, flat_key);
                    // An existing key must occupy a leaf below the next free slot.
                    if hardening_enabled() {
                        assert!(index < saved_next_free_slot);
                    }
                    let pos = open_leaf(oracle, index, &mut opened, &mut paths);
                    let OpenedLeaf {
                        leaf, modification, ..
                    } = opened.get_mut(pos);
                    assert_eq!(leaf.key, *flat_key);
                    assert_eq!(leaf.value, item.initial_value);
                    keys.insert_new(leaf.key, KeyTarget::Opened(pos));
                    *modification = Some(LeafUpdateRecord {
                        value: Some(item.current_value),
                        next: None, // this value will be adjusted later
                    });
                    let existing = by_index.insert(index, pos);
                    assert!(existing.is_none());
                }
            }
        }

        // now we can adjust indexes

        if num_nonexisting_reads + num_new_writes == 0 {
            // it means that we do not need to remake a linked list and just read slots with existing matching keys or update them
        } else {
            // It should be a case when we have at least one initial write or empty read - we should at least have key::MIN and key::MAX,
            // so we can take
            let num_elements = keys.len();
            assert!(
                num_elements >= 2,
                "There should be at least bound leaves in the cache"
            );

            // here we should walk over tuples of keys, in key order
            // first one is special, and last one is special
            let mut targets = keys.iter().map(|(_, target)| target);
            let first = targets.next().unwrap();
            let first_index = first.index(&opened, &new_writes);
            let second = targets.next().unwrap();
            let second_index = second.index(&opened, &new_writes);

            // it's initial guard leaf with key == 0
            let pos = first
                .resolve(&by_index, &opened, &new_writes)
                .expect("first leaf");
            let OpenedLeaf {
                leaf, modification, ..
            } = opened.get_mut(pos);
            if first_index == MIN_KEY_LEAF_MARKER_IDX {
                assert!(modification.is_none());
                assert_eq!(leaf.key, Bytes32::ZERO);
            }

            if second_index >= saved_next_free_slot {
                // We only need to relink pointers if either the current leaf is new, or the previously existing leaf
                // has a new leaf inserted after it, or both. If both the current and next leaves are old, linking them is generally incorrect.
                // As an example consider a leaf with `key_1` loaded as a next leaf for one or more inserts, and `key_2` loaded as a previous leaf
                // for one or more inserts, such that `key_1 < key_2` and there are no other leaves in `key_1..key_2` loaded for the proof. In this case,
                // unless we do filtering above, we'll link these 2 leaves together, but there may be an indefinite number of keys in the `key_1..key_2` range in the full tree!
                if let Some(modification) = modification.as_mut() {
                    modification.set_next(second_index);
                } else {
                    // no value update, but only index update
                    *modification = Some(LeafUpdateRecord {
                        value: None,
                        next: Some(second_index),
                    });
                }
            }

            // now we work over sets of 3
            let mut current = second;
            let mut current_index = second_index;
            for next in targets {
                let next_index = next.index(&opened, &new_writes);
                // we need to modify current
                match current.resolve(&by_index, &opened, &new_writes) {
                    Some(pos) => {
                        let OpenedLeaf {
                            leaf, modification, ..
                        } = opened.get_mut(pos);
                        // See the explanation above why this check is required.
                        if next_index >= saved_next_free_slot {
                            if let Some(modification) = modification.as_mut() {
                                if leaf.next != next_index {
                                    modification.set_next(next_index);
                                }
                            } else {
                                let mut new_modification = LeafUpdateRecord {
                                    value: None,
                                    next: None,
                                };
                                if leaf.next != next_index {
                                    new_modification.set_next(next_index);
                                }
                                *modification = Some(new_modification);
                            }
                        }
                    }
                    None => {
                        // it's initial write
                        let idx = current_index - saved_next_free_slot;
                        let (expected_tree_index, leaf) = &mut new_writes[idx as usize];
                        assert_eq!(*expected_tree_index, current_index);
                        leaf.next = next_index;
                    }
                }

                current = next;
                current_index = next_index;
            }

            // and finish with last two

            let last_index = current_index;

            let pos = current
                .resolve(&by_index, &opened, &new_writes)
                .expect("last leaf");
            if last_index == MAX_KEY_LEAF_MARKER_IDX {
                let OpenedLeaf {
                    leaf, modification, ..
                } = opened.get(pos);
                assert!(modification.is_none());
                assert_eq!(leaf.key, Bytes32::MAX);
            }

            // now we potentially augment the opened leaves using the path for the "rightmost" tree element,
            // and recompute a binary tree by folding either computed nodes, or computed + witness

            if num_new_writes > 0 {
                // get rightmost path as it'll be needed anyway to append more leaves
                if by_index.contains_key(&(saved_next_free_slot - 1)) == false {
                    let pos = open_leaf(oracle, saved_next_free_slot - 1, &mut opened, &mut paths);
                    by_index.insert(saved_next_free_slot - 1, pos);
                }
            }
        }

        // and now we join
        let mut hasher = Blake2sStorageHasher::new();
        let empty_hashes =
            compute_empty_hashes::<N, Blake2sStorageHasher, A>(&mut hasher, allocator.clone());

        // now we should have fun and join the paths
        let buffer_size = by_index.len() + num_new_writes;
        let mut current_hashes_buffer = Vec::with_capacity_in(buffer_size, allocator.clone());
        let mut next_hashes_buffer = Vec::with_capacity_in(buffer_size, allocator.clone());

        for (&index, &pos) in by_index.iter() {
            let OpenedLeaf {
                leaf, modification, ..
            } = opened.get(pos);
            let leaf_hash = hasher.hash_leaf(leaf);
            let updated_hash = if let Some(modification) = modification.as_ref() {
                let mut updated_leaf = *leaf;
                if let Some(value) = modification.value.as_ref() {
                    updated_leaf.value = *value;
                }
                if let Some(next) = modification.next.as_ref() {
                    updated_leaf.next = *next;
                }
                let updated_leaf_hash = hasher.hash_leaf(&updated_leaf);

                Some(updated_leaf_hash)
            } else {
                None
            };

            push_within_capacity(
                &mut current_hashes_buffer,
                FoldNode::new(index, index, pos, Some(leaf_hash), updated_hash),
            );
        }
        // append new writes
        for (index, new_leaf) in new_writes.into_iter() {
            let leaf_hash = hasher.hash_leaf(&new_leaf);
            // The index bounds checks keep opened leaves below the saved next free slot.
            // Keep the lookup for the unhardened differential test against the old implementation.
            let path = by_index.get(&index).copied().unwrap_or(NO_PATH);
            #[cfg(test)]
            if path != NO_PATH {
                test::count(test::Counter::NewWriteAliasPath);
            }
            push_within_capacity(
                &mut current_hashes_buffer,
                FoldNode::new(index, index, path, None, Some(leaf_hash)),
            );
        }
        // Fold entries must be strictly ordered by leaf index. The index bounds checks
        // keep opened leaves below the saved next free slot; new writes are appended from it.
        debug_assert!(
            !hardening_enabled()
                || current_hashes_buffer
                    .array_windows::<2>()
                    .all(|[a, b]| a.index < b.index)
        );

        // then merge
        fn can_merge(pair: (u64, u64)) -> bool {
            let (a, b) = pair;
            debug_assert_ne!(a, b);
            a & !1 == b & !1
        }

        let process_single = |a: &FoldNode,
                              level: u32,
                              dst: &mut Vec<FoldNode, A>,
                              hasher: &mut Blake2sStorageHasher| {
            let is_left = a.index & 1 == 0;
            let proof = if a.path != NO_PATH {
                &paths.get(a.path)[level as usize]
            } else {
                // use default
                &empty_hashes[level as usize]
            };

            let read_path = if let Some(read_path) = a.read() {
                let (left, right) = if is_left {
                    (read_path, proof)
                } else {
                    (proof, read_path)
                };
                let node_hash = hasher.hash_node(left, right);

                Some(node_hash)
            } else {
                None
            };

            let write_path = if let Some(write_path) = a.write() {
                let (left, right) = if is_left {
                    (write_path, proof)
                } else {
                    (proof, write_path)
                };
                let node_hash = hasher.hash_node(left, right);

                Some(node_hash)
            } else {
                None
            };

            let index = a.index >> 1;
            push_within_capacity(
                dst,
                FoldNode::new(index, a.origin, a.path, read_path, write_path),
            );
        };

        for level in 0..N {
            assert!(!current_hashes_buffer.is_empty());
            if current_hashes_buffer.len() == 1 {
                // just progress
                let a = &current_hashes_buffer[0];
                process_single(a, level as u32, &mut next_hashes_buffer, &mut hasher);

                current_hashes_buffer.clear();
                core::mem::swap(&mut current_hashes_buffer, &mut next_hashes_buffer);
                continue;
            }

            let mut next_merged = false;
            let num_windows = current_hashes_buffer.len() - 1;
            let mut last_merged = false;
            for (idx, [a, b]) in current_hashes_buffer.array_windows::<2>().enumerate() {
                if next_merged {
                    next_merged = false;
                    continue;
                }

                assert_ne!(a.index, b.index);

                let is_last = idx == num_windows - 1;
                let empty_path = &empty_hashes[level];

                if can_merge((a.index, b.index)) {
                    // two paths will converge now
                    let (read_hash, a_read_path, b_read_path) = match (a.read(), b.read()) {
                        (Some(a_read_path), Some(b_read_path)) => {
                            let node_hash = hasher.hash_node(a_read_path, b_read_path);
                            (Some(node_hash), a_read_path, b_read_path)
                        }
                        (Some(a_read_path), None) => {
                            let node_hash = hasher.hash_node(a_read_path, empty_path);
                            (Some(node_hash), a_read_path, empty_path)
                        }
                        (None, Some(b_read_path)) => {
                            let node_hash = hasher.hash_node(empty_path, b_read_path);
                            (Some(node_hash), empty_path, b_read_path)
                        }
                        (None, None) => {
                            assert!(a.origin >= saved_next_free_slot);
                            assert!(b.origin >= saved_next_free_slot);
                            (None, empty_path, empty_path)
                        }
                    };

                    let write_hash = match (a.write(), b.write()) {
                        (Some(a_write_path), Some(b_write_path)) => {
                            let node_hash = hasher.hash_node(a_write_path, b_write_path);
                            Some(node_hash)
                        }
                        (Some(a_write_path), None) => {
                            let node_hash = hasher.hash_node(a_write_path, b_read_path);
                            Some(node_hash)
                        }
                        (None, Some(b_write_path)) => {
                            let node_hash = hasher.hash_node(a_read_path, b_write_path);
                            Some(node_hash)
                        }
                        (None, None) => {
                            assert!(read_hash.is_some());
                            None
                        }
                    };

                    let merged_index = a.index >> 1;
                    debug_assert_eq!(merged_index, b.index >> 1);
                    push_within_capacity(
                        &mut next_hashes_buffer,
                        FoldNode::new(merged_index, a.origin, a.path, read_hash, write_hash),
                    );
                    next_merged = true;
                    if is_last {
                        last_merged = true;
                    }
                } else {
                    // progress for `a`` only
                    process_single(a, level as u32, &mut next_hashes_buffer, &mut hasher);
                }
            }

            if last_merged == false {
                // we need to progress last
                let a: &FoldNode = current_hashes_buffer.last().unwrap();
                process_single(a, level as u32, &mut next_hashes_buffer, &mut hasher);
            }

            current_hashes_buffer.clear();
            core::mem::swap(&mut current_hashes_buffer, &mut next_hashes_buffer);
        }

        assert!(next_hashes_buffer.is_empty());
        assert_eq!(current_hashes_buffer.len(), 1);
        assert_eq!(
            *current_hashes_buffer[0].read().expect("read root"),
            self.root,
            "storage reads are inconsistent"
        );
        // if we have new root - use it
        if let Some(&new_root) = current_hashes_buffer[0].write() {
            if num_total_writes > 0 {
                assert!(
                    new_root != self.root,
                    "hash collision on state root with non-zero number of writes"
                );
            }
            self.root = new_root;
        } else {
            assert_eq!(num_new_writes, 0);
            // root should not change in such case
        }

        Ok(())
    }
}

pub(super) fn get_prev_index<O: IOOracle>(oracle: &mut O, flat_key: &Bytes32) -> u64 {
    PreviousIndexQuery::get(oracle, flat_key).expect("must deserialize prev index")
}

pub(super) fn get_index<O: IOOracle>(oracle: &mut O, flat_key: &Bytes32) -> u64 {
    ExactIndexQuery::get(oracle, flat_key).expect("must deserialize index for key")
}

/// Gets the proof of the leaf at `index`: the path is written straight into the next slot of
/// `paths`, and the leaf is opened at the same position of `opened`, which is returned.
pub(super) fn open_leaf<const N: usize, O: IOOracle, A: Allocator + Clone>(
    oracle: &mut O,
    index: u64,
    opened: &mut ChunkedVec<OpenedLeaf<N>, LEAF_CHUNK_LEN, A>,
    paths: &mut ChunkedVec<[Bytes32; N], LEAF_CHUNK_LEN, A>,
) -> usize {
    let pos = opened.len();
    debug_assert_eq!(pos, paths.len());
    let mut proven_index = MaybeUninit::uninit();
    let mut leaf = MaybeUninit::uninit();
    let (proven_index, leaf, _) = ProofForIndexQuery::<N>::get_into(
        oracle,
        &index,
        (&mut proven_index, &mut leaf, paths.next_slot()),
    )
    .expect("must get the proof for index");
    assert_eq!(*proven_index, index);
    let leaf = *leaf;
    // SAFETY: the path was just written and validated
    unsafe { paths.commit_next() };
    opened.push(OpenedLeaf {
        index,
        leaf,
        modification: None,
    });

    pos
}

/// Whether the additional existing-leaf index checks run. They hold for an honest oracle over
/// a well-formed tree, and reject some malicious answers before the final root check: an
/// existing key must have an index below the saved next free slot, and a cached predecessor
/// must link to a leaf below the saved next free slot.
/// These bounds also ensure that the fold buffer is strictly ordered by leaf index,
/// which is checked by a debug assertion.
///
/// Only the unit tests that compare the function without them to the reference copy of the previous
/// implementation turn them off.
#[inline(always)]
pub(super) fn hardening_enabled() -> bool {
    #[cfg(test)]
    {
        test::HARDENING.get()
    }
    #[cfg(not(test))]
    {
        true
    }
}

/// An element of the batch, with its flat key.
pub(super) struct BatchItem {
    pub(super) flat_key: Bytes32,
    pub(super) initial_value: Bytes32,
    pub(super) current_value: Bytes32,
    pub(super) is_new: bool,
    pub(super) initial_value_used: bool,
    /// The current value differs from the initial one: a write, else a read
    pub(super) is_write: bool,
}

/// A leaf opened with a proof, whose path is at the same position of the paths.
pub(super) struct OpenedLeaf<const N: usize> {
    pub(super) index: u64,
    pub(super) leaf: FlatStorageLeaf<N>,
    pub(super) modification: Option<LeafUpdateRecord>,
}
