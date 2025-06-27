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

pub(crate) const WORST_CASE_PREFIX_LEN: usize = 64;

#[inline]
fn consume<'a>(src: &mut &'a [u8], bytes: usize) -> Result<&'a [u8], ()> {
    let (data, rest) = src.split_at_checked(bytes).ok_or(())?;
    *src = rest;

    Ok(data)
}

// const MAX_BITS: usize = 55 * 8;

// /// Allows a [`Uint`] to be serialized as RLP.
// ///
// /// See <https://ethereum.org/en/developers/docs/data-structures-and-encoding/rlp/>
// impl<const BITS: usize, const LIMBS: usize> Encodable for Uint<BITS, LIMBS> {
//     #[inline]
//     fn length(&self) -> usize {
//         let bits = self.bit_len();
//         if bits <= 7 {
//             1
//         } else {
//             let bytes = (bits + 7) / 8;
//             bytes + length_of_length(bytes)
//         }
//     }

//     #[inline]
//     fn encode(&self, out: &mut dyn BufMut) {
//         // fast paths, avoiding allocation due to `to_be_bytes_vec`
//         match LIMBS {
//             0 => return out.put_u8(EMPTY_STRING_CODE),
//             1 => return self.limbs[0].encode(out),
//             #[allow(clippy::cast_lossless)]
//             2 => return (self.limbs[0] as u128 | ((self.limbs[1] as u128) << 64)).encode(out),
//             _ => {}
//         }

//         match self.bit_len() {
//             0 => out.put_u8(EMPTY_STRING_CODE),
//             1..=7 => {
//                 #[allow(clippy::cast_possible_truncation)] // self < 128
//                 out.put_u8(self.limbs[0] as u8);
//             }
//             bits => {
//                 // avoid heap allocation in `to_be_bytes_vec`
//                 // SAFETY: we don't re-use `copy`
//                 #[cfg(target_endian = "little")]
//                 let mut copy = *self;
//                 #[cfg(target_endian = "little")]
//                 let bytes = unsafe { copy.as_le_slice_mut() };
//                 #[cfg(target_endian = "little")]
//                 bytes.reverse();

//                 #[cfg(target_endian = "big")]
//                 let bytes = self.to_be_bytes_vec();

//                 let leading_zero_bytes = Self::BYTES - (bits + 7) / 8;
//                 let trimmed = &bytes[leading_zero_bytes..];
//                 if bits > MAX_BITS {
//                     trimmed.encode(out);
//                 } else {
//                     #[allow(clippy::cast_possible_truncation)] // bytes.len() < 56 < 256
//                     out.put_u8(EMPTY_STRING_CODE + trimmed.len() as u8);
//                     out.put_slice(trimmed);
//                 }
//             }
//         }
//     }
// }

// Stable index. We assume that number of nodes is small enough
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct NodeType {
    inner: usize,
}

// Stable index
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct EncodingCacheIndex {
    inner: usize,
}

impl EncodingCacheIndex {
    pub(crate) const fn empty() -> Self {
        Self { inner: 0 }
    }

    pub(crate) const fn new(index: usize) -> Self {
        Self { inner: index }
    }

    pub(crate) const fn inner(&self) -> usize {
        self.inner
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

    pub(crate) const fn index(&self) -> usize {
        self.inner >> Self::RAW_INDEX_SHIFT
    }

    pub(crate) const fn empty() -> Self {
        Self {
            inner: Self::EMPTY_TYPE_MARKER,
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Nibbles<'a> {
    path: &'a [u8],
    prefix_len: usize,
}

impl<'a> Nibbles<'a> {
    pub(crate) fn new(path: &'a [u8]) -> Self {
        Self {
            path,
            prefix_len: 0,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        assert_eq!(self.prefix_len, 64);
        self.path.is_empty()
    }

    pub(crate) const fn prefix_len(&self) -> usize {
        self.prefix_len
    }

    pub(crate) const fn path(&self) -> &'a [u8] {
        self.path
    }

    pub(crate) fn path_char_to_digit(c: u8) -> u8 {
        match c {
            b'A'..=b'F' => c - b'A' + 10,
            b'a'..=b'f' => c - b'a' + 10,
            b'0'..=b'9' => c - b'0',
            _ => {
                unreachable!()
            }
        }
    }

    #[inline]
    pub(crate) fn follow(
        &mut self,
        raw_nibbles: &[u8],
        skip_single_char: bool,
    ) -> Result<Self, ()> {
        // raw nibbles are bytes, that have to be interpreted as chars
        let mut num_nibbles = raw_nibbles.len() * 2 - 1;
        if skip_single_char == false {
            num_nibbles -= 1;
        }
        if self.path.len() < num_nibbles {
            return Err(());
        }
        let taken_nibbles = &self.path[..num_nibbles];
        // actually check char by char
        let mut it = raw_nibbles.iter();

        unsafe {
            let mut nibbles_byte = it.next().unwrap_unchecked();
            let mut process_next = false;
            if skip_single_char == false {
                process_next = true;
            }
            for el in taken_nibbles.iter() {
                let value = if process_next {
                    nibbles_byte = it.next().unwrap_unchecked();
                    process_next = false;
                    nibbles_byte >> 4
                } else {
                    process_next = true;
                    nibbles_byte & 0x0f
                };
                if Self::path_char_to_digit(*el) != value {
                    return Err(());
                }
            }
        }
        self.path = &self.path[num_nibbles..];
        let t = self.prefix_len;
        self.prefix_len += num_nibbles;

        Ok(Self {
            path: taken_nibbles,
            prefix_len: t,
        })
    }

    pub(crate) fn take_branch(&mut self) -> Result<usize, ()> {
        if self.path.is_empty() {
            return Err(());
        }
        let t = Self::path_char_to_digit(self.path[0]);
        self.path = &self.path[1..];
        self.prefix_len += 1;

        Ok(t as usize)
    }
}

// One of the hard topics is how to easily identify nodes. We need to define some types that
// would be unique enough, to guarantee that even if we somehow encounter

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct LeafNode<'a> {
    pub(crate) path: Nibbles<'a>,
    pub(crate) parent_node: NodeType,
    pub(crate) raw_encoding: &'a [u8],
    pub(crate) value: &'a [u8],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ExtensionNode<'a> {
    pub(crate) path: Nibbles<'a>,
    pub(crate) parent_node: NodeType,
    pub(crate) child_node: NodeType,
    pub(crate) raw_encoding: &'a [u8],
    pub(crate) value: &'a [u8],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct UnreferencedPath<'a> {
    pub(crate) path: Nibbles<'a>,
    pub(crate) parent_node: NodeType,
    pub(crate) raw_encoding: &'a [u8],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct BranchNode<'a> {
    pub(crate) parent_node: NodeType,
    pub(crate) child_nodes: [NodeType; 16],
    pub(crate) raw_encoding: &'a [u8],
    // in practice branch nodes can not have value - consensus forbids branch nodes with 0 or 1 children,
    // and all storage slot keys are fixed 32 bytes, so branch node can not be "passthrough"
}
