use super::nodes::*;
use super::*;
use alloc::alloc::Allocator;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::fmt::Debug;
use crypto::MiniDigest;
use zk_ee::utils::Bytes32;

pub(crate) enum DescendPath<'a> {
    PathDiverged {
        alternative_node: NodeType,
        common_prefix_len: usize,
    },
    EmptyBranchTaken {
        branch_node: NodeType,
        branch_index: usize,
    },
    Follow {
        next_node: NodeType,
    },
    LeafReached {
        final_node: NodeType,
        value: RLPSlice<'a>,
    },
    BranchReached {
        final_branch_node: NodeType,
        branch_index: usize,
        value: RLPSlice<'a>,
    },
    UnreferencedPathEncountered {
        last_known_node: NodeType,
        branch_index: usize,
        next_key: RLPSlice<'a>,
    },
}

pub(crate) enum AppendPath<'a> {
    PathDiverged {
        allocated_node: NodeType,
    },
    EmptyBranchTaken {
        allocated_node: NodeType,
    },
    Follow {
        allocated_node: NodeType,
        next_key: RLPSlice<'a>,
    },
    BranchTaken {
        allocated_node: NodeType,
        branch_index: usize,
        next_key: RLPSlice<'a>,
    },
    LeafReached {
        allocated_node: NodeType,
        value: RLPSlice<'a>,
    },
    BranchReached {
        final_branch_node: NodeType,
        value: RLPSlice<'a>,
    },
}

/// Ethereum MPT implementation, that assumes constant-length paths of length at most 64 characters,
/// and hash function that outputs 32 bytes
#[derive(Debug)]
pub struct EthereumMPT<'a, A: Allocator + Clone> {
    pub(crate) root: NodeType,
    pub(crate) interned_root_node_key: &'a [u8], // We follow the same logic here - either hash, or short key
    // we want to store nodes separately
    pub(crate) leaf_nodes: Vec<LeafNode<'a>, A>,
    pub(crate) extension_nodes: Vec<ExtensionNode<'a>, A>,
    pub(crate) branch_nodes: Vec<BranchNode<'a>, A>,
    pub(crate) branch_unreferenced_values: Vec<OpaqueValue<'a>, A>,
    pub(crate) branch_terminal_values: Vec<OpaqueValue<'a>, A>,
    // We will cache preimages
    pub(crate) preimages_cache: BTreeMap<Bytes32, &'a [u8], A>,
    pub(crate) keys_cache: BTreeMap<NodeType, &'a [u8], A>,
}

impl<'a, A: Allocator + Clone> EthereumMPT<'a, A> {
    pub fn new_in(
        root_hash: [u8; 32],
        interner: &mut (impl Interner<'a> + 'a),
        allocator: A,
    ) -> Result<Self, ()> {
        let root = if root_hash == EMPTY_ROOT_HASH.as_u8_array() {
            NodeType::empty()
        } else {
            NodeType::opaque_nontrivial_root()
        };

        let interned_root_node_key = if root.is_empty() {
            EMPTY_SLICE_ENCODING
        } else {
            let mut buffer = interner.get_buffer(33)?;
            buffer.write_byte(0x80 + 32);
            buffer.write_slice(&root_hash);

            buffer.flush()
        };

        let new = Self {
            root,
            interned_root_node_key,
            leaf_nodes: Vec::new_in(allocator.clone()),
            extension_nodes: Vec::new_in(allocator.clone()),
            branch_nodes: Vec::new_in(allocator.clone()),
            branch_unreferenced_values: Vec::new_in(allocator.clone()),
            branch_terminal_values: Vec::new_in(allocator.clone()),
            preimages_cache: BTreeMap::new_in(allocator.clone()),
            keys_cache: BTreeMap::new_in(allocator.clone()),
        };

        Ok(new)
    }

    // we will not use a separate pre-fill of the tree to avoid
    pub fn get(
        &mut self,
        mut path: Path<'_>,
        preimages_oracle: &mut impl PreimagesOracle,
        interner: &mut (impl Interner<'a> + 'a),
        hasher: &mut crypto::sha3::Keccak256,
    ) -> Result<&'a [u8], ()> {
        if self.root.is_empty() {
            return Ok(&[]);
        }

