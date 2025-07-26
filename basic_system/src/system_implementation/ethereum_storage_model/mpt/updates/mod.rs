use super::*;

mod delete_from_branch;
mod delete_leaf;
mod insert_new_leaf_into_branch;
mod make_branch_and_extension;
mod update_leaf_value;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum ExistingTerminalNode {
    Branch {
        branch: NodeType,
        branch_index: usize,
    },
    Leaf {
        leaf: NodeType,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum ValueInsertionStrategy {
    WriteIntoBranchValue {
        branch: NodeType,
        branch_index: usize,
    },
    MakeLeafAttachedToBranch {
        branch: NodeType,
        branch_index: usize,
    },
    MakeBranchAndExtension {
        alternative_path: NodeType,
        parent_branch_or_empty: NodeType,
        branch_index: usize,
        extension_len: usize,
    },
}

impl<'a, A: Allocator + Clone> EthereumMPT<'a, A> {
    // we will mark descend path as dirty, but final node will be marked and updated only in the corresponding path
    pub(crate) fn find_terminal_node_for_update_or_delete(
        &mut self,
        mut path: Path<'_>,
    ) -> Result<ExistingTerminalNode, ()> {
        let mut current_node = self.root;
        loop {
            self.keys_cache.remove(&current_node);
            match self.descend_through_existing_nodes(&mut path, current_node)? {
                DescendPath::PathDiverged { .. } => return Err(()),
                DescendPath::EmptyBranchTaken { .. } => return Err(()),
                DescendPath::LeafReached { final_node, .. } => {
                    debug_assert_eq!(current_node, final_node);
                    return Ok(ExistingTerminalNode::Leaf { leaf: final_node });
                }
                DescendPath::BranchReached {
                    final_branch_node,
                    branch_index,
                    ..
                } => {
                    debug_assert_eq!(current_node, final_branch_node);
                    return Ok(ExistingTerminalNode::Branch {
                        branch: final_branch_node,
                        branch_index,
                    });
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
            let node = &self.extension_nodes[alternative_node.index()];
            node.parent_node
        } else if alternative_node.is_leaf() {
            let node = &self.leaf_nodes[alternative_node.index()];
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
        Ok(ValueInsertionStrategy::MakeBranchAndExtension {
            alternative_path: alternative_node,
            parent_branch_or_empty: parent,
            branch_index,
            extension_len: common_prefix_len,
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
            self.keys_cache.remove(&current_node);
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
                DescendPath::BranchReached {
                    final_branch_node,
                    branch_index,
                    ..
                } => {
                    debug_assert_eq!(current_node, final_branch_node);
                    return Ok(ValueInsertionStrategy::WriteIntoBranchValue {
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
        self.keys_cache.remove(&current_node);

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
        _hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
    ) -> Result<(), ()> {
        let final_node = self.find_terminal_node_for_update_or_delete(path)?;
        match final_node {
            ExistingTerminalNode::Leaf { leaf } => {
                let _ = self.update_leaf_node(leaf, pre_encoded_value, interner)?;

                Ok(())
            }
            ExistingTerminalNode::Branch {
                branch,
                branch_index,
            } => {
                self.keys_cache.remove(&branch);

                let child = &mut self.branch_nodes[branch.index()].child_nodes[branch_index];
                if child.is_empty() {
                    // short and rare, can do right here
                    let mut interned_value = interner.intern_slice(pre_encoded_value)?;
                    let encoding = RLPSlice::parse(&mut interned_value)?;
                    if interned_value.is_empty() == false {
                        return Err(());
                    };
                    let new_opaque = OpaqueValue {
                        parent_node: branch,
                        branch_index,
                        encoding,
                    };
                    let index = self.branch_terminal_values.len();
                    self.branch_terminal_values.push(new_opaque);
                    *child = NodeType::terminal_value_in_branch(index);

                    Ok(())
                } else if child.is_terminal_value_in_branch() {
                    self.keys_cache.remove(&child);
                    // just update it
                    let existing_opaque = &mut self.branch_terminal_values[child.index()];
                    let mut interned_value = interner.intern_slice(pre_encoded_value)?;
                    let encoding = RLPSlice::parse(&mut interned_value)?;
                    if interned_value.is_empty() == false {
                        return Err(());
                    };
                    existing_opaque.encoding = encoding;

                    Ok(())
                } else {
                    Err(())
                }
            }
        }
    }

    pub fn delete(
        &mut self,
        mut path: Path<'_>,
        preimages_oracle: &mut impl PreimagesOracle,
        interner: &mut (impl Interner<'a> + 'a),
        hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
    ) -> Result<(), ()> {
        let final_node = self.find_terminal_node_for_update_or_delete(path)?;
        match final_node {
            ExistingTerminalNode::Leaf { leaf } => {
                self.delete_leaf_node(leaf, path, preimages_oracle, interner, hasher)
            }
            ExistingTerminalNode::Branch { branch, .. } => {
                path.seek_to_end();
                self.delete_from_branch_node(branch, path, preimages_oracle, interner, hasher)
            }
        }
    }

    pub fn insert(
        &mut self,
        mut path: Path<'_>,
        pre_encoded_value: &[u8],
        preimages_oracle: &mut impl PreimagesOracle,
        interner: &mut (impl Interner<'a> + 'a),
        hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
    ) -> Result<(), ()> {
        // find insertion point
        if self.root.is_empty() {
            let path_segment = interner.intern_slice(path.full_path())?;
            let mut value = interner.intern_slice(pre_encoded_value)?;
            let value = RLPSlice::parse(&mut value)?;
            let leaf_node = LeafNode {
                path_segment,
                parent_node: NodeType::empty(),
                raw_nibbles_encoding: &[], // it's a fresh one, so we do not benefit from it
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
                pre_encoded_value,
                interner,
            ),
            ValueInsertionStrategy::MakeBranchAndExtension {
                alternative_path,
                parent_branch_or_empty,
                branch_index,
                extension_len,
            } => {
                // it's recursive!()
                let extension = &path.prefix()[(path.prefix_len - extension_len)..];
                self.termorary_split_existing_as_extension_and_branch(
                    parent_branch_or_empty,
                    branch_index,
                    alternative_path,
                    extension,
                    interner,
                )?;

                self.insert(
                    original_path,
                    pre_encoded_value,
                    preimages_oracle,
                    interner,
                    hasher,
                )
            }
            ValueInsertionStrategy::WriteIntoBranchValue {
                branch,
                branch_index,
            } => {
                self.keys_cache.remove(&branch);

                // short and rare, can do right here
                let mut interned_value = interner.intern_slice(pre_encoded_value)?;
                let encoding = RLPSlice::parse(&mut interned_value)?;
                if interned_value.is_empty() == false {
                    return Err(());
                };
                let new_opaque = OpaqueValue {
                    parent_node: branch,
                    branch_index,
                    encoding,
                };
                let index = self.branch_terminal_values.len();
                self.branch_terminal_values.push(new_opaque);
                self.branch_nodes[branch.index()].child_nodes[branch_index] =
                    NodeType::terminal_value_in_branch(index);

                Ok(())
            }
        }
    }

    pub fn recompute(
        &mut self,
        interner: &mut (impl Interner<'a> + 'a),
        hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
    ) -> Result<(), ()> {
        if self.root.is_empty() {
            return Ok(());
        }
        let (_, new_root) = self.get_node_key(self.root, interner, hasher)?;
        debug_assert!(new_root.len() < 32 || new_root.len() == 33);
        self.interned_root_node_key = new_root;

        Ok(())
    }

    pub(crate) fn get_node_key(
        &mut self,
        node: NodeType,
        interner: &mut (impl Interner<'a> + 'a),
        hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
    ) -> Result<(bool, &'a [u8]), ()> {
        let (is_new, key) = if node.is_leaf() {
            self.get_leaf_key(node, interner, hasher)?
        } else if node.is_extension() {
            self.get_extension_key(node, interner, hasher)?
        } else if node.is_branch() {
            self.get_branch_key(node, interner, hasher)?
        } else if node.is_unreferenced_value_in_branch() {
            self.get_unreferenced_branch_key(node)?
        } else if node.is_terminal_value_in_branch() {
            self.get_terminal_branch_value_key(node, interner, hasher)?
        } else if node.is_opaque_nontrivial_root() {
            (false, self.interned_root_node_key)
        } else {
            return Err(());
        };

        debug_assert!(
            key.len() < 32 || key.len() == 33,
            "key len is invalid for node {node:?}"
        );

        Ok((is_new, key))
    }

    fn get_leaf_key(
        &mut self,
        leaf_node: NodeType,
        interner: &mut (impl Interner<'a> + 'a),
        hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
    ) -> Result<(bool, &'a [u8]), ()> {
        if let Some(known_key) = self.keys_cache.get(&leaf_node).copied() {
            debug_assert_ne!(known_key.len(), 32);
            Ok((false, known_key))
        } else {
            let leaf = &self.leaf_nodes[leaf_node.index()];
            let new_key =
                interner.make_leaf_key(leaf.path_segment, leaf.value.full_encoding(), hasher)?;
            debug_assert_ne!(new_key.len(), 32);
            self.keys_cache.insert(leaf_node, new_key);

            Ok((true, new_key))
        }
    }

    fn get_extension_key(
        &mut self,
        extension_node: NodeType,
        interner: &mut (impl Interner<'a> + 'a),
        hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
    ) -> Result<(bool, &'a [u8]), ()> {
        if let Some(known_key) = self.keys_cache.get(&extension_node).copied() {
            debug_assert_ne!(known_key.len(), 32);
            Ok((false, known_key))
        } else {
            let child_node = self.extension_nodes[extension_node.index()].child_node;
            let (_child_key_is_new, child_key) = self.get_node_key(child_node, interner, hasher)?;

            let extension = &self.extension_nodes[extension_node.index()];
            let new_key = interner.make_extension_key(
                extension.path_segment,
                extension.raw_nibbles_encoding,
                child_key,
                hasher,
            )?;
            debug_assert_ne!(new_key.len(), 32);
            self.keys_cache.insert(extension_node, new_key);

            Ok((true, new_key))
        }
    }

    fn get_terminal_branch_value_key(
        &mut self,
        terminal_branch_value: NodeType,
        interner: &mut (impl Interner<'a> + 'a),
        hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
    ) -> Result<(bool, &'a [u8]), ()> {
        if let Some(known_key) = self.keys_cache.get(&terminal_branch_value).copied() {
            debug_assert_ne!(known_key.len(), 32);
            Ok((false, known_key))
        } else {
            let existing_terminal_branch_value =
                &self.branch_terminal_values[terminal_branch_value.index()];
            let new_key = interner.make_terminal_branch_value_key(
                existing_terminal_branch_value.encoding.full_encoding(),
                hasher,
            )?;
            debug_assert_ne!(new_key.len(), 32);
            self.keys_cache.insert(terminal_branch_value, new_key);

            Ok((true, new_key))
        }
    }

    fn get_unreferenced_branch_key(
        &mut self,
        unreferenced_branch_value: NodeType,
    ) -> Result<(bool, &'a [u8]), ()> {
        let Some(known_key) = self.keys_cache.get(&unreferenced_branch_value).copied() else {
            panic!("Unreferenced branch {unreferenced_branch_value:?} has unknown key");
        };
        debug_assert_ne!(known_key.len(), 32);

        Ok((false, known_key))
    }

    fn get_branch_key(
        &mut self,
        branch_node: NodeType,
        interner: &mut (impl Interner<'a> + 'a),
        hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
    ) -> Result<(bool, &'a [u8]), ()> {
        // maybe it was never touched
        if let Some(known_key) = self.keys_cache.get(&branch_node).copied() {
            debug_assert_ne!(known_key.len(), 32);
            Ok((false, known_key))
        } else {
            // walk over the children
            let child_nodes = self.branch_nodes[branch_node.index()].child_nodes;
            let mut new_keys = [EMPTY_SLICE_ENCODING; 16];
            for (idx, child_node) in child_nodes.into_iter().enumerate() {
                if child_node.is_empty() == false {
                    let (_, child_key) = self.get_node_key(child_node, interner, hasher)?;
                    debug_assert_ne!(child_key.len(), 32);
                    new_keys[idx] = child_key;
                }
            }

            // have to recompute
            let new_key = interner.make_branch_key(&new_keys, hasher)?;
            self.keys_cache.insert(branch_node, new_key);
            debug_assert_ne!(new_key.len(), 32);

            Ok((true, new_key))
        }
    }
}
