use super::super::*;

#[rustfmt::skip]
impl<const N: usize> FlatStorageCommitment<N> {
    ///
    /// Vefifies all the state reads and applies writes.
    ///
    pub(super) fn verify_and_apply_batch_reference<'a, O: IOOracle, A: Allocator + Clone + Default>(
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

        let mut key_to_index_cache = BTreeMap::<Bytes32, u64, A>::new_in(allocator.clone());
        let mut index_to_leaf_cache = BTreeMap::<
            u64,
            (
                LeafProof<N, Blake2sStorageHasher, A>,
                Option<LeafUpdateRecord>,
            ),
            A,
        >::new_in(allocator.clone());

        // If there was no IO, just return
        if source.clone().next().is_none() {
            return Ok(());
        }

        let reads_iter = source
            .clone()
            .filter(|(_, v)| v.current_value == v.initial_value);
        let writes_iter = source
            .clone()
            .filter(|(_, v)| v.current_value != v.initial_value);

        let num_new_writes = source
            .clone()
            .filter(|(_, v)| v.current_value != v.initial_value && v.is_new_storage_slot)
            .count();

        let mut new_writes = Vec::with_capacity_in(num_new_writes, allocator.clone());

        // save it for later
        let saved_next_free_slot = self.next_free_slot;

        let mut num_nonexisting_reads = 0;

        let mut deriver = FlatStorageKeyDeriver::new();
        for (key, value) in reads_iter {
            let flat_key = deriver.derive(&key.address, &key.key);
            // reads
            let expect_new = value.is_new_storage_slot;
            assert!(value.initial_value_used);
            if expect_new {
                assert_eq!(
                    value.initial_value,
                    Bytes32::ZERO,
                    "initial value of empty slot must be trivial"
                );
                num_nonexisting_reads += 1;

                logger_log!(
                    logger,
                    "checking empty read for address = {:?}, key = {:?}\n",
                    &key.address,
                    &key.key,
                );

                let previous_idx = get_prev_index::<O>(oracle, &flat_key);
                assert!(previous_idx < saved_next_free_slot);
                let next_idx;

                // Check if indexes are in cache,
                // otherwise get leaf and add to caches.
                {
                    let previous_check = |previous: &LeafProof<N, Blake2sStorageHasher, A>| {
                        assert!(previous.leaf.key < flat_key);
                    };
                    match index_to_leaf_cache.get(&previous_idx) {
                        None => {
                            let prev = get_proof_for_index::<N, O, Blake2sStorageHasher, A>(
                                oracle,
                                previous_idx,
                            )
                            .proof
                            .existing;
                            previous_check(&prev);

                            next_idx = prev.leaf.next;
                            assert!(next_idx < saved_next_free_slot);
                            let existing = key_to_index_cache.insert(prev.leaf.key, previous_idx);
                            assert!(existing.is_none());
                            index_to_leaf_cache.insert(previous_idx, (prev, None));
                        }
                        Some((prev, _)) => {
                            previous_check(prev);
                            next_idx = prev.leaf.next;
                        }
                    }
                }

                {
                    let next_check = |next: &LeafProof<N, Blake2sStorageHasher, A>| {
                        assert!(next.leaf.key > flat_key);
                    };
                    match index_to_leaf_cache.get(&next_idx) {
                        None => {
                            let next = get_proof_for_index::<N, O, Blake2sStorageHasher, A>(
                                oracle, next_idx,
                            )
                            .proof
                            .existing;
                            next_check(&next);
                            let existing = key_to_index_cache.insert(next.leaf.key, next_idx);
                            assert!(existing.is_none());
                            index_to_leaf_cache.insert(next_idx, (next, None));
                        }
                        Some((leaf, _)) => {
                            next_check(leaf);
                        }
                    }
                }
                // now we will need to check merkle paths for that indexes
            } else {
                logger_log!(
                    logger,
                    "checking existing read for address = {:?}, key = {:?}, value = {:?}\n",
                    &key.address,
                    &key.key,
                    value.current_value,
                );

                let index = get_index::<O>(oracle, &flat_key);

                let check = |leaf: &LeafProof<N, Blake2sStorageHasher, A>| {
                    assert_eq!(leaf.leaf.key, flat_key);
                    assert_eq!(leaf.leaf.value, value.current_value);
                };

                // Check if index is  in cache,
                // otherwise get leaf and add to caches.
                match index_to_leaf_cache.get(&index) {
                    None => {
                        let leaf =
                            get_proof_for_index::<N, O, Blake2sStorageHasher, A>(oracle, index)
                                .proof
                                .existing;
                        check(&leaf);
                        let existing = key_to_index_cache.insert(leaf.leaf.key, index);
                        assert!(existing.is_none());
                        index_to_leaf_cache.insert(index, (leaf, None));
                    }
                    Some((leaf, _)) => {
                        check(leaf);
                    }
                }
            }
        }

