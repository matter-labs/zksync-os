// As we will not use advanced interner to allow variable-length arrays,
// instead we will just store worst-case common prefix in leaf and extension nodes

// Small note on the logic: we implement nodes just as indexes,
// but we should store sufficicent information for deletes or inserts (updates can not change node type)
// Let's go through the different types of inserts and deletes (we will delete before inserts for simplicity in practice)
// Deletes:
// - Delete leaf - cascade it all the way up until we hit branch, see below
// - Delete from branch, and branch doesn't get converted - fine
// - Delete from branch, so it becomes extension - huge pain, as we have to cascase it all the way down to next branch or leaf
// Inserts are more involved:
// - Inserts directly into branch - simplest case
// - Inserts somewhere near the leaf - convert to branch, but types of nodes do not change
// - Inserts somewhere near the extension - convert to branch too, potentially eliminating extension itself

use crate::system_implementation::ethereum_storage_model::{mpt::LeafValue, ByteBuffer};

// Stable index. We assume that number of nodes is small enough
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct NodeType {
    inner: usize,
}

impl core::fmt::Debug for NodeType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.is_empty() {
            f.debug_tuple("Node: Empty").finish()
        } else if self.is_leaf() {
            f.debug_struct("Node: Leaf")
                .field("index", &self.index())
                .finish()
        } else if self.is_extension() {
            f.debug_struct("Node: Extension")
                .field("index", &self.index())
                .finish()
        } else if self.is_branch() {
            f.debug_struct("Node: Branch")
                .field("index", &self.index())
                .finish()
        } else if self.is_unreferenced_key() {
            f.debug_struct("Node: Unreferenced key")
                .field("index", &self.index())
                .finish()
        } else if self.is_unlinked() {
            f.debug_tuple("Node: Unlinked").finish()
        } else if self.is_opaque_nontrivial_root() {
            f.debug_tuple("Node: Opaque non-trivial root").finish()
        } else {
            unreachable!()
        }
    }
}

impl NodeType {
    const RAW_INDEX_SHIFT: u32 = 3;
    const TYPE_MASK: usize = 0b111;
    const EMPTY_TYPE_MARKER: usize = 0b000;
    const LEAF_TYPE_MARKER: usize = 0b001;
    const EXTENSION_TYPE_MARKER: usize = 0b010;
    const BRANCH_TYPE_MARKER: usize = 0b011;
    const UNREFERENCED_KEY: usize = 0b100;
    const UNLINKED_MARKER: usize = 0b101;
    const OPAQUE_NONTRIVIAL_ROOT: usize = 0b111;

    pub(crate) const fn index(&self) -> usize {
        self.inner >> Self::RAW_INDEX_SHIFT
    }

    pub(crate) const fn empty() -> Self {
        Self {
            inner: Self::EMPTY_TYPE_MARKER,
        }
    }

    pub(crate) const fn unlinked() -> Self {
        Self {
            inner: Self::UNLINKED_MARKER,
        }
    }

    pub(crate) const fn opaque_nontrivial_root() -> Self {
        Self {
            inner: Self::OPAQUE_NONTRIVIAL_ROOT,
        }
    }

    pub(crate) const fn is_opaque_nontrivial_root(&self) -> bool {
        self.inner & Self::TYPE_MASK == Self::OPAQUE_NONTRIVIAL_ROOT
    }

    pub(crate) const fn leaf(index: usize) -> Self {
        Self {
            inner: (index << Self::RAW_INDEX_SHIFT) | Self::LEAF_TYPE_MARKER,
        }
    }

    pub(crate) const fn extension(index: usize) -> Self {
        Self {
            inner: (index << Self::RAW_INDEX_SHIFT) | Self::EXTENSION_TYPE_MARKER,
        }
    }

    pub(crate) const fn branch(index: usize) -> Self {
        Self {
            inner: (index << Self::RAW_INDEX_SHIFT) | Self::BRANCH_TYPE_MARKER,
        }
    }

    pub(crate) const fn unreferenced_value(index: usize) -> Self {
        Self {
            inner: (index << Self::RAW_INDEX_SHIFT) | Self::UNREFERENCED_KEY,
        }
    }

    pub(crate) const fn is_empty(&self) -> bool {
        self.inner & Self::TYPE_MASK == Self::EMPTY_TYPE_MARKER
    }

    pub(crate) const fn is_leaf(&self) -> bool {
        self.inner & Self::TYPE_MASK == Self::LEAF_TYPE_MARKER
    }

    pub(crate) const fn is_extension(&self) -> bool {
        self.inner & Self::TYPE_MASK == Self::EXTENSION_TYPE_MARKER
    }

    pub(crate) const fn is_branch(&self) -> bool {
        self.inner & Self::TYPE_MASK == Self::BRANCH_TYPE_MARKER
    }

    pub(crate) const fn is_unreferenced_key(&self) -> bool {
        self.inner & Self::TYPE_MASK == Self::UNREFERENCED_KEY
    }

    pub(crate) const fn is_unlinked(&self) -> bool {
        self.inner & Self::TYPE_MASK == Self::UNLINKED_MARKER
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Path<'a> {
    pub(crate) path: &'a [u8],
    pub(crate) prefix_len: usize,
}

impl<'a> Path<'a> {
    pub fn new(path: &'a [u8]) -> Self {
        Self {
            path,
            prefix_len: 0,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.remaining_path().is_empty()
    }

    pub(crate) fn prefix(&self) -> &'a [u8] {
        &self.path[..self.prefix_len]
    }

