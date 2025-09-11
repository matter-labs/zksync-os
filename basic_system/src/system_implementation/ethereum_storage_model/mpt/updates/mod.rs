use super::*;

use zk_ee::memory::vec_trait::VecLikeCtor;

mod delete_from_branch;
mod delete_leaf;
mod delete_subtree;
mod insert_new_leaf_into_branch;
mod reattach;
mod split_existing;
mod split_extension;
mod split_leaf;
mod update_leaf_value;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum ValueInsertionStrategy {
    MakeLeafAttachedToBranch {
        branch: NodeType,
        branch_index: usize,
    },
    Split {
        alternative_path: NodeType,
        parent_branch_or_empty: NodeType,
        branch_index: usize,
        common_prefix_len: usize,
    },
}

impl<'a, A: Allocator + Clone, VC: VecLikeCtor> EthereumMPT<'a, A, VC> {
    #[inline(always)]
    pub(crate) fn remove_from_cache(&mut self, node: NodeType) {
        if node.is_leaf() {
            self.capacities.leaf_nodes[node.index()].invalidate_cache();
        } else if node.is_extension() {
            self.capacities.extension_nodes[node.index()].invalidate_cache();
        } else if node.is_branch() {
            self.capacities.branch_nodes[node.index()].invalidate_cache();
        } else if node.is_unreferenced_key() {
            panic!("tried to delete unreferenced key from cache",);
        } else if node.is_empty() {
            // nothing
        } else {
            unreachable!("trying to remove cache for node {:?}", node);
        }
    }

