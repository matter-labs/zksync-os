use super::nodes::*;
use super::*;
use alloc::alloc::Allocator;
use alloc::collections::btree_map::Entry;
use alloc::collections::BTreeMap;
use core::fmt::Debug;
use core::ops::RangeBounds;
use crypto::MiniDigest;

#[inline]
pub(crate) fn consume<'a>(src: &mut &'a [u8], bytes: usize) -> Result<&'a [u8], ()> {
    let (data, rest) = src.split_at_checked(bytes).ok_or(())?;
    *src = rest;

    Ok(data)
}

pub(crate) fn rlp_parse_short_bytes<'a>(src: &'a [u8]) -> Result<&'a [u8], ()> {
    let mut data = src;
    let b0 = consume(&mut data, 1)?;
    let bb0 = b0[0];
    if bb0 >= 0xc0 {
        // it can not be a list
        return Err(());
    }
    if bb0 < 0x80 {
        if src.len() != 1 {
            return Err(());
        }
        Ok(src)
    } else if bb0 < 0xb8 {
        let expected_len = (bb0 - 0x80) as usize;
        if data.len() != expected_len {
            return Err(());
        }
        Ok(data)
    } else {
        Err(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct RLPSlice<'a> {
    full_encoding: &'a [u8],
    raw_data_offset: usize,
}

impl<'a> RLPSlice<'a> {
    pub(crate) const fn empty() -> Self {
        Self {
            full_encoding: &[],
            raw_data_offset: 0,
        }
    }
    pub(crate) const fn full_encoding(&self) -> &'a [u8] {
        self.full_encoding
    }

    pub(crate) fn data(&self) -> &'a [u8] {
        // we pre-validated
        unsafe { self.full_encoding.get_unchecked(self.raw_data_offset..) }
    }

    pub(crate) const fn is_empty(&self) -> bool {
        self.raw_data_offset == self.full_encoding.len()
    }

    #[track_caller]
    pub(crate) fn parse(data: &mut &'a [u8]) -> Result<Self, ()> {
        let data_start = data.as_ptr();
        let b0 = consume(data, 1)?;
        let bb0 = b0[0];
        let mut raw_data_offset = 1;
        if bb0 >= 0xc0 {
            // it can not be a list
            return Err(());
        }
        if bb0 < 0x80 {
            raw_data_offset -= 1;
            // fallthrough
        } else if bb0 < 0xb8 {
            let expected_len = (bb0 - 0x80) as usize;
            let _ = consume(data, expected_len)?;
        } else if bb0 < 0xc0 {
            let length_encoding_length = (bb0 - 0xb7) as usize;
            let length_encoding_bytes = consume(data, length_encoding_length)?;
            raw_data_offset += length_encoding_length;
            if length_encoding_bytes.len() > 2 {
                return Err(());
            }
            let mut be_bytes = [0u8; 4];
            be_bytes[(4 - length_encoding_bytes.len())..].copy_from_slice(length_encoding_bytes);
            let length = u32::from_be_bytes(be_bytes) as usize;
            let _ = consume(data, length)?;
        } else {
            return Err(());
        }

        Ok(Self {
            full_encoding: unsafe { core::slice::from_ptr_range(data_start..data.as_ptr()) },
            raw_data_offset,
        })
    }
}

// We want to map path (key) into short index, that can itself be used to access
// key and value
#[derive(Clone, Debug)]
pub(crate) struct BidirectionalMap<
    K: Ord + Eq + Clone + Debug,
    V: Eq + Clone + Debug,
    A: Allocator + Clone,
> {
    map: BTreeMap<K, usize, A>,
    storage: Vec<(K, V), A>,
}