    pub(crate) fn full_path(&self) -> &'a [u8] {
        self.path
    }

    pub(crate) fn remaining_path(&self) -> &'a [u8] {
        &self.path[self.prefix_len..]
    }

    pub(crate) fn seek_to_end(&mut self) {
        self.prefix_len = self.path.len();
    }

    pub(crate) fn ascend(&mut self, path_segment: &[u8]) {
        let Some(..) = self.prefix().strip_suffix(path_segment) else {
            panic!()
        };
        self.prefix_len -= path_segment.len();
    }

    pub(crate) fn ascend_branch(&mut self) -> Result<usize, ()> {
        if let Some(last) = self.prefix().last().copied() {
            self.prefix_len -= 1;

            Ok(last as usize)
        } else {
            Err(())
        }
    }

    #[inline]
    pub(crate) fn follow(&mut self, path_segment: &[u8]) -> Result<bool, ()> {
        if self.remaining_path().len() < path_segment.len() {
            // try to follow too far
            return Err(());
        }
        let follows = self.remaining_path().starts_with(path_segment);
        if follows {
            self.prefix_len += path_segment.len();
        }

        Ok(follows)
    }

    #[track_caller]
    pub(crate) fn follow_common_prefix(&mut self, path_segment: &[u8]) -> Result<usize, ()> {
        let remaining = self.remaining_path();
        if remaining.len() < path_segment.len() {
            // try to follow too far
            return Err(());
        }
        let max_len = path_segment.len();
        for i in 0..max_len {
            if remaining[i] != path_segment[i] {
                return Ok(i);
            }
            self.prefix_len += 1;
        }

        Ok(max_len)
    }

    pub(crate) fn take_branch(&mut self) -> Result<usize, ()> {
        if self.remaining_path().is_empty() {
            return Err(());
        }
        let t = self.remaining_path()[0];
        self.prefix_len += 1;

        Ok(t as usize)
    }
}

// One of the hard topics is how to easily identify nodes. We need to define some types that
// would be unique enough, to guarantee that even if we somehow encounter

// #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(Debug)]
pub(crate) struct LeafNode<'a> {
    pub(crate) cached_key: &'a [u8],
    pub(crate) path_segment: &'a [u8],
    pub(crate) parent_node: NodeType,
    pub(crate) value: LeafValue<'a>,
}

impl<'a> LeafNode<'a> {
    pub(crate) fn invalidate_cache(&mut self) {
        self.cached_key = &[];
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ExtensionNode<'a> {
    pub(crate) cached_key: &'a [u8],
    pub(crate) path_segment: &'a [u8],
    pub(crate) parent_node: NodeType,
    pub(crate) child_node: NodeType,
}

impl<'a> ExtensionNode<'a> {
    pub(crate) fn invalidate_cache(&mut self) {
        self.cached_key = &[];
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct UnreferencedKey<'a> {
    pub(crate) cached_key: &'a [u8],
    pub(crate) parent_node: NodeType,
    pub(crate) branch_index: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct BranchNode<'a> {
    pub(crate) cached_key: &'a [u8],
    pub(crate) parent_node: NodeType,
    pub(crate) child_nodes: [NodeType; 16],
    pub(crate) num_occupied: usize,
    pub(crate) _marker: core::marker::PhantomData<&'a ()>,
}

impl<'a> core::fmt::Debug for BranchNode<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("BranchNode")
            .field("parent_node", &self.parent_node)
            .field("child_nodes", &self.child_nodes)
            .field("num_occupied", &self.num_occupied)
            .finish()
    }
}

impl<'a> BranchNode<'a> {
    pub(crate) fn num_occupied(&self) -> usize {
        self.num_occupied
    }

    pub(crate) fn attach(&mut self, child: NodeType, branch_index: usize) -> Result<(), ()> {
        if self.child_nodes[branch_index].is_empty() {
            self.child_nodes[branch_index] = child;
            self.num_occupied += 1;
            Ok(())
        } else {
            Err(())
        }
    }

    pub(crate) fn delete(&mut self, branch_index: usize) -> Result<(), ()> {
        if self.child_nodes[branch_index].is_empty() == false {
            self.child_nodes[branch_index] = NodeType::empty();
            self.num_occupied -= 1;
            Ok(())
        } else {
            Err(())
        }
    }

    pub(crate) fn invalidate_cache(&mut self) {
        self.cached_key = &[];
    }

    #[allow(dead_code)]
    pub(crate) fn replace_child(&mut self, existing: NodeType, new: NodeType) -> Result<(), ()> {
        if existing.is_empty() {
            // should use linkage instead
            return Err(());
        }

        for child in self.child_nodes.iter_mut() {
            if *child == existing {
                *child = new;
                return Ok(());
            }
        }

        Err(())
    }
}

pub(crate) fn write_nibbles(buffer: &mut impl ByteBuffer, is_leaf: bool, path: &[u8]) {
    if path.is_empty() {
        assert!(is_leaf);
        buffer.write_byte(0x20);
        return;
    }

    let num_nibbles = path.len();
    let (mut byte, mut write_high) = if num_nibbles % 2 == 1 {
        if is_leaf {
            (0x30, false)
        } else {
            (0x10, false)
        }
    } else if is_leaf {
        (0x20, true)
    } else {
        (0x00, true)
    };

    for el in path.iter() {
        if write_high {
            buffer.write_byte(byte);
            byte = *el << 4;
            write_high = false;
        } else {
            byte |= *el;
            write_high = true;
        }
    }
    if write_high {
        buffer.write_byte(byte);
    }
}