        // NOTE: we effectively build a linked list backed by the fixed capacity (merkle tree) array,
        // and our `key_to_index_cache` is just another representation of it

        let mut num_total_writes = 0;

        for (key, value) in writes_iter {
            num_total_writes += 1;
            let flat_key = deriver.derive(&key.address, &key.key);
            // writes
            let expect_new = value.is_new_storage_slot;
            if expect_new {
                logger_log!(
                    logger,
                    "applying initial write for address = {:?}, key = {:?}, value {:?} -> {:?}\n",
                    &key.address,
                    &key.key,
                    &value.initial_value,
                    &value.current_value
                );

                // since it's new, we always ask for neighbours
                let previous_idx = get_prev_index::<O>(oracle, &flat_key);
                assert!(previous_idx < saved_next_free_slot);
                let next_idx;
                // but we can still have two cases regarding touching such neighbours, if we insert something in the middle

                match index_to_leaf_cache.get(&previous_idx) {
                    Some((previous, _)) => {
                        next_idx = previous.leaf.next;
                        assert!(previous.leaf.key < flat_key);
                    }
                    None => {
                        let previous = get_proof_for_index::<N, O, Blake2sStorageHasher, A>(
                            oracle,
                            previous_idx,
                        );
                        next_idx = previous.proof.existing.leaf.next;
                        assert!(next_idx < saved_next_free_slot);
                        assert!(previous.proof.existing.leaf.key < flat_key);
                        // and we can insert
                        let existing = key_to_index_cache
                            .insert(previous.proof.existing.leaf.key, previous_idx);
                        assert!(existing.is_none());
                        let existing = index_to_leaf_cache.insert(
                            previous_idx,
                            (
                                previous.proof.existing,
                                None, // will be adjusted later one
                            ),
                        );
                        assert!(existing.is_none());
                    }
                }
                // same for next
                match index_to_leaf_cache.get(&next_idx) {
                    Some((next, _)) => {
                        assert!(next.leaf.key > flat_key);
                    }
                    None => {
                        let next =
                            get_proof_for_index::<N, O, Blake2sStorageHasher, A>(oracle, next_idx);
                        assert!(next.proof.existing.leaf.key > flat_key);
                        // and we can insert
                        let existing =
                            key_to_index_cache.insert(next.proof.existing.leaf.key, next_idx);
                        assert!(existing.is_none());
                        let existing = index_to_leaf_cache.insert(
                            next_idx,
                            (
                                next.proof.existing,
                                None, // will be adjusted later one
                            ),
                        );
                        assert!(existing.is_none());
                    }
                }

                // and we will append it to the tree
                let index = self.next_free_slot;
                self.next_free_slot += 1;
                // insert only in one of the caches
                let existing = key_to_index_cache.insert(flat_key, index);
                assert!(existing.is_none());
                new_writes.push((
                    index,
                    FlatStorageLeaf::<N> {
                        key: flat_key,
                        value: value.current_value,
                        next: 0, // will be adjusted later
                    },
                ));
            } else {
                logger_log!(
                    logger,
                    "applying repeated write for address = {:?}, key = {:?}, value {:?} -> {:?}\n",
                    &key.address,
                    &key.key,
                    &value.initial_value,
                    &value.current_value
                );
                // here we branch because we COULD have requested this one as a read witness

                if let Some(existing_index) = key_to_index_cache.get(&flat_key).copied() {
                    let (existing_leaf, modification) = index_to_leaf_cache
                        .get_mut(&existing_index)
                        .expect("must be in cache");
                    assert_eq!(existing_leaf.leaf.key, flat_key);
                    assert_eq!(existing_leaf.leaf.value, value.initial_value);
                    assert!(modification.is_none());
                    *modification = Some(LeafUpdateRecord {
                        value: Some(value.current_value),
                        next: None, // this value will be adjusted later
                    });
                } else {
                    let index = get_index::<O>(oracle, &flat_key);
                    let leaf = get_proof_for_index::<N, O, Blake2sStorageHasher, A>(oracle, index);
                    assert_eq!(leaf.proof.existing.leaf.key, flat_key);
                    assert_eq!(leaf.proof.existing.leaf.value, value.initial_value);
                    let existing = key_to_index_cache.insert(leaf.proof.existing.leaf.key, index);
                    assert!(existing.is_none());
                    let existing = index_to_leaf_cache.insert(
                        index,
                        (
                            leaf.proof.existing,
                            Some(LeafUpdateRecord {
                                value: Some(value.current_value),
                                next: None, // this value will be adjusted later
                            }),
                        ),
                    );
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
            let num_elements = key_to_index_cache.len();
            assert!(
                num_elements >= 2,
                "There should be at least bound leaves in the cache"
            );

            // here we should collect keys and walk over tuples of them
            // first one is special, and last one is special
            let mut keys = key_to_index_cache.keys();
            let first = keys.next().unwrap();
            let first_index = key_to_index_cache[first];
            let second = keys.next().unwrap();
            let second_index = key_to_index_cache[second];

            // it's initial guard leaf with key == 0
            let (leaf, modification) = index_to_leaf_cache
                .get_mut(&first_index)
                .expect("first leaf");
            if first_index == MIN_KEY_LEAF_MARKER_IDX {
                assert!(modification.is_none());
                assert_eq!(leaf.leaf.key, Bytes32::ZERO);
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
            let mut current_index = second_index;
            for next in keys {
                let next_index = key_to_index_cache[next];
                // we need to modify current
                match index_to_leaf_cache.get_mut(&current_index) {
                    Some((leaf, modification)) => {
                        // See the explanation above why this check is required.
                        if next_index >= saved_next_free_slot {
                            if let Some(modification) = modification.as_mut() {
                                if leaf.leaf.next != next_index {
                                    modification.set_next(next_index);
                                }
                            } else {
                                let mut new_modification = LeafUpdateRecord {
                                    value: None,
                                    next: None,
                                };
                                if leaf.leaf.next != next_index {
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

                current_index = next_index;
            }

            // and finish with last two

            let last_index = current_index;

            let (leaf, modification) = index_to_leaf_cache.get(&last_index).expect("last leaf");
            if last_index == MAX_KEY_LEAF_MARKER_IDX {
                assert!(modification.is_none());
                assert_eq!(leaf.leaf.key, Bytes32::MAX);
            }

            // now we potentially augment `index_to_leaf_cache` using the path for the "rightmost" tree element,
            // and recompute a binary tree by folding either computed nodes, or computed + witness

            if num_new_writes > 0 {
                // get rightmost path as it'll be needed anyway to append more leaves
                if index_to_leaf_cache.contains_key(&(saved_next_free_slot - 1)) == false {
                    let proof = get_proof_for_index::<N, O, Blake2sStorageHasher, A>(
                        oracle,
                        saved_next_free_slot - 1,
                    );
                    index_to_leaf_cache
                        .insert(saved_next_free_slot - 1, (proof.proof.existing, None));
                }
            }
        }

        // and now we join
        let mut hasher = Blake2sStorageHasher::new();
        let empty_hashes =
            compute_empty_hashes::<N, Blake2sStorageHasher, A>(&mut hasher, allocator.clone());

        // now we should have fun and join the paths
        let buffer_size = index_to_leaf_cache.len() + num_new_writes;
        let mut current_hashes_buffer = Vec::with_capacity_in(buffer_size, allocator.clone());
        let mut next_hashes_buffer = Vec::with_capacity_in(buffer_size, allocator.clone());

        for (index, (leaf, modification)) in index_to_leaf_cache.iter() {
            let leaf_hash = hasher.hash_leaf(&leaf.leaf);
            let updated_hash = if let Some(modification) = modification.as_ref() {
                let mut updated_leaf = leaf.leaf;
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

            current_hashes_buffer.push((*index, *index, Some(leaf_hash), updated_hash));
        }
        // append new writes
        for (index, new_leaf) in new_writes.into_iter() {
            let leaf_hash = hasher.hash_leaf(&new_leaf);
            current_hashes_buffer.push((index, index, None, Some(leaf_hash)));
        }

        // then merge
        fn can_merge(pair: (u64, u64)) -> bool {
            let (a, b) = pair;
            debug_assert_ne!(a, b);
            a & !1 == b & !1
        }

        let process_single = |a: &(u64, u64, Option<Bytes32>, Option<Bytes32>),
                              level: u32,
                              dst: &mut Vec<(u64, u64, Option<Bytes32>, Option<Bytes32>), A>,
                              hasher: &mut Blake2sStorageHasher| {
            let is_left = a.0 & 1 == 0;
            let proof = match index_to_leaf_cache.get(&a.1) {
                Some((leaf, _)) => &leaf.path[level as usize],
                None => {
                    // use default
                    &empty_hashes[level as usize]
                }
            };

            let read_path = if let Some(read_path) = a.2.as_ref() {
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

            let write_path = if let Some(write_path) = a.3.as_ref() {
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

            let index = a.0 >> 1;
            dst.push((index, a.1, read_path, write_path));
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

                assert_ne!(a.0, b.0);

                let is_last = idx == num_windows - 1;
                let empty_path = &empty_hashes[level];

                if can_merge((a.0, b.0)) {
                    // two paths will converge now
                    let (read_hash, a_read_path, b_read_path) = match (a.2.as_ref(), b.2.as_ref()) {
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
                            assert!(a.1 >= saved_next_free_slot);
                            assert!(b.1 >= saved_next_free_slot);
                            (None, empty_path, empty_path)
                        }
                    };

                    let write_hash = match (a.3.as_ref(), b.3.as_ref()) {
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

                    let merged_index = a.0 >> 1;
                    debug_assert_eq!(merged_index, b.0 >> 1);
                    next_hashes_buffer.push((merged_index, a.1, read_hash, write_hash));
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
                let a: &(u64, u64, Option<Bytes32>, Option<Bytes32>) =
                    current_hashes_buffer.last().unwrap();
                process_single(a, level as u32, &mut next_hashes_buffer, &mut hasher);
            }

            current_hashes_buffer.clear();
            core::mem::swap(&mut current_hashes_buffer, &mut next_hashes_buffer);
        }

        assert!(next_hashes_buffer.is_empty());
        assert_eq!(current_hashes_buffer.len(), 1);
        assert_eq!(
            current_hashes_buffer[0].2.expect("read root"),
            self.root,
            "storage reads are inconsistent"
        );
        // if we have new root - use it
        if let Some(new_root) = current_hashes_buffer[0].3 {
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

#[rustfmt::skip]
fn get_proof_for_index<
    const N: usize,
    O: IOOracle,
    H: FlatStorageHasher,
    A: Allocator + Clone + Default,
>(
    oracle: &mut O,
    index: u64,
) -> ValueAtIndexProof<N, H, A> {
    let mut proven_index = core::mem::MaybeUninit::uninit();
    let mut leaf = core::mem::MaybeUninit::uninit();
    // the oracle writes the path straight into its allocation
    let mut path = Box::<[Bytes32; N], A>::new_uninit_in(A::default());
    let (proven_index, leaf, _) = ProofForIndexQuery::<N>::get_into(
        oracle,
        &index,
        (&mut proven_index, &mut leaf, &mut *path),
    )
    .expect("must get the proof for index");
    assert_eq!(*proven_index, index);
    let leaf = *leaf;
    // SAFETY: the path was just written and validated
    let path = unsafe { path.assume_init() };

    ValueAtIndexProof {
        proof: ExistingReadProof {
            existing: LeafProof::new(index, leaf, path),
        },
    }
}
