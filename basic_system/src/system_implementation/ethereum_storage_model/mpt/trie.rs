use super::nodes::*;
use super::*;
use alloc::alloc::Allocator;
use alloc::collections::BTreeMap;
use core::fmt::Debug;
use crypto::MiniDigest;
use zk_ee::utils::Bytes32;

enum ProofPath<'a> {
    Diverged {
        allocated_node: NodeType,
    },
    Follow {
        allocated_node: NodeType,
        next_key: &'a [u8],
    },
    BranchTaken {
        allocated_node: NodeType,
        branch_index: usize,
        next_key: &'a [u8],
    },
    EndReached {
        allocated_node: NodeType,
        value: &'a [u8],
    },
    UnreferencedPathEncountered {
        last_known_node: NodeType,
        branch_index: usize,
        next_key: &'a [u8],
    },
}

#[derive(Debug)]
pub struct EthereumMPT<'a, A: Allocator + Clone> {
    pub(crate) root: NodeType,
    pub(crate) interned_root_hash: &'a [u8],
    // we want to store nodes separately
    pub(crate) leaf_nodes: Vec<LeafNode<'a>, A>,
    pub(crate) extension_nodes: Vec<ExtensionNode<'a>, A>,
    pub(crate) branch_nodes: Vec<BranchNode<'a>, A>,
    // We will cache preimages
    pub(crate) preimages_cache: BTreeMap<Bytes32, &'a [u8], A>,
}

impl<'a, A: Allocator + Clone> EthereumMPT<'a, A> {
    pub fn new_in(
        initial_root: &[u8],
        interner: &mut (impl Interner<'a> + 'a),
        allocator: A,
    ) -> Result<Self, ()> {
        let mut buffer = interner.get_buffer(32)?;
        buffer.write_slice(initial_root);
        let interned_root_hash = buffer.flush();

        let new = Self {
            root: NodeType::empty(),
            interned_root_hash,
            leaf_nodes: Vec::new_in(allocator.clone()),
            extension_nodes: Vec::new_in(allocator.clone()),
            branch_nodes: Vec::new_in(allocator.clone()),
            preimages_cache: BTreeMap::new_in(allocator.clone()),
        };

        Ok(new)
    }

    // we will not use a separate pre-fill of the tree to avoid
    pub fn access_initial_value(
        &mut self,
        mut path: Path<'_>,
        preimages_oracle: &mut impl PreimagesOracle,
        interner: &mut (impl Interner<'a> + 'a),
        hasher: &mut crypto::sha3::Keccak256,
    ) -> Result<&'a [u8], ()> {
        if path.remaining_path().len() != 64 {
            return Err(());
        }

        // TODO: change to constant
        if self.root.is_empty() {
            if self.interned_root_hash == EMPTY_ROOT_HASH.as_u8_ref() {
                return Ok(&[]);
            } else {
                // allocate root, special case once
                self.root = self.allocate_node_from_oracle(
                    self.interned_root_hash,
                    NodeType::empty(),
                    preimages_oracle,
                    interner,
                    hasher,
                )?;
            }
        }

        // descend
        let mut current_node = self.root;
        let (mut key, mut parent_branch_index)  = loop {
            match self.descend_through_existing_nodes(&mut path, current_node)? {
                ProofPath::Diverged { allocated_node } => {
                    return Ok(&[]);
                }
                ProofPath::BranchTaken {
                    allocated_node,
                    branch_index,
                    next_key,
                } => {
                    current_node = allocated_node;
                }
                ProofPath::EndReached {
                    allocated_node,
                    value,
                } => {
                    return Ok(value);
                }
                ProofPath::UnreferencedPathEncountered {
                    last_known_node,
                    branch_index,
                    next_key,
                } => {
                    current_node = last_known_node;
                    break (next_key, branch_index);
                }
                ProofPath::Follow {
                    allocated_node,
                    next_key,
                } => {
                    current_node = allocated_node;
                }
            }
        };