        if self.root.is_opaque_nontrivial_root() {
            // allocate root, special case once
            let key = rlp_parse_short_bytes(self.interned_root_node_key)?;
            self.root = self.allocate_root_node_from_oracle(
                key,
                NodeType::empty(),
                preimages_oracle,
                interner,
                hasher,
            )?;
            self.keys_cache
                .insert(self.root, self.interned_root_node_key);
        }

        debug_assert_ne!(self.root, NodeType::empty());

        // descend
        let mut current_node = self.root;
        let (mut key, mut parent_branch_index) = loop {
            debug_assert!(current_node.is_empty() == false);
            match self.descend_through_existing_nodes(&mut path, current_node)? {
                DescendPath::PathDiverged { .. } => {
                    return Ok(&[]);
                }
                DescendPath::EmptyBranchTaken { branch_node, .. } => {
                    debug_assert_eq!(current_node, branch_node);
                    return Ok(&[]);
                }
                DescendPath::LeafReached { final_node, value } => {
                    debug_assert_eq!(current_node, final_node);
                    return Ok(value.data());
                }
                DescendPath::BranchReached {
                    final_branch_node,
                    value,
                    ..
                } => {
                    debug_assert_eq!(current_node, final_branch_node);
                    return Ok(value.data());
                }
                DescendPath::UnreferencedPathEncountered {
                    last_known_node,
                    branch_index,
                    next_key,
                } => {
                    debug_assert_eq!(current_node, last_known_node);
                    current_node = last_known_node;
                    break (next_key, branch_index);
                }
                DescendPath::Follow { next_node, .. } => {
                    debug_assert_ne!(current_node, next_node);
                    current_node = next_node;
                }
            }
        };

        debug_assert!(self.root.is_empty() == false);