    #[inline(always)]
    pub(crate) fn get_cached_key(&mut self, node: NodeType) -> &'a [u8] {
        if node.is_leaf() {
            self.capacities.leaf_nodes[node.index()].cached_key
        } else if node.is_extension() {
            self.capacities.extension_nodes[node.index()].cached_key
        } else if node.is_branch() {
            self.capacities.branch_nodes[node.index()].cached_key
        } else if node.is_unreferenced_key() {
            self.capacities.unreferenced_keys[node.index()].cached_key
        } else if node.is_empty() {
            EMPTY_SLICE_ENCODING
        } else {
            unreachable!("trying to get cached key for node {:?}", node);
        }
    }

    // we will mark descend path as dirty, but final node will be marked and updated only in the corresponding path
    pub(crate) fn find_terminal_node_for_update_or_delete(
        &mut self,
        mut path: Path<'_>,
    ) -> Result<NodeType, ()> {
        let mut current_node = self.root;
        loop {
            self.remove_from_cache(current_node);
            match self.descend_through_existing_nodes(&mut path, current_node)? {
                DescendPath::PathDiverged { .. } => return Err(()),
                DescendPath::EmptyBranchTaken { .. } => return Err(()),
                DescendPath::LeafReached { final_node, .. } => {
                    debug_assert_eq!(current_node, final_node);
                    return Ok(final_node);
                }
                DescendPath::EndReachedAtEmptyBranchValue { .. } => {
                    return Err(());
                }
                DescendPath::UnreferencedPathEncountered { .. } => {
                    return Err(());
                }
                DescendPath::Follow { next_node, .. } => {
                    debug_assert_ne!(current_node, next_node);
                    current_node = next_node;
                }
            }
        }
    }

    fn make_diverging_case(
        &self,
        path: &Path<'_>,
        alternative_node: NodeType,
        common_prefix_len: usize,
    ) -> Result<ValueInsertionStrategy, ()> {
        // we have another extension/leaf node as the nearest neighbour,
        // and we need to understand whether we diverge at the first path element
        // immediately (so we just make branch), or make extension + branch
        let parent = if alternative_node.is_extension() {
            let node = &self.capacities.extension_nodes[alternative_node.index()];
            node.parent_node
        } else if alternative_node.is_leaf() {
            let node = &self.capacities.leaf_nodes[alternative_node.index()];
            node.parent_node
        } else {
            return Err(());
        };
        let branch_index = if parent.is_empty() {
            debug_assert_eq!(self.root, alternative_node);
            0
        } else if parent.is_branch() {
            path.prefix()[path.prefix_len - common_prefix_len - 1] as usize
        } else {
            return Err(());
        };
        Ok(ValueInsertionStrategy::Split {
            alternative_path: alternative_node,
            parent_branch_or_empty: parent,
            branch_index,
            common_prefix_len,
        })
    }

    pub(crate) fn find_insertion_strategy(
        &mut self,
        path: &mut Path<'_>,
        preimages_oracle: &mut impl PreimagesOracle,
        interner: &mut (impl Interner<'a> + 'a),
        hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
    ) -> Result<ValueInsertionStrategy, ()> {
        // we will mark descend path as dirty, but final node will be marked and updated only in the corresponding path
        debug_assert!(self.root.is_empty() == false);
        let mut current_node = self.root;
        let (mut key, mut parent_branch_index) = loop {
            self.remove_from_cache(current_node);
            match self.descend_through_existing_nodes(path, current_node)? {
                DescendPath::PathDiverged {
                    alternative_node,
                    common_prefix_len,
                } => {
                    return self.make_diverging_case(&*path, alternative_node, common_prefix_len);
                }
                DescendPath::EmptyBranchTaken {
                    branch_node,
                    branch_index,
                } => {
                    debug_assert_eq!(branch_node, current_node);
                    return Ok(ValueInsertionStrategy::MakeLeafAttachedToBranch {
                        branch: branch_node,
                        branch_index,
                    });
                }
                DescendPath::LeafReached { .. } => {
                    return Err(());
                }
                DescendPath::EndReachedAtEmptyBranchValue {
                    final_branch_node,
                    branch_index,
                } => {
                    debug_assert_eq!(current_node, final_branch_node);
                    return Ok(ValueInsertionStrategy::MakeLeafAttachedToBranch {
                        branch: final_branch_node,
                        branch_index,
                    });
                }
                DescendPath::UnreferencedPathEncountered {
                    last_known_node,
                    branch_index,
                    next_key,
                } => {
                    debug_assert_eq!(last_known_node, current_node);

                    break (next_key, branch_index);
                }
                DescendPath::Follow { next_node, .. } => {
                    debug_assert_ne!(current_node, next_node);
                    current_node = next_node;
                }
            }
        };
        self.remove_from_cache(current_node);

        loop {
            debug_assert!(current_node.is_empty() == false);
            match self.descend_through_proof(
                path,
                key,
                current_node,
                preimages_oracle,
                interner,
                hasher,
            )? {
                AppendPath::PathDiverged { allocated_node } => {
                    debug_assert_ne!(current_node, allocated_node);
                    self.link_if_needed(current_node, parent_branch_index, allocated_node)?;
                    todo!();
                }
                AppendPath::EmptyBranchTaken { allocated_node, .. } => {
                    debug_assert_ne!(current_node, allocated_node);
                    self.link_if_needed(current_node, parent_branch_index, allocated_node)?;

                    todo!();
                }
                AppendPath::BranchTaken {
                    allocated_node,
                    branch_index,
                    next_key,
                } => {
                    debug_assert_ne!(current_node, allocated_node);
                    self.link_if_needed(current_node, parent_branch_index, allocated_node)?;
                    current_node = allocated_node;
                    parent_branch_index = branch_index;
                    key = next_key;
                }
                AppendPath::LeafReached { allocated_node, .. } => {
                    debug_assert_ne!(current_node, allocated_node);
                    self.link_if_needed(current_node, parent_branch_index, allocated_node)?;
                    return Err(());
                }
                AppendPath::BranchReached {
                    final_branch_node, ..
                } => {
                    debug_assert_ne!(current_node, final_branch_node);
                    self.link_if_needed(current_node, parent_branch_index, final_branch_node)?;
                    return Err(());
                }
                AppendPath::Follow {
                    allocated_node,
                    next_key,
                } => {
                    self.link_if_needed(current_node, parent_branch_index, allocated_node)?;
                    debug_assert_ne!(current_node, allocated_node);
                    current_node = allocated_node;
                    key = next_key;
                }
            }
        }
    }

    pub fn update(
        &mut self,
        path: Path<'_>,
        pre_encoded_value: &[u8],
        interner: &mut (impl Interner<'a> + 'a),
    ) -> Result<(), ()> {
        let final_node = self.find_terminal_node_for_update_or_delete(path)?;
        assert!(final_node.is_leaf());
        self.update_leaf_node(final_node, pre_encoded_value, interner)?;

        Ok(())
    }

    pub fn delete(&mut self, path: Path<'_>) -> Result<(), ()> {
        let final_node = self.find_terminal_node_for_update_or_delete(path)?;
        assert!(final_node.is_leaf());
        self.delete_leaf_node(final_node, path)
    }

    pub fn insert(
        &mut self,
        path: Path<'_>,
        pre_encoded_value: &[u8],
        preimages_oracle: &mut impl PreimagesOracle,
        interner: &mut (impl Interner<'a> + 'a),
        hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
    ) -> Result<(), ()> {
        self.insert_lazy_value(
            path,
            LeafValue::from_pre_encoded_with_interner(pre_encoded_value, interner)?,
            preimages_oracle,
            interner,
            hasher,
        )
    }

    pub fn insert_lazy_value(
        &mut self,
        mut path: Path<'_>,
        value: LeafValue<'a>,
        preimages_oracle: &mut impl PreimagesOracle,
        interner: &mut (impl Interner<'a> + 'a),
        hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
    ) -> Result<(), ()> {
        // find insertion point
        if self.root.is_empty() {
            let path_segment = interner.intern_slice(path.full_path())?;
            let leaf_node = LeafNode {
                cached_key: &[],
                path_segment,
                parent_node: NodeType::empty(),
                value,
            };
            self.root = self.push_leaf(leaf_node);

            return Ok(());
        }

        let original_path = path;
        // Path is now "eaten" to reflect anything that may exist in the trie before
        let insertion_strategy =
            self.find_insertion_strategy(&mut path, preimages_oracle, interner, hasher)?;
        match insertion_strategy {
            ValueInsertionStrategy::MakeLeafAttachedToBranch {
                branch,
                branch_index,
            } => self.insert_new_leaf_into_existing_branch(
                branch,
                branch_index,
                path,
                value,
                interner,
            )?,
            ValueInsertionStrategy::Split {
                alternative_path,
                parent_branch_or_empty,
                branch_index,
                common_prefix_len,
            } => {
                // it's recursive!
                let common_prefix = &path.prefix()[(path.prefix_len - common_prefix_len)..];
                self.temporary_split_existing(
                    parent_branch_or_empty,
                    branch_index,
                    alternative_path,
                    common_prefix,
                    interner,
                )?;

                self.insert_lazy_value(original_path, value, preimages_oracle, interner, hasher)?
            }
        }
        debug_assert_eq!(self.ensure_linked(), ());

        Ok(())
    }

    pub fn recompute(
        &mut self,
        preimages_oracle: &mut impl PreimagesOracle,
        interner: &mut (impl Interner<'a> + 'a),
        hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
    ) -> Result<(), ()> {
        debug_assert_eq!(self.ensure_linked(), ());

        if self.root.is_empty() {
            return Ok(());
        }

        if self.get_cached_key(self.root).is_empty() == false {
            return Ok(());
        }

        self.relink_if_needed(preimages_oracle, interner, hasher)?;

        debug_assert_eq!(self.ensure_linked(), ());

        let (_, new_root) = self.get_node_key(self.root, preimages_oracle, interner, hasher)?;

        self.interned_root_node_key = new_root;

        Ok(())
    }

    pub(crate) fn get_node_key(
        &mut self,
        node: NodeType,
        preimages_oracle: &mut impl PreimagesOracle,
        interner: &mut (impl Interner<'a> + 'a),
        hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
    ) -> Result<(bool, &'a [u8]), ()> {
        let (is_new, key) = if node.is_leaf() {
            self.get_leaf_key(node, interner, hasher)?
        } else if node.is_extension() {
            self.get_extension_key(node, preimages_oracle, interner, hasher)?
        } else if node.is_branch() {
            self.get_branch_key(node, preimages_oracle, interner, hasher)?
        } else if node.is_unreferenced_key() {
            self.get_unreferenced_key(node)?
        } else if node.is_opaque_nontrivial_root() {
            (false, self.interned_root_node_key)
        } else {
            return Err(());
        };

        // account that "key" is raw one, so it's RLP of some byte slice
        debug_assert!(key.len() <= 33, "key len is invalid for node {node:?}");

        Ok((is_new, key))
    }

    fn get_leaf_key(
        &mut self,
        leaf_node: NodeType,
        interner: &mut (impl Interner<'a> + 'a),
        hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
    ) -> Result<(bool, &'a [u8]), ()> {
        // Leaves are easy - they do not have children
        let leaf = &mut self.capacities.leaf_nodes[leaf_node.index()];
        if leaf.cached_key.is_empty() == false {
            return Ok((false, leaf.cached_key));
        }
        let path_for_nibbles = leaf.path_segment;
        let value = leaf.value.take_value();
        let new_key = interner.make_leaf_key_for_value(path_for_nibbles, value, hasher)?;
        leaf.cached_key = new_key;

        Ok((true, leaf.cached_key))
    }

    fn get_extension_key(
        &mut self,
        extension_node: NodeType,
        preimages_oracle: &mut impl PreimagesOracle,
        interner: &mut (impl Interner<'a> + 'a),
        hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
    ) -> Result<(bool, &'a [u8]), ()> {
        debug_assert!(
            self.capacities.extension_nodes[extension_node.index()]
                .path_segment
                .len()
                > 0,
            "extension has empty path with parent {:?} and child {:?}",
            self.capacities.extension_nodes[extension_node.index()].parent_node,
            self.capacities.extension_nodes[extension_node.index()].child_node
        );

        // unconditionally try to get key if the child - it may end up being cached recursively
        let child_node = self.capacities.extension_nodes[extension_node.index()].child_node;
        let (child_key_is_new, child_key) =
            self.get_node_key(child_node, preimages_oracle, interner, hasher)?;

        let cached_key = self.capacities.extension_nodes[extension_node.index()].cached_key;
        if cached_key.is_empty() == false && child_key_is_new == false {
            return Ok((false, cached_key));
        }

        // otherwise - recompute

        let extension = &mut self.capacities.extension_nodes[extension_node.index()];
        let new_key = interner.make_extension_key(extension.path_segment, child_key, hasher)?;
        extension.cached_key = new_key;

        Ok((true, extension.cached_key))
    }

    fn get_unreferenced_key(&mut self, unreferenced_key: NodeType) -> Result<(bool, &'a [u8]), ()> {
        // unreferenced keys just bear the key
        let known_key = self.capacities.unreferenced_keys[unreferenced_key.index()].cached_key;
        if known_key.is_empty() {
            panic!("Unreferenced branch {unreferenced_key:?} has unknown key");
        };

        Ok((false, known_key))
    }

    fn get_branch_key(
        &mut self,
        branch_node: NodeType,
        preimages_oracle: &mut impl PreimagesOracle,
        interner: &mut (impl Interner<'a> + 'a),
        hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
    ) -> Result<(bool, &'a [u8]), ()> {
        // walk over the children - maybe all of them are cached
        let child_nodes = self.capacities.branch_nodes[branch_node.index()].child_nodes;
        let mut new_keys = [EMPTY_SLICE_ENCODING; 16];
        let mut any_mutation = false;
        for (idx, child_node) in child_nodes.into_iter().enumerate() {
            if child_node.is_empty() == false {
                let (is_new_child_key, child_key) =
                    self.get_node_key(child_node, preimages_oracle, interner, hasher)?;
                new_keys[idx] = child_key;
                any_mutation |= is_new_child_key;
            }
        }

        // maybe it was never touched
        let cached_key = self.capacities.branch_nodes[branch_node.index()].cached_key;
        if cached_key.is_empty() == false && any_mutation == false {
            return Ok((false, cached_key));
        }

        // have to recompute
        let new_key = interner.make_branch_key(&new_keys, hasher)?;
        self.capacities.branch_nodes[branch_node.index()].cached_key = new_key;

        Ok((true, new_key))
    }
}