        // continue to descend, but use oracle and verify proofs now
        loop {
            match self.descend_through_proof(
                &mut path,
                key,
                current_node,
                preimages_oracle,
                interner,
                hasher,
            )? {
                ProofPath::Diverged { allocated_node } => {
                    self.link_if_needed(current_node, parent_branch_index, allocated_node)?;
                    return Ok(&[]);
                }
                ProofPath::BranchTaken {
                    allocated_node,
                    branch_index,
                    next_key,
                } => {
                    self.link_if_needed(current_node, parent_branch_index, allocated_node)?;
                    current_node = allocated_node;
                    parent_branch_index = branch_index;
                    key = next_key;
                }
                ProofPath::EndReached {
                    allocated_node,
                    value,
                } => {
                    self.link_if_needed(current_node, parent_branch_index, allocated_node)?;
                    return Ok(value);
                }
                ProofPath::UnreferencedPathEncountered {
                    last_known_node,
                    branch_index,
                    next_key,
                } => {
                    return Err(());
                }
                ProofPath::Follow {
                    allocated_node,
                    next_key,
                } => {
                    current_node = allocated_node;
                    key = next_key;
                }
            }
        }
    }

    fn descend_through_existing_nodes(
        &self,
        path: &mut Path<'_>,
        current_node: NodeType,
    ) -> Result<ProofPath<'a>, ()> {
        if path.remaining_path().len() > 64 {
            return Err(());
        }

        if current_node.is_leaf() {
            // we need to follow the path
            let existing_leaf = &self.leaf_nodes[current_node.index()];
            let follows = path.follow(&existing_leaf.path_segment)?;
            if follows {
                if path.is_empty() == false {
                    Err(())
                } else {
                    Ok(ProofPath::EndReached {
                        allocated_node: current_node,
                        value: existing_leaf.value,
                    })
                }
            } else {
                return Ok(ProofPath::Diverged {
                    allocated_node: current_node,
                });
            }
        } else if current_node.is_extension() {
            let existing_extension = &self.extension_nodes[current_node.index()];
            let follows = path.follow(&existing_extension.path_segment)?;
            if follows {
                if path.is_empty() {
                    Err(())
                } else {
                    Ok(ProofPath::Follow { 
                        allocated_node: current_node, 
                        next_key: existing_extension.next_node_key, 
                    })
                }
            } else {
                return Ok(ProofPath::Diverged {
                    allocated_node: current_node,
                });
            }
        } else if current_node.is_branch() {
            let existing_branch = &self.branch_nodes[current_node.index()];
            let branch_index = path.take_branch()?;
            let child_node = existing_branch.child_nodes[branch_index];
            if child_node.is_empty() {
                return Ok(ProofPath::Diverged {
                    allocated_node: child_node,
                });
            } else {
                let branch_raw_encoding = existing_branch.encoding_of_branch(branch_index);
                let next_key_encoding = rlp_parse_short_bytes(branch_raw_encoding)?;
                if child_node.is_unreferenced_path() {
                    // we should continue via oracle and proofs
                    Ok(
                        ProofPath::UnreferencedPathEncountered {
                            last_known_node: current_node,
                            branch_index,
                            next_key: next_key_encoding,
                        }
                    )
                } else {
                    Ok(ProofPath::Follow { 
                        allocated_node: current_node, 
                        next_key: next_key_encoding, 
                    })
                }
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
        hasher: &mut crypto::sha3::Keccak256,
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

    fn allocate_node_from_oracle(
        &mut self,
        key: &'a [u8],
        parent_node: NodeType,
        preimages_oracle: &mut impl PreimagesOracle,
        interner: &mut (impl Interner<'a> + 'a),
        hasher: &mut crypto::sha3::Keccak256,
    ) -> Result<NodeType, ()> {
        let raw_encoding = self.consult_cache_or_oracle(key, preimages_oracle, interner, hasher)?;
        match parse_node_from_bytes(raw_encoding, interner)? {
            ParsedNode::Leaf(mut leaf) => {
                let index = self.leaf_nodes.len();
                leaf.parent_node = parent_node;
                self.leaf_nodes.push(leaf);

                Ok(NodeType::leaf(index))
            }
            ParsedNode::Extension(mut extension) => {
                let index = self.extension_nodes.len();
                extension.parent_node = parent_node;
                self.extension_nodes.push(extension);

                Ok(NodeType::extension(index))
            }
            ParsedNode::Branch(mut branch) => {
                let index = self.branch_nodes.len();
                branch.parent_node = parent_node;
                self.branch_nodes.push(branch);

                Ok(NodeType::branch(index))
            }
        }
    }

    // we return node type, and it's parsed "value", that is either terminal value,
    // or a "key" for next node
    fn descend_through_proof(
        &mut self,
        path: &mut Path<'_>,
        key: &'a [u8],
        parent_node: NodeType,
        preimages_oracle: &mut impl PreimagesOracle,
        interner: &mut (impl Interner<'a> + 'a),
        hasher: &mut crypto::sha3::Keccak256,
    ) -> Result<ProofPath<'a>, ()> {
        if path.remaining_path().len() > 64 {
            return Err(());
        }
        let raw_encoding = self.consult_cache_or_oracle(key, preimages_oracle, interner, hasher)?;

        match parse_node_from_bytes(raw_encoding, interner)? {
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

                if follows {
                    Ok(ProofPath::EndReached {
                        allocated_node: node_type,
                        value: leaf_value,
                    })
                } else {
                    Ok(ProofPath::Diverged {
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

                if follows {
                    Ok(ProofPath::Follow {
                        allocated_node: node_type,
                        next_key: next_node_key,
                    })
                } else {
                    Ok(ProofPath::Diverged {
                        allocated_node: node_type,
                    })
                }
            }
            ParsedNode::Branch(mut branch) => {
                branch.parent_node = parent_node;
                let branch_index = path.take_branch()?;
                if branch_index >= 16 {
                    return Err(());
                }
                let child_node = branch.child_nodes[branch_index];
                let index = self.branch_nodes.len();
                if child_node.is_empty() {
                    self.branch_nodes.push(branch);
                    let node_type = NodeType::branch(index);
                    Ok(ProofPath::Diverged {
                        allocated_node: node_type
                    })
                } else {
                    debug_assert!(child_node.is_unreferenced_path());
                    let next_node_key = rlp_parse_short_bytes(branch.encoding_of_branch(branch_index))?;
                    self.branch_nodes.push(branch);
                    let node_type = NodeType::branch(index);

                    Ok(ProofPath::BranchTaken {
                        allocated_node: node_type,
                        branch_index,
                        next_key: next_node_key,
                    })
                }
            }
        }
    }

    pub fn root(&self) -> &'a [u8] {
        self.interned_root_hash
    }

    fn link_if_needed(
        &mut self,
        parent_node: NodeType,
        parent_branch_index: usize,
        child_node: NodeType,
    ) -> Result<(), ()> {
        if parent_node.is_branch() {
            // link
            let parent_branch_node = &mut self.branch_nodes[parent_node.index()];
            let branch_child = parent_branch_node.child_nodes[parent_branch_index];
            if branch_child.is_unlinked() || branch_child.is_unreferenced_path() {
                parent_branch_node.child_nodes[parent_branch_index] = child_node;
            } else {
                if child_node != branch_child {
                    // then it must be the same node, and we rely on indexing to do it
                    return Err(());
                }
            }
        } else if parent_node.is_extension() {
            let parent_extension_node = &mut self.extension_nodes[parent_node.index()];
            if parent_extension_node.child_node.is_unlinked() {
                parent_extension_node.child_node = child_node;
            } else {
                if child_node != parent_extension_node.child_node {
                    // then it must be the same node, and we rely on indexing to do it
                    return Err(());
                }
            }
        }

        Ok(())
    }
}