        // continue to descend, but use oracle and verify proofs now
        loop {
            debug_assert!(current_node.is_empty() == false);
            match self.descend_through_proof(
                &mut path,
                key,
                current_node,
                preimages_oracle,
                interner,
                hasher,
            )? {
                AppendPath::PathDiverged { allocated_node } => {
                    debug_assert_ne!(current_node, allocated_node);
                    self.link_if_needed(current_node, parent_branch_index, allocated_node)?;
                    return Ok(&[]);
                }
                AppendPath::EmptyBranchTaken { allocated_node, .. } => {
                    debug_assert_ne!(current_node, allocated_node);
                    self.link_if_needed(current_node, parent_branch_index, allocated_node)?;
                    return Ok(&[]);
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
                AppendPath::LeafReached {
                    allocated_node,
                    value,
                } => {
                    debug_assert_ne!(current_node, allocated_node);
                    self.link_if_needed(current_node, parent_branch_index, allocated_node)?;
                    return Ok(value.data());
                }
                AppendPath::BranchReached {
                    final_branch_node,
                    value,
                    ..
                } => {
                    debug_assert_ne!(current_node, final_branch_node);
                    self.link_if_needed(current_node, parent_branch_index, final_branch_node)?;
                    return Ok(value.data());
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

    // Descend returns fully RLP-stripped slices - either final value,
    // or branch/extension raw key
    pub(crate) fn descend_through_existing_nodes(
        &self,
        path: &mut Path<'_>,
        current_node: NodeType,
    ) -> Result<DescendPath<'a>, ()> {
        if path.remaining_path().len() > 64 {
            return Err(());
        }
        if path.remaining_path().len() == 64 {
            debug_assert_eq!(current_node, self.root);
        }
        if current_node.is_leaf() {
            // we need to follow the path
            let existing_leaf = &self.leaf_nodes[current_node.index()];
            let common_prefix_len = path.follow_common_prefix(&existing_leaf.path_segment)?;
            if path.is_empty() {
                Ok(DescendPath::LeafReached {
                    final_node: current_node,
                    value: existing_leaf.value,
                })
            } else {
                Ok(DescendPath::PathDiverged {
                    alternative_node: current_node,
                    common_prefix_len,
                })
            }
        } else if current_node.is_extension() {
            let existing_extension = &self.extension_nodes[current_node.index()];
            let common_prefix_len = path.follow_common_prefix(&existing_extension.path_segment)?;
            if path.is_empty() {
                // Terminating extension
                Err(())
            } else if common_prefix_len == existing_extension.path_segment.len() {
                // we went thought all the extension
                let child_node = existing_extension.child_node;
                if child_node.is_unlinked() {
                    Ok(DescendPath::UnreferencedPathEncountered {
                        last_known_node: current_node,
                        branch_index: 0,
                        next_key: existing_extension.next_node_key,
                    })
                } else {
                    Ok(DescendPath::Follow {
                        next_node: child_node,
                    })
                }
            } else {
                Ok(DescendPath::PathDiverged {
                    alternative_node: current_node,
                    common_prefix_len,
                })
            }
        } else if current_node.is_branch() {
            let existing_branch = &self.branch_nodes[current_node.index()];
            let branch_index = path.take_branch()?;
            let child_node = existing_branch.child_nodes[branch_index];
            if path.is_empty() {
                if child_node.is_empty() {
                    Ok(DescendPath::BranchReached {
                        final_branch_node: current_node,
                        branch_index,
                        value: RLPSlice::empty(),
                    })
                } else if child_node.is_terminal_value_in_branch() {
                    let opaque = &self.branch_terminal_values[child_node.index()];
                    Ok(DescendPath::BranchReached {
                        final_branch_node: current_node,
                        branch_index,
                        value: opaque.encoding,
                    })
                } else {
                    Err(())
                }
            } else if child_node.is_empty() {
                Ok(DescendPath::EmptyBranchTaken {
                    branch_node: current_node,
                    branch_index,
                })
            } else if child_node.is_unreferenced_value_in_branch() {
                let opaque = &self.branch_unreferenced_values[child_node.index()];
                Ok(DescendPath::UnreferencedPathEncountered {
                    last_known_node: current_node,
                    branch_index,
                    next_key: opaque.encoding,
                })
            } else if child_node.is_branch() || child_node.is_extension() || child_node.is_leaf() {
                Ok(DescendPath::Follow {
                    next_node: child_node,
                })
            } else {
                Err(())
            }
        } else {
            Err(())
        }
    }

    fn consult_cache_or_oracle(
        &mut self,
        key: &'a [u8],
        preimages_oracle: &mut impl PreimagesOracle,
        interner: &mut (impl Interner<'a> + 'a),
        hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
    ) -> Result<&'a [u8], ()> {
        if key.len() < 32 {
            Ok(key)
        } else if key.len() == 32 {
            let key = Bytes32::from_array(key.try_into().expect("must be 32 bytes"));
            if let Some(known) = self.preimages_cache.get(&key).copied() {
                Ok(known)
            } else {
                let new = preimages_oracle.provide_preimage(key.as_u8_array_ref(), interner)?;
                hasher.update(new);
                let recomputed = hasher.finalize_reset();
                assert_eq!(recomputed, key.as_u8_array());
                self.preimages_cache.insert(key, new);

                Ok(new)
            }
        } else {
            Err(())
        }
    }

    fn allocate_root_node_from_oracle(
        &mut self,
        key: &'a [u8],
        parent_node: NodeType,
        preimages_oracle: &mut impl PreimagesOracle,
        interner: &mut (impl Interner<'a> + 'a),
        hasher: &mut crypto::sha3::Keccak256,
    ) -> Result<NodeType, ()> {
        let raw_encoding = self.consult_cache_or_oracle(key, preimages_oracle, interner, hasher)?;
        let (parsed_node, pieces) = parse_node_from_bytes(raw_encoding, interner)?;
        match parsed_node {
            ParsedNode::Leaf(mut leaf) => {
                assert_eq!(leaf.path_segment.len(), 64);
                leaf.parent_node = parent_node;
                let node_type = self.push_leaf(leaf);

                Ok(node_type)
            }
            ParsedNode::Extension(mut extension) => {
                assert_eq!(extension.path_segment.len(), 64);
                extension.parent_node = parent_node;
                let node_type = self.push_extension(extension);

                Ok(node_type)
            }
            ParsedNode::Branch(mut branch) => {
                for (branch_index, (child, encoding)) in
                    branch.child_nodes.iter_mut().zip(pieces.iter()).enumerate()
                {
                    if encoding.is_empty() {
                        *child = NodeType::empty()
                    } else {
                        // cache
                        let opaque = OpaqueValue {
                            parent_node,
                            branch_index,
                            encoding: *encoding,
                        };
                        let index = self.branch_unreferenced_values.len();
                        self.branch_unreferenced_values.push(opaque);
                        let node_type = NodeType::unreferenced_value_in_branch(index);
                        self.keys_cache.insert(node_type, encoding.full_encoding());
                        *child = node_type;
                    }
                }
                branch.parent_node = parent_node;
                let node_type = self.push_branch(branch);

                Ok(node_type)
            }
        }
    }

    // we return node type, and it's parsed "value", that is either terminal value,
    // or a "key" for next node
    pub(crate) fn descend_through_proof(
        &mut self,
        path: &mut Path<'_>,
        key: RLPSlice<'a>,
        parent_node: NodeType,
        preimages_oracle: &mut impl PreimagesOracle,
        interner: &mut (impl Interner<'a> + 'a),
        hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
    ) -> Result<AppendPath<'a>, ()> {
        if path.remaining_path().len() > 64 {
            return Err(());
        }
        let raw_encoding =
            self.consult_cache_or_oracle(key.data(), preimages_oracle, interner, hasher)?;
        let (parsed_node, pieces) = parse_node_from_bytes(raw_encoding, interner)?;
        match parsed_node {
            ParsedNode::Leaf(mut leaf) => {
                if !(parent_node.is_empty()
                    || parent_node.is_branch()
                    || parent_node.is_extension())
                {
                    return Err(());
                }
                leaf.parent_node = parent_node;
                let follows = path.follow(leaf.path_segment)?;
                let leaf_value = leaf.value;

                let index = self.leaf_nodes.len();
                self.leaf_nodes.push(leaf);
                let node_type = NodeType::leaf(index);
                self.keys_cache.insert(node_type, key.full_encoding());

                if follows {
                    Ok(AppendPath::LeafReached {
                        allocated_node: node_type,
                        value: leaf_value,
                    })
                } else {
                    Ok(AppendPath::PathDiverged {
                        allocated_node: node_type,
                    })
                }
            }
            ParsedNode::Extension(mut extension) => {
                if !(parent_node.is_empty() || parent_node.is_branch()) {
                    return Err(());
                }
                extension.parent_node = parent_node;
                let follows = path.follow(extension.path_segment)?;
                let next_node_key = extension.next_node_key;

                let index = self.extension_nodes.len();
                self.extension_nodes.push(extension);
                let node_type = NodeType::extension(index);
                self.keys_cache.insert(node_type, key.full_encoding());

                if follows {
                    Ok(AppendPath::Follow {
                        allocated_node: node_type,
                        next_key: next_node_key,
                    })
                } else {
                    Ok(AppendPath::PathDiverged {
                        allocated_node: node_type,
                    })
                }
            }
            ParsedNode::Branch(mut branch) => {
                if !(parent_node.is_empty()
                    || parent_node.is_extension()
                    || parent_node.is_branch())
                {
                    return Err(());
                }
                branch.parent_node = parent_node;
                let branch_index = path.take_branch()?;
                if branch_index >= 16 {
                    return Err(());
                }
                let index = self.branch_nodes.len();
                let inserted_node = NodeType::branch(index);
                if path.is_empty() {
                    let mut final_value = RLPSlice::empty();
                    // we still need to enumerate all branches
                    for (idx, (child_node, encoding)) in branch
                        .child_nodes
                        .iter_mut()
                        .zip(pieces[..16].iter())
                        .enumerate()
                    {
                        if encoding.is_empty() {
                            *child_node = NodeType::empty();
                        } else {
                            if idx == branch_index {
                                final_value = *encoding;
                            }
                            let index = self.branch_terminal_values.len();
                            let opaque = OpaqueValue {
                                parent_node: inserted_node,
                                branch_index: idx,
                                encoding: *encoding,
                            };
                            self.branch_terminal_values.push(opaque);
                            let node_type = NodeType::terminal_value_in_branch(index);
                            self.keys_cache.insert(node_type, encoding.full_encoding());
                            *child_node = node_type;
                        }
                    }
                    self.branch_nodes.push(branch);
                    self.keys_cache.insert(inserted_node, key.full_encoding());

                    Ok(AppendPath::BranchReached {
                        final_branch_node: inserted_node,
                        value: final_value,
                    })
                } else {
                    let mut next_node_key = RLPSlice::empty();
                    // we still need to enumerate all branches
                    for (idx, (child_node, encoding)) in branch
                        .child_nodes
                        .iter_mut()
                        .zip(pieces[..16].iter())
                        .enumerate()
                    {
                        if encoding.is_empty() {
                            *child_node = NodeType::empty();
                        } else {
                            if idx == branch_index {
                                next_node_key = *encoding;
                            }
                            let index = self.branch_unreferenced_values.len();
                            let opaque = OpaqueValue {
                                parent_node: inserted_node,
                                branch_index: idx,
                                encoding: *encoding,
                            };
                            self.branch_unreferenced_values.push(opaque);
                            let node_type = NodeType::unreferenced_value_in_branch(index);
                            self.keys_cache.insert(node_type, encoding.full_encoding());
                            *child_node = node_type;
                        }
                    }
                    self.branch_nodes.push(branch);
                    self.keys_cache.insert(inserted_node, key.full_encoding());

                    if next_node_key.is_empty() {
                        Ok(AppendPath::EmptyBranchTaken {
                            allocated_node: inserted_node,
                        })
                    } else {
                        Ok(AppendPath::BranchTaken {
                            allocated_node: inserted_node,
                            branch_index,
                            next_key: next_node_key,
                        })
                    }
                }
            }
        }
    }

    pub fn root(&self, hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>) -> [u8; 32] {
        if self.interned_root_node_key.len() == 33 {
            rlp_parse_short_bytes(self.interned_root_node_key)
                .unwrap()
                .try_into()
                .unwrap()
        } else {
            debug_assert!(
                self.interned_root_node_key.len() < 32,
                "root key len is {}",
                self.interned_root_node_key.len()
            );
            hasher.update(self.interned_root_node_key);
            hasher.finalize_reset()
        }
    }

    pub(crate) fn link_if_needed(
        &mut self,
        parent_node: NodeType,
        parent_branch_index: usize,
        child_node: NodeType,
    ) -> Result<(), ()> {
        if parent_node.is_branch() {
            // link
            let parent_branch_node = &mut self.branch_nodes[parent_node.index()];
            let branch_child = parent_branch_node.child_nodes[parent_branch_index];
            if branch_child.is_unreferenced_value_in_branch() {
                parent_branch_node.child_nodes[parent_branch_index] = child_node;
            } else if child_node != branch_child {
                // then it must be the same node, and we rely on indexing to do it
                return Err(());
            }
        } else if parent_node.is_extension() {
            let parent_extension_node = &mut self.extension_nodes[parent_node.index()];
            if parent_extension_node.child_node.is_unlinked() {
                parent_extension_node.child_node = child_node;
            } else if child_node != parent_extension_node.child_node {
                // then it must be the same node, and we rely on indexing to do it
                return Err(());
            }
        }

        Ok(())
    }

    #[inline(always)]
    pub(crate) fn push_leaf(&mut self, new_leaf: LeafNode<'a>) -> NodeType {
        let index = self.leaf_nodes.len();
        self.leaf_nodes.push(new_leaf);
        NodeType::leaf(index)
    }

    #[inline(always)]
    pub(crate) fn push_extension(&mut self, new_branch: ExtensionNode<'a>) -> NodeType {
        let index = self.extension_nodes.len();
        self.extension_nodes.push(new_branch);
        NodeType::extension(index)
    }

    #[inline(always)]
    pub(crate) fn push_branch(&mut self, new_branch: BranchNode<'a>) -> NodeType {
        let index = self.branch_nodes.len();
        self.branch_nodes.push(new_branch);
        NodeType::branch(index)
    }

    #[cfg(test)]
    pub(crate) fn ensure_linked(&self) {
        if self.root.is_empty() || self.root.is_opaque_nontrivial_root() {
            return;
        }
        self.ensure_linked_pair(NodeType::empty(), self.root);
    }

    #[cfg(test)]
    fn ensure_linked_pair(&self, parent: NodeType, child_node: NodeType) {
        if child_node.is_empty() {
            // nothing
            return;
        }
        let index = child_node.index();
        if child_node.is_leaf() {
            assert_eq!(self.leaf_nodes[index].parent_node, parent);
        } else if child_node.is_extension() {
            assert_eq!(self.extension_nodes[index].parent_node, parent);
            self.ensure_linked_pair(child_node, self.extension_nodes[index].child_node);
        } else if child_node.is_unlinked() {
            assert!(parent.is_extension())
        } else if child_node.is_branch() {
            assert_eq!(self.branch_nodes[index].parent_node, parent);
            for next_child in self.branch_nodes[index].child_nodes.into_iter() {
                self.ensure_linked_pair(child_node, next_child);
            }
        } else if child_node.is_terminal_value_in_branch() {
            assert!(parent.is_branch())
        } else if child_node.is_unreferenced_value_in_branch() {
            assert!(parent.is_branch())
        } else {
            panic!("Unknown pair {:?} -> {:?}", parent, child_node);
        }
    }
}