impl<K: Ord + Eq + Clone + Debug, V: Eq + Clone + Debug, A: Allocator + Clone>
    BidirectionalMap<K, V, A>
{
    pub(crate) fn new_in(allocator: A) -> Self {
        Self {
            map: BTreeMap::new_in(allocator.clone()),
            storage: Vec::new_in(allocator),
        }
    }

    pub(crate) fn get_mut(&'_ mut self, key: &K) -> Option<(&'_ mut (K, V), usize)> {
        if let Some(pos) = self.map.get(key).copied() {
            let kv = &mut self.storage[pos];

            Some((kv, pos))
        } else {
            None
        }
    }

    #[inline]
    pub(crate) fn insert_considering_unique_persistent(
        &mut self,
        key: K,
        value: V,
    ) -> Result<usize, ()> {
        match self.map.entry(key.clone()) {
            Entry::Occupied(occupied) => {
                let pos = *occupied.get();
                let (_, v) = &self.storage[pos];
                if v == &value {
                    Ok(pos)
                } else {
                    Err(())
                }
            }
            Entry::Vacant(vacant) => {
                let pos = self.storage.len();
                self.storage.push((key.clone(), value));
                vacant.insert(pos);

                Ok(pos)
            }
        }
    }

    #[inline]
    pub(crate) fn insert_unique_persistent_prechecked(&mut self, key: K, value: V) -> usize {
        let pos = self.storage.len();
        self.storage.push((key.clone(), value));
        let existing = self.map.insert(key, pos);
        assert!(existing.is_none());

        pos
    }

    #[inline]
    pub(crate) fn get_persistent_by_index(&'_ self, index: usize) -> &'_ (K, V) {
        &self.storage[index]
    }

    #[inline]
    pub(crate) fn get_persistent_mut_by_index(&'_ mut self, index: usize) -> &'_ mut (K, V) {
        &mut self.storage[index]
    }

    #[inline]
    pub(crate) fn get_with_cache_check(&'_ self, index: usize) -> Option<&'_ (K, V)> {
        let kv = &self.storage[index];
        if let Some(pos) = self.map.get(&kv.0) {
            if *pos == index {
                Some(kv)
            } else {
                None
            }
        } else {
            None
        }
    }

    #[inline]
    pub(crate) fn contains_key(&self, key: &K) -> bool {
        self.map.contains_key(key)
    }

    #[inline]
    pub(crate) fn remove_persisted(&mut self, index: usize) -> Option<(K, V)> {
        let (key, value) = &self.storage[index];
        if let Some((key, pos)) = self.map.remove_entry(key) {
            if pos == index {
                Some((key, value.clone()))
            } else {
                None
            }
        } else {
            None
        }
    }

    pub(crate) fn range<'a, R>(&'a self, range: R) -> alloc::collections::btree_map::Range<'a, K, usize> where R: RangeBounds<K> {
        self.map.range(range)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ShortNodeIndex<'a> {
    // in total it should guarantee uniqueness:
    // prefix describes the path
    pub(crate) node_prefix: &'a [u8],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ShortBranchPathIndex<'a> {
    // same uniquenss guarantees
    pub(crate) branch_node_prefix: &'a [u8],
    pub(crate) branch_index: usize,
}

#[derive(Debug)]
pub struct EthereumMPT<'a, A: Allocator + Clone> {
    // pub(crate) unique_nodes: BTreeMap<ShortNodeIndex<'a>, NodeType, A>,
    pub(crate) leaf_nodes: BidirectionalMap<ShortNodeIndex<'a>, LeafNode<'a>, A>,
    pub(crate) extension_nodes: BidirectionalMap<ShortNodeIndex<'a>, (&'a [u8], ExtensionNode<'a>), A>,
    pub(crate) branch_nodes: BidirectionalMap<ShortNodeIndex<'a>, (&'a [u8], BranchNode<'a>), A>,
    // pub(crate) branches: BidirectionalMap<ShortBranchPathIndex<'a>, A>,
    pub(crate) untaken_branchs: BidirectionalMap<ShortBranchPathIndex<'a>, UnreferencedPath<'a>, A>,
    pub(crate) root: NodeType,
    pub(crate) root_raw_encoding: &'a [u8],
}

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
}

impl<'a, A: Allocator + Clone> EthereumMPT<'a, A> {
    pub fn new_in(allocator: A) -> Self {
        Self {
            // unique_nodes: BTreeMap::new_in(allocator.clone()),
            leaf_nodes: BidirectionalMap::new_in(allocator.clone()),
            extension_nodes: BidirectionalMap::new_in(allocator.clone()),
            branch_nodes: BidirectionalMap::new_in(allocator.clone()),
            // taken_branches: BidirectionalMap::new_in(allocator.clone()),
            untaken_branchs: BidirectionalMap::new_in(allocator.clone()),
            root: NodeType::empty(),
            root_raw_encoding: &[],
        }
    }

    // we return node type, and it's parsed "value", that is either terminal value,
    // or a "key" for next node
    fn descend_with_proof(
        &mut self,
        path: &mut Path<'a>,
        key: &'a [u8],
        raw_encoding: &'a [u8],
        parent_node: NodeType,
        interner: &'_ mut (impl Interner<'a> + 'a),
    ) -> Result<ProofPath<'a>, ()> {
        if path.remaining_path().len() > 64 {
            return Err(());
        }
        // we should also consider cases when we prove that value is not in the tree. For well-formed case we should only support the following cases
        // - we parse branch node, and one of the branches encodes empty value
        // - we follow some path, and it ends, but our path is not yet empty - it'll be handled at the level above

        if path.prefix_len() == 0 {
            assert!(parent_node.is_empty());
            // we are that the root
            if key.len() == 0 {
                // we do not have expectations, no root yet
            } else {
                if raw_encoding != key {
                    return Err(());
                }
            }
        } else {
            if key.len() < 32 {
                if key != raw_encoding {
                    return Err(());
                }
            } else if key.len() == 32 {
                let hashed = crypto::sha3::Keccak256::digest(raw_encoding);
                if key != &hashed {
                    return Err(());
                }
            } else {
                return Err(());
            }
        }

        let (num_filled, pieces, num_non_empty_branches) = Self::parse_initial(raw_encoding)?;

        if num_filled == 2 {
            // leaf or extension
            // nibbles bytes(!) have to be re-interpreted at hex-chars(!), and then matched against the path
            let nibbles_encoding = pieces[0];
            let nibbles = nibbles_encoding.data();
            let (path_segment, is_leaf) = interner.intern_nibbles(nibbles)?;
            if is_leaf == false {
                // extension
                if !(parent_node.is_empty() || parent_node.is_branch()) {
                    return Err(());
                }
                let prefix = path.prefix();
                let follows = path.follow(path_segment)?;
                let next_node_key = pieces[1].data();
                // index is only a path towards the node
                let short_index = ShortNodeIndex {
                    node_prefix: prefix,
                };

                // check if we went over this extension before
                let node_type = if let Some(((_, (existing_key, existing_extension_node)), index)) =
                    self.extension_nodes.get_mut(&short_index)
                {
                    if *existing_key != key {
                        return Err(());
                    }
                    NodeType::extension(index)
                } else {
                    let extension_node = ExtensionNode {
                        // key: key,
                        // prefix,
                        path_segment,
                        parent_node,
                        child_node: NodeType::unlinked(), // will be re-linked
                        raw_nibbles_encoding: nibbles_encoding.full_encoding(),
                        // raw_encoding,
                        // next_node_key,
                    };

                    let index = self
                        .extension_nodes
                        .insert_considering_unique_persistent(short_index, (key, extension_node))?;
                    NodeType::extension(index)
                };

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
            } else {
                if !(parent_node.is_empty()
                    || parent_node.is_branch()
                    || parent_node.is_extension())
                {
                    return Err(());
                }
                let prefix = path.prefix();
                let follows = path.follow(path_segment)?;
                // we can show the leaf that is just the closest one to the path we want,
                // so we do not check that we consumed all the path
                if follows && path.is_empty() == false {
                    return Err(());
                }
                let leaf_value = pieces[1].data();
                // index is only a path towards the node
                let short_index = ShortNodeIndex {
                    node_prefix: prefix,
                };

                let leaf_node = LeafNode {
                    // key: key,
                    // prefix,
                    path_segment,
                    raw_nibbles_encoding: nibbles_encoding.full_encoding(),
                    parent_node,
                    // raw_encoding,
                    // value: leaf_value,
                };

                let index = self
                    .leaf_nodes
                    .insert_considering_unique_persistent(short_index, leaf_node)?;
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
        } else if num_filled == 17 {
            // branch
            if pieces[16].is_empty() == false {
                // can not have a value in our applications
                return Err(());
            }
            if num_non_empty_branches < 2 {
                return Err(());
            }

            // it is a branch, and we must parse it in full, but only take a single path that we are interested in. We do not need to
            // verify well-formedness of branches too much, just to the extend that they are short enough
            let branch_node_prefix = path.prefix();
            let branch_prefix_as_path = path.prefix();
            let branch_index = path.take_branch()?;
            if branch_index >= 16 {
                return Err(());
            }
            let next_node_key = pieces[branch_index].data();
            let short_index = ShortNodeIndex {
                node_prefix: branch_node_prefix,
            };

            // for child nodes the process to insert and update is more complex
            let node_type = if let Some(((_, (existing_key, existing_branch_node)), index)) =
                self.branch_nodes.get_mut(&short_index)
            {
                if *existing_key != key {
                    return Err(());
                }
                // update branch by preparing to re-link
                if existing_branch_node.child_nodes[branch_index].is_unreferenced_path() {
                    // mark to re-link
                    existing_branch_node.child_nodes[branch_index] = NodeType::unlinked();
                }
                NodeType::branch(index)
            } else {
                // we insert it as a fresh one
                // it is fresh branch node
                let mut child_nodes = [NodeType::unknown_branch(); 16];
                let mut child_encoding_lengths = [0u8; 16];
                // a little unsafe, but why not
                let branches_encodings_concatenation = unsafe {
                    core::slice::from_ptr_range(pieces[0].full_encoding().as_ptr()..pieces[15].full_encoding().as_ptr_range().end)
                };

                for (idx, branch_value) in pieces[..16].iter().enumerate() {
                    if branch_value.data().len() > 32 {
                        return Err(());
                    }
                    child_encoding_lengths[idx] = branch_value.full_encoding().len() as u8;
                    if idx != branch_index {
                        if branch_value.is_empty() {
                            child_nodes[idx] = NodeType::empty();
                        }
                    } else {
                        // relink
                        child_nodes[idx] = NodeType::unlinked()
                    }
                }
                let branch_node = BranchNode {
                    // key,
                    // prefix: branch_prefix_as_path,
                    parent_node,
                    child_nodes,
                    branches_encodings_concatenation,
                    child_encoding_lengths,
                    // raw_encoding,
                };
                let index = self
                    .branch_nodes
                    .insert_considering_unique_persistent(short_index, (key, branch_node))?;

                NodeType::branch(index)
            };

            Ok(ProofPath::BranchTaken {
                allocated_node: node_type,
                branch_index,
                next_key: next_node_key,
            })
        } else {
            return Err(());
        }
    }

    pub fn root(&self) -> &'a [u8] {
        self.root_raw_encoding
    }

    fn link_if_needed(
        &mut self,
        parent_node: NodeType,
        parent_branch_index: usize,
        child_node: NodeType,
    ) -> Result<(), ()> {
        if parent_node.is_branch() {
            // link
            let (_, (_, parent_branch_node)) = self
                .branch_nodes
                .get_persistent_mut_by_index(parent_node.index());
            let branch_child =
                parent_branch_node.child_nodes[parent_branch_index];
            if branch_child.is_unlinked() || branch_child.is_unreferenced_path() {
                parent_branch_node.child_nodes[parent_branch_index] =
                    child_node;
            } else {
                if child_node != branch_child {
                    // then it must be the same node, and we rely on indexing to do it
                    return Err(());
                }
            }
        } else if parent_node.is_extension() {
            let (_, (_, parent_extension_node)) = self
                .extension_nodes
                .get_persistent_mut_by_index(parent_node.index());
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

    pub fn insert_proof(
        &mut self,
        full_path: &'a [u8],
        proof: impl Iterator<Item = &'a [u8]>,
        interner: &'_ mut (impl Interner<'a> + 'a),
    ) -> Result<&'a [u8], ()> {
        // this one we will follow, and it'll be consumed gradually
        let mut path = Path::new(full_path);

        let mut parent_node = NodeType::empty();
        // we consider that first node doesn't have a "key", but take special case in this case
        // in all the functions below
        let mut key = &[][..];
        let mut parent_branch_index = 0;

        for layer in proof {
            if key.is_empty() {
                if parent_node.is_empty() == false {
                    return Err(());
                }
            }
            if parent_node.is_empty() {
                if self.root.is_empty() == false {
                    if self.root_raw_encoding != layer {
                        return Err(());
                    }
                }
            }
            if parent_node.is_leaf() {
                return Err(());
            }
            if self.root.is_empty() {
                self.root_raw_encoding = layer;
            }
            'inner: loop {
                match self.descend_with_proof(&mut path, key, layer, parent_node, interner)? {
                    ProofPath::Diverged { allocated_node } => {
                        self.link_if_needed(
                            parent_node,
                            parent_branch_index,
                            allocated_node
                        )?;

                        // Non-inclusion
                        if self.root.is_empty() {
                            self.root = allocated_node;
                        }
                        // TODO: consider if we want to check that proof iterator is exhausted - it may be
                        // good for well-formedness of proofs, but not strictly needed to make a trie itself
                        return Ok(&[]);
                    }
                    ProofPath::Follow {
                        allocated_node,
                        next_key,
                    } => {
                        // we took an extension
                        self.link_if_needed(
                            parent_node,
                            parent_branch_index,
                            allocated_node
                        )?;

                        // follow
                        if self.root.is_empty() {
                            self.root = allocated_node;
                        }
                        parent_node = allocated_node;
                        key = next_key;
                    }
                    ProofPath::EndReached {
                        allocated_node,
                        value,
                    } => {
                        // we took an matching leaf
                        self.link_if_needed(
                            parent_node,
                            parent_branch_index,
                            allocated_node
                        )?;

                        // terminate
                        assert!(path.is_empty());
                        if self.root.is_empty() {
                            self.root = allocated_node;
                        }
                        // TODO: consider if we want to check that proof iterator is exhausted - it may be
                        // good for well-formedness of proofs, but not strictly needed to make a trie itself
                        return Ok(value);
                    }
                    ProofPath::BranchTaken {
                        allocated_node,
                        branch_index,
                        next_key,
                    } => {
                        // we took a branch
                        self.link_if_needed(
                            parent_node,
                            parent_branch_index,
                            allocated_node
                        )?;

                        // follow
                        if self.root.is_empty() {
                            self.root = allocated_node;
                        }
                        parent_branch_index = branch_index;
                        parent_node = allocated_node;
                        key = next_key;
                    }
                }
                if key.len() == 0 || key.len() == 32 {
                    // should peek into next proof element, or terminate, otherwise we will parse again
                    break 'inner;
                }
            }
        }
        if self.root.is_empty() {
            self.root = parent_node;
        }
        if parent_node.is_branch() {
            if path.is_empty() {
                // we ended up with a branch at the very bottom
                Ok(key)
            } else {
                // Non-inclusion
                Ok(&[])
            }
        } else {
            // end of proof is reached, but we didn't either diverge or terminate
            Err(())
        }
    }

    #[inline]
    fn parse_initial(raw_encoding: &'a [u8]) -> Result<(usize, [RLPSlice<'a>; 17], usize), ()> {
        // we try to insert node encoding and see if it exists
        if raw_encoding.len() < 3 {
            return Err(());
        }
        let mut data = raw_encoding;
        let b0 = consume(&mut data, 1)?;
        let b0 = b0[0];
        // we can not make any conclusion based on the first byte. At best we can make a decision that it's a list,
        // but not even the number of elements in it...
        if b0 < 0xc0 {
            return Err(());
        }
        if b0 < 0xf8 {
            // list of unknown(!) length, even though the concatenation is short. Yes, we can not make a decision about
            // validity until we parse the full encoding, but at least let's reject some trivial cases
            let expected_len = b0 - 0xc0;
            if data.len() != expected_len as usize {
                return Err(());
            }
            // either it's a leaf/extension that is a list of two, or branch
            let mut pieces = [RLPSlice::empty(); 17];
            let mut filled = 0;
            let mut num_non_empty_branches = 0;
            for dst in pieces.iter_mut() {
                // and itself it must be a string, not a list
                *dst = RLPSlice::parse(&mut data)?;
                if dst.is_empty() == false {
                    num_non_empty_branches += 1;
                }
                filled += 1;
                if data.is_empty() {
                    break;
                }
            }
            if data.is_empty() == false {
                return Err(());
            }

            Ok((filled, pieces, num_non_empty_branches))
        } else {
            // list of large length. But we do not expect it "too large"
            let length_encoding_length = (b0 - 0xf7) as usize;
            let length_encoding_bytes = consume(&mut data, length_encoding_length)?;
            if length_encoding_bytes.len() > 2 {
                return Err(());
            }
            let mut be_bytes = [0u8; 4];
            be_bytes[(4 - length_encoding_bytes.len())..].copy_from_slice(length_encoding_bytes);
            let length = u32::from_be_bytes(be_bytes) as usize;
            if data.len() != length {
                return Err(());
            }
            let mut pieces = [RLPSlice::empty(); 17];
            let mut filled = 0;
            let mut num_non_empty_branches = 0;
            for dst in pieces.iter_mut() {
                // and itself it must be a string, not a list, and can not be longer than 32 bytes
                *dst = RLPSlice::parse(&mut data)?;
                if dst.is_empty() == false {
                    num_non_empty_branches += 1;
                }
                filled += 1;
                if data.is_empty() {
                    break;
                }
            }
            if data.is_empty() == false {
                return Err(());
            }

            Ok((filled, pieces, num_non_empty_branches))
        }
    }

    // pub fn get_element(&self, full_path: &[u8]) -> Result<Option<(NodeType, &'a [u8])>, ()> {
    //     if self.root.is_empty() {
    //         return Err(());
    //     }
    //     let mut root = self.root;
    //     let mut path = full_path;
    //     loop {
    //         if root.is_leaf() {
    //             let (_, node) = self.leaf_nodes.get_persistent_by_index(root.index());
    //             let node_path = node.path;
    //             if path != node_path.segment() {
    //                 // Leaf, but another one
    //                 return Ok(None);
    //             } else {
    //                 return Ok(Some((root, node.value)));
    //             }
    //         } else if root.is_branch() {
    //             if path.len() < 1 {
    //                 return Err(());
    //             }
    //             let branch_index = Path::path_char_to_digit(path[0]);
    //             if branch_index >= 16 {
    //                 return Err(());
    //             }
    //             path = &path[1..];
    //             let (_, node) = self.branch_nodes.get_persistent_by_index(root.index());
    //             let child_node = node.child_nodes[branch_index as usize];
    //             root = child_node;
    //         } else if root.is_extension() {
    //             let (_, node) = self.extension_nodes.get_persistent_by_index(root.index());
    //             let node_path = node.path;
    //             if path.starts_with(node_path.segment()) == false {
    //                 return Ok(None);
    //             } else {
    //                 root = node.child_node;
    //                 path = &path[node_path.segment().len()..];
    //             }
    //         } else if root.is_unreferenced_path() {
    //             return Err(());
    //         } else if root.is_empty() {
    //             return Ok(None);
    //         }
    //     }
    // }
}

