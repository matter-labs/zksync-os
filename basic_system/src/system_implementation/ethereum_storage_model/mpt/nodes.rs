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
        } else if self.is_unreferenced_path() {
            f.debug_struct("Node: Unreferenced")
                .field("index", &self.index())
                .finish()
        } else if self.is_unlinked() {
            f.debug_tuple("Node: Unlinked").finish()
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
    const UNREFERENCED_PATH: usize = 0b100;
    const UNLINKED_MARKER: usize = 0b101;

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

    pub(crate) const fn unknown_branch() -> Self {
        Self {
            inner: Self::UNREFERENCED_PATH,
        }
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

    pub(crate) const fn unreferenced_path(index: usize) -> Self {
        Self {
            inner: (index << Self::RAW_INDEX_SHIFT) | Self::UNREFERENCED_PATH,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.inner & Self::TYPE_MASK == Self::EMPTY_TYPE_MARKER
    }

    pub(crate) fn is_leaf(&self) -> bool {
        self.inner & Self::TYPE_MASK == Self::LEAF_TYPE_MARKER
    }

    pub(crate) fn is_extension(&self) -> bool {
        self.inner & Self::TYPE_MASK == Self::EXTENSION_TYPE_MARKER
    }

    pub(crate) fn is_branch(&self) -> bool {
        self.inner & Self::TYPE_MASK == Self::BRANCH_TYPE_MARKER
    }

    pub(crate) fn is_unreferenced_path(&self) -> bool {
        self.inner & Self::TYPE_MASK == Self::UNREFERENCED_PATH
    }

    pub(crate) fn is_unlinked(&self) -> bool {
        self.inner & Self::TYPE_MASK == Self::UNLINKED_MARKER
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct PathSegment<'a> {
    path: Path<'a>,
    segment_len: usize,
}

impl<'a> PathSegment<'a> {
    pub(crate) fn is_empty(&self) -> bool {
        self.segment().is_empty()
    }

    pub(crate) const fn prefix_len(&self) -> usize {
        self.path.prefix_len()
    }

    pub(crate) fn segment(&self) -> &'a [u8] {
        &self.path.remaining_path()[..self.segment_len]
    }

    pub(crate) fn prefix(&self) -> &'a [u8] {
        self.path.prefix()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Path<'a> {
    path: &'a [u8],
    prefix_len: usize,
}

impl<'a> Path<'a> {
    pub(crate) fn new(path: &'a [u8]) -> Self {
        Self {
            path,
            prefix_len: 0,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.remaining_path().is_empty()
    }

    pub(crate) const fn prefix_len(&self) -> usize {
        self.prefix_len
    }

    pub(crate) fn into_prefix_only(&self) -> Self {
        Self {
            path: self.prefix(),
            prefix_len: self.prefix_len,
        }
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

    #[inline]
    pub(crate) fn follow(&mut self, path_segment: &[u8]) -> Result<bool, ()> {
        if self.remaining_path().len() < path_segment.len() {
            // try to follow too far
            return Err(());
        }
        let follows = self.remaining_path().starts_with(path_segment);
        self.prefix_len += path_segment.len();

        Ok(follows)
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

// TODO: manually check that derives first compare keys
// TODO: consider if pointer equality is enough. Most likely yes

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct LeafNode<'a> {
    // pub(crate) key: &'a [u8], // NOTE: only used when tree is being constructed from proofs. Invalid after updates
    // pub(crate) prefix: &'a [u8],
    pub(crate) path_segment: &'a [u8],
    pub(crate) parent_node: NodeType,
    pub(crate) raw_nibbles_encoding: &'a [u8], // RLP, not even internals. Handy for updates
    // pub(crate) raw_encoding: &'a [u8], // of full node (including prefix)
    // pub(crate) value: &'a [u8], // fully parsed, and if any update will happen we can benefit from it
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ExtensionNode<'a> {
    // pub(crate) key: &'a [u8], // NOTE: only used when tree is being constructed from proofs. Invalid after updates
    // pub(crate) prefix: &'a [u8],
    pub(crate) path_segment: &'a [u8],
    pub(crate) parent_node: NodeType,
    pub(crate) child_node: NodeType,
    pub(crate) raw_nibbles_encoding: &'a [u8], // RLP, not even internals. Handy for updates
    // pub(crate) raw_encoding: &'a [u8], // of full node (including prefix)
    // pub(crate) next_node_key: &'a [u8], // fully parsed, and if any update will happen we can benefit from it
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct UnreferencedPath<'a> {
    pub(crate) key: &'a [u8],
    pub(crate) path: PathSegment<'a>,
    pub(crate) parent_node: NodeType,
    pub(crate) raw_encoding: &'a [u8],
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct BranchNode<'a> {
    // pub(crate) key: &'a [u8],  // NOTE: only used when tree is being constructed from proofs. Invalid after updates
    // pub(crate) prefix: &'a [u8],
    pub(crate) parent_node: NodeType,
    pub(crate) child_nodes: [NodeType; 16],
    pub(crate) branches_encodings_concatenation: &'a [u8],
    pub(crate) child_encoding_lengths: [u8; 16], // can not be more than 33 anyway

    // pub(crate) child_nodes_raw_encodings: [&'a [u8]; 16], // allows to avoid storing raw encodings in other node types
    // pub(crate) raw_encoding: &'a [u8],
    // in practice branch nodes can not have value - consensus forbids branch nodes with 0 or 1 children,
    // and all storage slot keys are fixed 32 bytes, so branch node can not be "passthrough"
}

impl<'a> core::fmt::Debug for BranchNode<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("BranchNode")
            .field("parent_node", &self.parent_node)
            .field("child_nodes", &self.child_nodes)
            .finish()
    }
}

impl<'a> BranchNode<'a> {
    pub(crate) fn num_occupied(&self) -> usize {
        let mut occupied = 0;
        for el in self.child_nodes.iter() {
            if el.is_empty() == false {
                occupied += 1;
            }
        }

        occupied
    }
}