#[cfg(test)]
mod test {
    use crate::system_implementation::ethereum_storage_model::mpt::BoxInterner;

    use super::*;
    use std::alloc::Global;

    // #[test]
    // fn parse_leaf() {
    //     let proof = vec![
    //             "0xf90211a018d6c6b28333daa5b9b0de77a587ebae880674def2a7878decb3ce8d2cbccd7da0450be2f3b4aa3f4e1b88c310134fe88533b9bbc6c5726e85efd1bfe944ac9158a0740f74da7ec67bbc0a6fc0429269e9aa9dc80db6fee96eef96db1812a0e2d327a06bc1617a8826a28f4c72ab3638f6fdba56d055b2ac3222d01e7b820f94e0eedfa0e93b21d3f73f4afc147cb9ad700110b175dad415419f9377f9f1c6b2681237efa04ff6fdabb0444d612ecc108f95d324da3e713cdbd4465a0cc71e67bff4872667a01f78e5e5eba52ec7a4a77c8af83b2325b1178f3309a4b26b1e866cfd60e1f98fa0d20a9849469d5b71edc193e5ebcea34730fe373bf4c96e45980ae26ebbc29efea002915348d9312cea04f300c49f37d4560626f78bda5f84558e6cf24589162fc2a09ec88734afaf01b65905665483ee05288436d901f973af09b9a75e3dbf992979a0902973f5cdec39260b9c9ce4f6bfd9381d37c9b85e43ebf5ebf5da2f78b12d18a026aaa39a63de4fb2bbcbe6b7df606b2f2843c959c7e123b9a4369811974dabfda08a6fadd0ec139c7c86647220e6e39a38cc5665e22b362b4936da3df733c46f22a02e3440883087c708a906e81a56610a86159e2623ef0b2e705f3a42220412af85a035c0a5ac653b5bc37648b57a49cb775940bc639bb5cae2669c8bef590d988b5aa033a23e9a565ae0a78c9c4ee36bebbbc964fce5cc4b75f4f2e32bb87d8f919b7680",
    //             "0xf90211a041252e075fa3ab3808a717489b0e1eada594198d7c1df5ff0fd57fa83ab9e91fa014878fa3de5b28ff2de84e850291ea8f61b27b040849525e9d0d8bbc9b8f23ada090d457c802bdeccaa3d0b2adc48ab157cca13dcd8ccd6afad511e99c614122fca0e85e0fec0094759d022334c16c08154c7c9dd8e03c6d1452f5519b7644d4ac2ba0063daa2d1aa24e342d4138b9a20d80c53e2db99066d69bc4d636a71f5fe6196da078748899234384aa7a3252d8a6d3ecc1e4c61877858aca50425827891ee97cfea0f2d97b2d1319e8f43be58f7a97bf685993a340ff79d5ded186eea82e8db1f7e1a078a3856568b05657c8719b914e53ac4ec26918b53682097ebe28ddd3a5baba5ea0ed00ae52e4ebab05a4d8a71f78bea3111757f83869dc985154666501f56c8d2fa0cadae92038de11a7f5b3e500d5b39ad925fff85f5b5e35b54925a48fa66bbce8a0bf0963f23952f8cffb505eabf5b3ac792de224737f154ff75cd8ac05ffebb557a0c0833076c017a2e903c420d184ffaad64acd5d4166a8083df2fcfdde912f9a50a054163d2a787439154d7dc77e5bbaf82645879e14f3b9b34910d93aed5a9305b4a02e849550df28ca6d1be6afade961d7da22561c32d78dac3630fdfde52215b46fa02604b09381761c60be5579d7e31b749b931ca14cb53c202e7066b89a58302714a08f7ee2ed381014ab0a2600ff4389f3b586ee39fa4ed3aa5c34a643880937b94080",
    //             "0xf90211a0050d77f309ad467d694d7742ef37a9c56a8fbd733943e8303905ccd2b5c5a31aa0890178cfe9134aa1efba9cb04fcb465f485614fc462fbd687d4265459f89e855a0f5657eeae46e4cc74e2313d1fdf6416f3713e5e3d8a78024cf2c17eb230b7d6ea0320c7c2d47ff89a12d7c17562f4b11358a2f0a6c295a418b86f7e1859dd15b8ea05b765613c30e1f28961e6db00738603d14bac0a2156fcd80cc3aff90faa85bc1a0f6af1af566e38c0e79dd558f33dc2191950f5419d3859c2ce14c86ed1ce86460a069ec9f67585224217f871811b278e3ac5513340ea8fb40abba9024144deb1c9ba07b4f37cf494d99d9b0f0a0c84b6b962bed6d9c50695ea01efbcb39ab7efa3feda08a671e3a2efdf64f63665f47e542b15fb0c5d4a46d18b13a303472c1e0b06cb7a0490313233bc3c1f05a7b857db9675529b77c4122afd072f6650fa4a5eb8acd50a08a9242ec83964381cd73218222e6cd8f348ae42bfd8bfb9d57646a1ef7e58e6aa09f017dd7a7003074da76ec93dbabe0937c4aac60ebe9acd6d4eee51570c20943a03dcf6e33625b70fdcbbd521777dca0f6c260fb7a054b55560c806f194588577aa02fc8ff257d0231b9decb65369f8554eb1a39d347303e69373e9baa0e96d3f892a03f4903282e362ac6b6612448bde022213af510772564079cd370c967be5b0c9ca01cded41896340d515edf0df64c2f989b2ff1b69d8e76f0ed2f780eea033c82a380",
    //             "0xf90211a0932b55f9eee8d7120f60ddb42b5c59271bc8794643856e61ac65044d338f8558a009e190ae5d1f4a9b056f9325aed4e91693e2aa28daf935dccf753376bbd98702a0809ba224dc09912ce1e5ce1b19031cbd9db8cdaaf8180bf6b773e0a1a0e7b0d6a04444f3f7146269776c0ff88b55592dfe09009d7dd837c4adf2b9ec800c280519a087d4322a7db7e87c97a11888d6385846851e7d34f7a9e8a948c635808bac66e1a0a2de659c14a2a7412259a23b844610b27091b88e68f39008c180c3a5e73bc1f3a03c8c395308ec72cd4488047cd49fd85afc1973879f32d18a6a78884575dce262a0e6f4c664af308354363b47d687c0a1f0b39fa44bdb4d1946e088d40e81d17604a0289b2296f9ef230b9b548141f07edd2f9afc0697e5cb13a6c052967482318db5a01b60eeee23088489b0f210b87981ad456066d81ddc7ca4831fd0d7f5684e83d5a096cb4737e61d2494bf615d204f1ab17b33a8e6cc2202d8c42d194e394c49cb8fa09a64452c574f30a68c0d0c3e8d610372375a3e6ccf6376f03dda297c362964a0a03d3f577b2b4ea7c7d1ea6b751f87b867616328dce3f3bbcc1829f5eced330d80a0f5cbe7bb776435eb78041748abea0371bcd2d743ca0f7b2945285b312f9c91a3a0b634122c093437dc70bb9f4354183e93eab37ce7e0e576048840c2b0cd55ca9ea0610727a8b4cd35e16d0296d7dccadc0a736ef62557d5dc36a828611049eaafc880",
    //             "0xf90211a039fb44712e438c7b8e0e3cefe57fab533424e44697abebd5184b9a796d3d9a19a042750d92bf27f6d2573210854749619ac21ce552f13b669cb6d8230ef85aa1b9a0dcf73fca6210a634d5c7ee66283155be12dd9eafe89d5e1bc141428958b380cba0aa149f6e9bf27d02783cd9c5539d5277f17e8f230772095feb103c4d073c7106a0db757353c616e0ed4146facc43089429d809b2b59a21bcee4481d0facf25daa7a0bba2aec3f6a69862e794c3ef27df5b135ff6f43e015cc1d4548fdc1c7721a5d6a0b74f10f8b31f2de595cc74e15083001ce04401285e244bf6fe990a59111b9ad7a0e69dac1014360b1a52a41c75a79db5d4f5e9581e30a477622b4f39e800272f39a0bf6371e0e8395e3da66073140d4d4284a3e10953bf50789b78959119b903e01ea0815bfac530740ff2f98f0abf4e28c020fdbe48f531b334324a78c003963e57aea0eefc05c5e110004f98ee1b7d36d67b3eb23bb232b96d17a7cae91720ed7efde4a0db8467645171691c4454555eaf81d1407c1c40620a6e0e6a8bd6af37a441970da0f861643b9e865c0a8cc68e0391219c1ea8a76c2ab78b83949b5f93e027402d06a06c09d87265fa9a2804904843dd98d95cfc48515ceb294571c0f6adcab24b03e6a03a01ee5b44c88d5e5728d3237c8d750d1dd02caff5b6903de4e31cb6eb909ce9a03bfc67659c7d7b2019b4d1fb9f434727cab270df7ab5990e53e948ddee57464180",
    //             "0xf90211a0c2717baa4e33cefd4fa4315bfa13d59c84fad4158113b11f3e320d9685a39d7da08bdbed90f58b09bb79e5cf8b322f595ddf8377d9ab7d3c451a7f56d663bf7377a05c67b94aacf25d1abf4d7a3f4e6c4540d81a1386ded86738c20f78c0182520c3a0a80be8e9110eeb82365bf4fd77f87d193120ca4384c99ddb1c6cb91d07b71cafa0e4ad621cf4d57986d3add88f38b0e7907130d2bbede05b067a5f1aff4ec58669a0652522234ca2bf0055749234d30ea49819e6b127029dd5c9c80c542473dbddeaa08a9be1f9a1fb6fb91114effbed5c75131dc57480ff0400550f86e7e56b851774a0a64ba35d28ffb39f920f90b5abb7ce809cbc53ab54fa3ad029a68146e4168d07a074bb4ea0440bfac93a8c40565f863acd45dcfe7d0c9534473ed295424e32ef12a0e7362a3396ecae24aaf06f50165eaf903739a2aa77effe4973373edc95d0af91a0813e366966f91e2a3f66949a2f07eaba03cfd7921c4d4d79a7874a303ec19880a082dfdd2dab55d9a4f4104d0cbde1c533504f5b23d567210b0f7ea8aa66e267f9a0b36c511f531ba943157b892f4f1b0289706d696335dbb290c48d2e361809c51da0c63d2e7e96575c78a8fdfc308254eb87944b28825c6111d18782cb9dc09b2bcea0b607a31934e8267b7dff817eeef4054d9b97260830b22bd54228c73e98fd8c18a080c6735390686d5e64384d146ccbce9af0020916a8a556d9890a8e8bfff44d3a80",
    //             "0xf90151808080a0eae0f9119829eb2c95ab9a7c512611dd15f865b0b6bcb79120e8baa66d41fb9fa046ec69737b80f7339615414902521dcfcdc65e2925bdbd1b7e39ceac75f17bb9a034f6eb707a84d7d369bdd53b45b92bdb807cb777dd4e38307b72597426808eeaa0ffaad9a6c4dc86c5538dcea592b211993debf94611498a4cee86b627ec7cceae80a03f84aeee6ce05e453011d0684aa4e95c1eae60ec0221f0bea08e99271977e638a044b7ec12971a2e91f5614fd5bcd1f6d5e92760f094966d731be8d9a3efe6b8b3a079690b80640075c24bed554559d5e037eac8c78c0ef6d164811127f3bc6e9761a023326ad841c693ecf304ad4f4daca45945d29997297141c3f615316169b6c78f80a0bc6ffeaf2d543e6448b061ed334362fdd22f97d1bfa630f7963d2396682cc0b8a0eca237f9f30343ac6aaf5d41501f6ae3e3272422791a2632d473bf1d872eee738080",
    //             "0xf8709d368c3ebf850d4896129da897b4c5d2da651431d5d0eaeda4de0ccff583b850f84e81c48908aea84b56865c19dfa056e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421a0c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
    //         ];
    //     let proof: Vec<_> = proof
    //         .into_iter()
    //         .map(|el| {
    //             let mut el = el;
    //             if el.starts_with("0x") {
    //                 el = &el[2..];
    //             }
    //             hex::decode(el).unwrap()
    //         })
    //         .collect();
    //     let mut trie = EthereumMPT::new_in(Global);
    //     let path_str = "317441868c3ebf850d4896129da897b4c5d2da651431d5d0eaeda4de0ccff583";
    //     assert_eq!(path_str.len(), 64);
    //     let full_path: Vec<u8> = path_str.bytes().collect();
    //     let value = trie
    //         .insert_proof(&full_path, proof.iter().map(|el| &el[..]))
    //         .unwrap();

    //     let element = trie.get_element(&full_path).unwrap();
    //     assert_eq!(value, element.unwrap().1);
    //     let leaf = element.unwrap().0;
    //     dbg!(hex::encode(trie.root()));
    //     let initial_root = trie.root();

    //     let mut close_path = full_path.clone();
    //     *close_path.last_mut().unwrap() = b"4"[0];
    //     let another_element = trie.get_element(&close_path).unwrap();
    //     assert!(another_element.is_none());

    //     // we didn't expose some leaves
    //     let path_str = "327441868c3ebf850d4896129da897b4c5d2da651431d5d0eaeda4de0ccff583";
    //     assert_eq!(path_str.len(), 64);
    //     let early_divergence_path: Vec<u8> = path_str.bytes().collect();
    //     let another_element = trie.get_element(&early_divergence_path);
    //     assert!(another_element.is_err());

    //     // but this one should work as we show empty place in branch node
    //     let path_str = "317441068c3ebf850d4896129da897b4c5d2da651431d5d0eaeda4de0ccff583";
    //     assert_eq!(path_str.len(), 64);
    //     let early_divergence_path: Vec<u8> = path_str.bytes().collect();
    //     let another_element = trie.get_element(&early_divergence_path).unwrap();
    //     assert!(another_element.is_none());

    //     // let mut interner = BoxInterner::with_capacity_in(1 << 20, Global);

    //     // println!("Trying to delete");
    //     // // try to delete our node
    //     // let _ = trie.delete_leaf(leaf, &mut interner).unwrap();
    //     // dbg!(trie.root);
    //     // dbg!(hex::encode(trie.root()));
    //     // assert!(trie.root() != initial_root);

    //     // let element = trie.get_element(&full_path).unwrap();
    //     // assert!(element.is_none());
    // }

    // #[test]
    // fn parse_leaf_2() {
    //     let proof = vec![
    //             "0xf851808080a0a26b2ac124718443aeed68fa0309225d0c8dd9dbee45685909e92fb594e1a4638080a02ccd118c9470c051689543b233ab109ad74d2fb4f57eb429c4d43294d6ae686780808080808080808080",
    //             "0xe5831616b6a0917fa5cab26d915e2a89a263a578fa5f9ecf02cc0b1d3eeb433e7f32499267e9",
    //             "0xf851808080808080808080808080a0cc97f12ea3217345e666974cd81b117ca02404f19c15d31158ac1d1e55398706a0822a55ca308aa885ad385d5e61aabaca54c2e4361eb03b6f851668c0f095ab77808080",
    //             "0xc482203104",
    //         ];
    //     let proof: Vec<_> = proof
    //         .into_iter()
    //         .map(|el| {
    //             let mut el = el;
    //             if el.starts_with("0x") {
    //                 el = &el[2..];
    //             }
    //             hex::decode(el).unwrap()
    //         })
    //         .collect();
    //     let mut trie = EthereumMPT::new_in(Global);
    //     let path_str = "616b6d31";
    //     let full_path: Vec<u8> = path_str.bytes().collect();
    //     let value = trie
    //         .insert_proof(&full_path, proof.iter().map(|el| &el[..]))
    //         .unwrap();

    //     let element = trie.get_element(&full_path).unwrap();
    //     assert_eq!(value, element.unwrap());
    // }
}
