//! Stack-based Ethereum MPT for sorted single-pass verification and update.
//!
//! The trie is never materialized as a node graph. Instead the caller visits keys in strictly
//! increasing order, and the trie keeps only the path from the root to the current key as a
//! stack of unfolded nodes ("lines"). Moving to the next key folds the lines that are to the
//! right of the common prefix with the previous key, so every witness node is unfolded (fetched
//! from the oracle, keccak-verified, parsed) at most once, unmodified nodes are dropped without
//! being re-encoded, and modified nodes are encoded and hashed exactly once when folded.
//!
//! Soundness: every node reached by hash is verified against its hash on unfold. A key that is
//! found gets its pre-state value reported to the caller (which compares it against the claimed
//! initial value), a key that is not found is proven absent by the structure of the verified
//! nodes on its path. The new root is the fold of the modified path.
//!
//! Lines are fixed slots that are written in place: a node is parsed straight into its slot,
//! and folding a line never moves it, as copies of the ~300 byte lines were the dominant cost.

use super::interner::{ByteBuffer, Interner, InterningWordBuffer, WORD};
use super::*;
use alloc::vec::Vec;
use core::alloc::Allocator;
use crypto::MiniDigest;

/// Trie key: 64 nibbles of a 32-byte hash, packed as 8 big-endian words so the derived
/// lexicographic order matches the byte order and common prefixes are found word-wise.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Default, Hash)]
pub struct TrieKey([u32; 8]);

impl TrieKey {
    pub const NUM_NIBBLES: usize = 64;

    pub fn from_hash(hash: &[u8; 32]) -> Self {
        let mut words = [0u32; 8];
        for (word, chunk) in words.iter_mut().zip(hash.as_chunks::<4>().0.iter()) {
            *word = u32::from_be_bytes(*chunk);
        }
        Self(words)
    }

    #[inline(always)]
    pub fn nibble(&self, index: usize) -> u8 {
        debug_assert!(index < Self::NUM_NIBBLES);
        ((self.0[index >> 3] >> (28 - 4 * (index & 7))) & 0xf) as u8
    }

    /// Number of leading nibbles shared with `other`
    pub fn common_prefix_len(&self, other: &Self) -> usize {
        for (i, (a, b)) in self.0.iter().zip(other.0.iter()).enumerate() {
            let diff = a ^ b;
            if diff != 0 {
                return i * 8 + (diff.leading_zeros() as usize) / 4;
            }
        }
        Self::NUM_NIBBLES
    }

    /// Common prefix of `segment` with the key starting at nibble `from`
    fn common_prefix_with_segment(&self, from: usize, segment: &[u8]) -> usize {
        debug_assert!(from + segment.len() <= Self::NUM_NIBBLES);
        for (i, nibble) in segment.iter().enumerate() {
            if self.nibble(from + i) != *nibble {
                return i;
            }
        }
        segment.len()
    }
}

/// A path segment of a leaf or an extension node, stored inline
#[derive(Clone, Copy)]
struct Segment {
    len: u8,
    nibbles: [u8; TrieKey::NUM_NIBBLES],
}

impl Segment {
    const EMPTY: Self = Self {
        len: 0,
        nibbles: [0u8; TrieKey::NUM_NIBBLES],
    };

    #[inline(always)]
    fn as_slice(&self) -> &[u8] {
        &self.nibbles[..self.len as usize]
    }

    /// Set to the tail of the key starting at nibble `from`
    fn set_from_key(&mut self, key: &TrieKey, from: usize) {
        let len = TrieKey::NUM_NIBBLES - from;
        for i in 0..len {
            self.nibbles[i] = key.nibble(from + i);
        }
        self.len = len as u8;
    }

    fn with_prefix(prefix: &[u8], suffix: &[u8]) -> Result<Self, ()> {
        if prefix.len() + suffix.len() > TrieKey::NUM_NIBBLES {
            return Err(());
        }
        let mut new = Self::EMPTY;
        new.nibbles[..prefix.len()].copy_from_slice(prefix);
        new.nibbles[prefix.len()..][..suffix.len()].copy_from_slice(suffix);
        new.len = (prefix.len() + suffix.len()) as u8;

        Ok(new)
    }

    /// Decode hex-prefix ("compact") encoding in place. Returns whether it is a leaf
    fn set_from_compact(&mut self, encoding: &[u8]) -> Result<bool, ()> {
        let Some((first, rest)) = encoding.split_first() else {
            return Err(());
        };
        let flags = first >> 4;
        if flags > 3 {
            return Err(());
        }
        let is_leaf = flags & 2 != 0;
        let is_odd = flags & 1 != 0;
        if is_odd == false && first & 0x0f != 0 {
            return Err(());
        }
        let len = rest.len() * 2 + (is_odd as usize);
        if len > TrieKey::NUM_NIBBLES {
            return Err(());
        }
        let mut index = 0;
        if is_odd {
            self.nibbles[0] = first & 0x0f;
            index = 1;
        }
        for byte in rest.iter() {
            self.nibbles[index] = byte >> 4;
            self.nibbles[index + 1] = byte & 0x0f;
            index += 2;
        }
        self.len = len as u8;

        Ok(is_leaf)
    }
}

/// Reference to a child node as it appears in the parent: RLP item that is either empty,
/// a 33-byte encoding of the hash, or the node itself if it is shorter than 32 bytes.
/// `encoding` is the full node encoding when it is known without going to the oracle
/// (the node was encoded by us), so a collapsing parent can re-parse it.
#[derive(Clone, Copy)]
struct ChildRef<'a> {
    key: &'a [u8],
    encoding: &'a [u8],
}

/// Placeholder for a child that is on the stack and will be written back when folded
const PENDING_CHILD_KEY: &[u8] = &[0xff];

impl<'a> ChildRef<'a> {
    const EMPTY: Self = Self {
        key: &[],
        encoding: &[],
    };

    const PENDING: Self = Self {
        key: PENDING_CHILD_KEY,
        encoding: &[],
    };

    #[inline(always)]
    fn is_empty(&self) -> bool {
        // NOTE: the parser returns an empty slice for the empty item (0x80), and we never
        // construct a reference from the encoding of the empty item
        self.key.is_empty()
    }

    #[inline(always)]
    fn is_pending(&self) -> bool {
        core::ptr::eq(self.key.as_ptr(), PENDING_CHILD_KEY.as_ptr())
    }

    #[inline(always)]
    fn from_piece(piece: &'a [u8]) -> Result<Self, ()> {
        if piece.len() > 33 {
            return Err(());
        }
        Ok(Self {
            key: piece,
            encoding: &[],
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// Vacant slot
    Empty,
    Leaf,
    Extension,
    Branch,
}

/// One unfolded node on the path. Flat layout: fields are used according to `kind`
struct Line<'a> {
    kind: Kind,
    /// Number of nibbles of the key consumed before this node
    depth: u8,
    /// Index in the parent branch (unused for a child of an extension or the root)
    parent_slot: u8,
    modified: bool,
    /// Branch: number of non-empty children
    num_children: u8,
    /// Leaf and extension
    segment: Segment,
    /// Leaf: full RLP item of the value
    value: &'a [u8],
    /// Extension
    child: ChildRef<'a>,
    /// Branch
    children: [ChildRef<'a>; 16],
}

impl<'a> Line<'a> {
    const VACANT: Self = Self {
        kind: Kind::Empty,
        depth: 0,
        parent_slot: 0,
        modified: false,
        num_children: 0,
        segment: Segment::EMPTY,
        value: &[],
        child: ChildRef::EMPTY,
        children: [ChildRef::EMPTY; 16],
    };

    fn validate(&self) -> Result<(), ()> {
        let depth = self.depth as usize;
        let valid = match self.kind {
            Kind::Empty => false,
            Kind::Leaf => depth + self.segment.len as usize == TrieKey::NUM_NIBBLES,
            Kind::Extension => {
                self.segment.len > 0 && (depth + self.segment.len as usize) < TrieKey::NUM_NIBBLES
            }
            Kind::Branch => depth < TrieKey::NUM_NIBBLES,
        };
        if valid {
            Ok(())
        } else {
            Err(())
        }
    }

    /// Parse a node encoding straight into the line
    fn parse_into(&mut self, raw_encoding: &'a [u8]) -> Result<(), ()> {
        if raw_encoding.len() < 3 {
            return Err(());
        }
        let mut data = raw_encoding;
        let b0 = consume(&mut data, 1)?[0];
        if b0 < 0xc0 {
            return Err(());
        }
        let payload_len = if b0 < 0xf8 {
            (b0 - 0xc0) as usize
        } else {
            let length_encoding_length = (b0 - 0xf7) as usize;
            let length_encoding_bytes = consume(&mut data, length_encoding_length)?;
            super::parse_node::decode_short_length(length_encoding_bytes)?
        };
        if data.len() != payload_len {
            return Err(());
        }
        let piece_0 = parse_node_piece(&mut data)?;
        let piece_1 = parse_node_piece(&mut data)?;
        if data.is_empty() {
            // leaf or extension
            let nibbles_encoding = RLPSlice::from_slice(piece_0)?;
            let is_leaf = self.segment.set_from_compact(nibbles_encoding.data())?;
            if is_leaf {
                // value must be a string item
                let _ = RLPSlice::from_slice(piece_1)?;
                self.value = piece_1;
                self.kind = Kind::Leaf;
            } else {
                if self.segment.len == 0 {
                    return Err(());
                }
                let child = ChildRef::from_piece(piece_1)?;
                if child.is_empty() {
                    return Err(());
                }
                self.child = child;
                self.kind = Kind::Extension;
            }
        } else {
            self.children[0] = ChildRef::from_piece(piece_0)?;
            self.children[1] = ChildRef::from_piece(piece_1)?;
            let mut count = (self.children[0].is_empty() == false) as u8
                + (self.children[1].is_empty() == false) as u8;
            for child in self.children[2..].iter_mut() {
                *child = ChildRef::from_piece(parse_node_piece(&mut data)?)?;
                count += (child.is_empty() == false) as u8;
            }
            // no values in branches for fixed-length keys
            let value = parse_node_piece(&mut data)?;
            if value.is_empty() == false || data.is_empty() == false {
                return Err(());
            }
            if count < 2 {
                return Err(());
            }
            self.num_children = count;
            self.kind = Kind::Branch;
        }

        Ok(())
    }
}

/// What a folded line contributes to its parent
enum Folded<'a> {
    /// Node vanished
    Empty,
    /// Full encoding of the node
    Encoded(&'a [u8]),
    /// Single-child branch collapsed into a leaf or extension that may be merged
    /// into a parent extension
    Merged {
        is_leaf: bool,
        segment: Segment,
        /// value item for a leaf, child key for an extension
        payload: &'a [u8],
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cursor {
    Invalid,
    /// Top line is the leaf with the current key
    AtLeaf,
    /// Trie is empty
    EmptyTrie,
    /// Top line is a branch, and the slot for the current key is empty
    EmptySlot {
        nibble: u8,
    },
    /// Top line is an extension whose child vanished
    DeadExtension,
    /// Top line is an extension and the key diverges from its segment
    DivergedExtension {
        common: u8,
    },
    /// Top line is a leaf and the key diverges from its segment
    DivergedLeaf {
        common: u8,
    },
}

// The deepest chain is 64 branches and a leaf with an empty segment
const MAX_LINES: usize = 66;

/// Full encoding of a leaf (payload is the value item) or extension (payload is the child key)
fn encode_short_node<'a>(
    interner: &mut (impl Interner<'a> + 'a),
    is_leaf: bool,
    segment: &[u8],
    payload: &[u8],
) -> Result<&'a [u8], ()> {
    let compact_len = segment.len() / 2 + 1;
    // single byte of compact encoding is always below 0x80
    let nibbles_item_len = if compact_len == 1 { 1 } else { 1 + compact_len };
    let payload_len = nibbles_item_len + payload.len();
    let total_len = list_encoding_prefix_len(payload_len) + payload_len;
    let mut buffer = interner.get_buffer(total_len)?;
    encode_list_len_into_buffer(&mut buffer, payload_len);
    if compact_len > 1 {
        buffer.write_byte(0x80 + compact_len as u8);
    }
    write_nibbles(&mut buffer, is_leaf, segment);
    buffer.write_slice(payload);

    Ok(buffer.flush())
}

/// Writes the RLP of a branch with the given children into `buffer`
fn write_branch_encoding<'a>(
    buffer: &mut impl ByteBuffer,
    children: &[ChildRef<'a>; 16],
    payload_len: usize,
) {
    encode_list_len_into_buffer(buffer, payload_len);

    // Children parsed from a node point into its encoding, in order, so unchanged neighbours
    // are contiguous runs of source bytes (an empty child is its 0x80 byte there, see
    // `parse_node_piece`): a run is written with one copy instead of one per child. A run
    // only continues through a child whose bytes start exactly where the run ends; a
    // reference built by `make_ref` or a synthetic empty child never does (a fresh interner
    // buffer cannot start inside the parent's encoding, and the empty slice is dangling).
    let mut run_start: *const u8 = core::ptr::null();
    let mut run_end: *const u8 = core::ptr::null();
    for child in children.iter() {
        let key = child.key;
        let continues = !run_start.is_null() && core::ptr::eq(key.as_ptr(), run_end);
        if child.is_empty() {
            if continues {
                run_end = run_end.wrapping_add(1);
            } else {
                if !run_start.is_null() {
                    // SAFETY: `run_start..run_end` are bytes of one node encoding
                    buffer.write_slice(unsafe { core::slice::from_ptr_range(run_start..run_end) });
                    run_start = core::ptr::null();
                }
                buffer.write_byte(0x80);
            }
        } else if continues {
            run_end = run_end.wrapping_add(key.len());
        } else {
            if !run_start.is_null() {
                // SAFETY: as above
                buffer.write_slice(unsafe { core::slice::from_ptr_range(run_start..run_end) });
            }
            run_start = key.as_ptr();
            run_end = key.as_ptr().wrapping_add(key.len());
        }
    }
    if !run_start.is_null() {
        // SAFETY: as above
        buffer.write_slice(unsafe { core::slice::from_ptr_range(run_start..run_end) });
    }
    // empty value at the end
    buffer.write_byte(0x80);
}

fn encode_branch<'a>(
    interner: &mut (impl Interner<'a> + 'a),
    children: &[ChildRef<'a>; 16],
) -> Result<&'a [u8], ()> {
    // empty value at the end
    let mut payload_len = 1;
    for child in children.iter() {
        debug_assert!(child.is_pending() == false);
        payload_len += if child.is_empty() { 1 } else { child.key.len() };
    }
    let total_len = list_encoding_prefix_len(payload_len) + payload_len;
    // The runs of unchanged children are long enough for a memcpy call to beat an inline
    // word copier (measured: the inline one lost ~20M cycles over the 16 blocks).
    let mut buffer = interner.get_buffer(total_len)?;
    write_branch_encoding(&mut buffer, children, payload_len);
    Ok(buffer.flush())
}

/// Reference of a node from its full encoding: hash for 32 bytes and longer, the node itself otherwise
fn make_ref<'a, I: Interner<'a> + 'a>(
    encoding: &'a [u8],
    interner: &mut I,
    hasher: &mut impl MiniDigest<HashOutput: core::ops::Deref<Target = [u8; 32]>>,
) -> Result<ChildRef<'a>, ()> {
    if encoding.len() >= 32 {
        hasher.update(encoding);
        let hash = hasher.finalize_reset();
        let hash: &[u8; 32] = &hash;
        if I::SUPPORTS_WORD_LEVEL_INTERNING {
            // The 33-byte reference is laid out so that the hash starts on a word boundary:
            // the prefix is the last byte of the first word, and the hash is copied by words.
            let mut buffer = interner.get_word_buffer(1 + 32 / WORD)?;
            buffer.write_word((0x80 + 32) << ((WORD - 1) * 8));
            if hash.as_ptr().addr() % WORD == 0 {
                for i in 0..32 / WORD {
                    // SAFETY: an aligned word of the hash
                    buffer.write_word(unsafe { hash.as_ptr().cast::<usize>().add(i).read() });
                }
            } else {
                for chunk in hash.chunks_exact(WORD) {
                    buffer.write_word(usize::from_le_bytes(chunk.try_into().unwrap()));
                }
            }
            Ok(ChildRef {
                key: &buffer.flush_as_bytes(WORD + 32)[WORD - 1..],
                encoding,
            })
        } else {
            let mut buffer = interner.get_buffer(33)?;
            buffer.write_byte(0x80 + 32);
            buffer.write_slice(hash);
            Ok(ChildRef {
                key: buffer.flush(),
                encoding,
            })
        }
    } else {
        Ok(ChildRef {
            key: encoding,
            encoding,
        })
    }
}

/// Full encoding of the referenced node, verified against the hash if it comes from the oracle
fn resolve_child<'a>(
    child: ChildRef<'a>,
    preimages_oracle: &mut impl PreimagesOracle,
    interner: &mut (impl Interner<'a> + 'a),
    hasher: &mut impl MiniDigest<HashOutput: core::ops::Deref<Target = [u8; 32]>>,
) -> Result<&'a [u8], ()> {
    if child.encoding.is_empty() == false {
        return Ok(child.encoding);
    }
    let key = child.key;
    if key.len() == 33 {
        if key[0] != 0x80 + 32 {
            return Err(());
        }
        let hash: &[u8; 32] = key[1..].try_into().map_err(|_| ())?;
        let raw = preimages_oracle.provide_preimage(hash, interner)?;
        hasher.update(raw);
        let recomputed = hasher.finalize_reset();
        // branchless byte loop: a slice comparison ends up as a `memcmp` call
        let mut diff = 0u8;
        for (a, b) in recomputed.iter().zip(hash.iter()) {
            diff |= a ^ b;
        }
        if diff != 0 {
            return Err(());
        }

        Ok(raw)
    } else if key.len() < 32 && child.is_empty() == false && child.is_pending() == false {
        // embedded node
        Ok(key)
    } else {
        Err(())
    }
}

pub struct StackMPT<'a, A: Allocator + Clone> {
    /// Fixed `MAX_LINES` slots, the first `num_lines` are the path from the root
    lines: Vec<Line<'a>, A>,
    num_lines: usize,
    /// High-water mark of `num_lines`: slots above it were never written
    max_num_lines: usize,
    root: ChildRef<'a>,
    prev_key: Option<TrieKey>,
    key: TrieKey,
    cursor: Cursor,
}

impl<'a, A: Allocator + Clone> StackMPT<'a, A> {
    pub fn new_in(allocator: A) -> Self {
        let mut lines = Vec::with_capacity_in(MAX_LINES, allocator);
        for _ in 0..MAX_LINES {
            lines.push(Line::VACANT);
        }

        Self {
            lines,
            num_lines: 0,
            max_num_lines: 0,
            root: ChildRef::EMPTY,
            prev_key: None,
            key: TrieKey::default(),
            cursor: Cursor::Invalid,
        }
    }

    /// Drop all the state and reinterpret for another interner lifetime
    pub fn purge_reborrow<'b>(self) -> StackMPT<'b, A>
    where
        A: 'b,
    {
        let Self {
            mut lines,
            max_num_lines,
            ..
        } = self;
        for line in lines[..max_num_lines].iter_mut() {
            line.kind = Kind::Empty;
            line.modified = false;
            line.value = &[];
            line.child = ChildRef::EMPTY;
            line.children = [ChildRef::EMPTY; 16];
        }
        // Safety: no slot holds a reference any longer
        let lines: Vec<Line<'b>, A> = unsafe { core::mem::transmute(lines) };

        StackMPT {
            lines,
            num_lines: 0,
            max_num_lines: 0,
            root: ChildRef::EMPTY,
            prev_key: None,
            key: TrieKey::default(),
            cursor: Cursor::Invalid,
        }
    }

    pub fn set_root(
        &mut self,
        root_hash: &[u8; 32],
        interner: &mut (impl Interner<'a> + 'a),
    ) -> Result<(), ()> {
        if self.num_lines != 0 || self.prev_key.is_some() {
            return Err(());
        }
        self.root = if *root_hash == EMPTY_ROOT_HASH.as_u8_array() {
            ChildRef::EMPTY
        } else {
            let mut buffer = interner.get_buffer(33)?;
            buffer.write_byte(0x80 + 32);
            buffer.write_slice(root_hash);
            ChildRef {
                key: buffer.flush(),
                encoding: &[],
            }
        };

        Ok(())
    }

    #[inline(always)]
    fn top(&self) -> Option<&Line<'a>> {
        self.num_lines
            .checked_sub(1)
            .map(|index| &self.lines[index])
    }

    #[inline(always)]
    fn top_mut(&mut self) -> Option<&mut Line<'a>> {
        self.num_lines
            .checked_sub(1)
            .map(|index| &mut self.lines[index])
    }

    /// Take a vacant slot on top of the stack. The node is written by the caller,
    /// and must be checked with `validate` afterwards
    #[inline(always)]
    fn push_slot(
        &mut self,
        depth: usize,
        parent_slot: u8,
        modified: bool,
    ) -> Result<&mut Line<'a>, ()> {
        if self.num_lines >= MAX_LINES {
            return Err(());
        }
        let line = &mut self.lines[self.num_lines];
        line.depth = depth as u8;
        line.parent_slot = parent_slot;
        line.modified = modified;
        self.num_lines += 1;
        if self.num_lines > self.max_num_lines {
            self.max_num_lines = self.num_lines;
        }

        Ok(line)
    }

    /// Push a new leaf for the current key with the segment starting at `depth`
    fn push_leaf(&mut self, depth: usize, parent_slot: u8, value: &'a [u8]) -> Result<(), ()> {
        let key = self.key;
        let line = self.push_slot(depth, parent_slot, true)?;
        line.kind = Kind::Leaf;
        line.segment.set_from_key(&key, depth);
        line.value = value;
        line.validate()
    }

    fn unfold(
        &mut self,
        child: ChildRef<'a>,
        depth: usize,
        parent_slot: u8,
        preimages_oracle: &mut impl PreimagesOracle,
        interner: &mut (impl Interner<'a> + 'a),
        hasher: &mut impl MiniDigest<HashOutput: core::ops::Deref<Target = [u8; 32]>>,
    ) -> Result<(), ()> {
        let raw = resolve_child(child, preimages_oracle, interner, hasher)?;
        let line = self.push_slot(depth, parent_slot, false)?;
        line.parse_into(raw)?;
        line.validate()
    }

    /// Descend to `key`, that must be greater than the key of the previous `seek`.
    /// Returns the data of the existing value (RLP item without the envelope), or `None` if there is
    /// no such key. Leaves the cursor at the key for `set` or `delete`.
    pub fn seek(
        &mut self,
        key: TrieKey,
        preimages_oracle: &mut impl PreimagesOracle,
        interner: &mut (impl Interner<'a> + 'a),
        hasher: &mut impl MiniDigest<HashOutput: core::ops::Deref<Target = [u8; 32]>>,
    ) -> Result<Option<&'a [u8]>, ()> {
        self.cursor = Cursor::Invalid;
        if let Some(prev_key) = self.prev_key {
            if key <= prev_key {
                return Err(());
            }
            let common_prefix_len = key.common_prefix_len(&prev_key);
            while let Some(top) = self.top() {
                if top.depth as usize > common_prefix_len {
                    self.fold_top(preimages_oracle, interner, hasher)?;
                } else {
                    break;
                }
            }
        } else {
            debug_assert_eq!(self.num_lines, 0);
        }
        self.prev_key = Some(key);
        self.key = key;

        if self.num_lines == 0 {
            if self.root.is_empty() {
                self.cursor = Cursor::EmptyTrie;
                return Ok(None);
            }
            self.unfold(self.root, 0, 0, preimages_oracle, interner, hasher)?;
        }

        loop {
            let top = self.top().expect("at least the root is unfolded");
            let depth = top.depth as usize;
            match top.kind {
                Kind::Empty => return Err(()),
                Kind::Branch => {
                    let nibble = key.nibble(depth);
                    let child = top.children[nibble as usize];
                    if child.is_empty() {
                        self.cursor = Cursor::EmptySlot { nibble };
                        return Ok(None);
                    }
                    self.unfold(child, depth + 1, nibble, preimages_oracle, interner, hasher)?;
                }
                Kind::Extension => {
                    let common = key.common_prefix_with_segment(depth, top.segment.as_slice());
                    if common == top.segment.len as usize {
                        let child = top.child;
                        if child.is_empty() {
                            self.cursor = Cursor::DeadExtension;
                            return Ok(None);
                        }
                        self.unfold(child, depth + common, 0, preimages_oracle, interner, hasher)?;
                    } else {
                        self.cursor = Cursor::DivergedExtension {
                            common: common as u8,
                        };
                        return Ok(None);
                    }
                }
                Kind::Leaf => {
                    let common = key.common_prefix_with_segment(depth, top.segment.as_slice());
                    if common == top.segment.len as usize {
                        let data = RLPSlice::from_slice(top.value)?.data();
                        self.cursor = Cursor::AtLeaf;
                        return Ok(Some(data));
                    } else {
                        self.cursor = Cursor::DivergedLeaf {
                            common: common as u8,
                        };
                        return Ok(None);
                    }
                }
            }
        }
    }

    /// Insert or update the value (full RLP item) at the key of the last `seek`
    pub fn set(
        &mut self,
        value: &[u8],
        interner: &mut (impl Interner<'a> + 'a),
        hasher: &mut impl MiniDigest<HashOutput: core::ops::Deref<Target = [u8; 32]>>,
    ) -> Result<(), ()> {
        let value = interner.intern_slice(value)?;
        let key = self.key;
        match self.cursor {
            Cursor::Invalid => return Err(()),
            Cursor::AtLeaf => {
                let top = self.top_mut().ok_or(())?;
                if top.kind != Kind::Leaf {
                    return Err(());
                }
                top.value = value;
                top.modified = true;
            }
            Cursor::EmptyTrie => {
                self.push_leaf(0, 0, value)?;
            }
            Cursor::EmptySlot { nibble } => {
                let top = self.top_mut().ok_or(())?;
                if top.kind != Kind::Branch {
                    return Err(());
                }
                let depth = top.depth as usize;
                top.children[nibble as usize] = ChildRef::PENDING;
                top.num_children += 1;
                top.modified = true;
                self.push_leaf(depth + 1, nibble, value)?;
            }
            Cursor::DeadExtension => {
                let top = self.top_mut().ok_or(())?;
                if top.kind != Kind::Extension {
                    return Err(());
                }
                let depth = top.depth as usize;
                top.kind = Kind::Leaf;
                top.segment.set_from_key(&key, depth);
                top.value = value;
                top.modified = true;
            }
            Cursor::DivergedExtension { common } => {
                let top = self.top_mut().ok_or(())?;
                if top.kind != Kind::Extension {
                    return Err(());
                }
                let child = top.child;
                if child.is_empty() {
                    // the whole subtree under the extension was deleted, so the new leaf takes its place
                    let depth = top.depth as usize;
                    top.kind = Kind::Leaf;
                    top.segment.set_from_key(&key, depth);
                    top.value = value;
                    top.modified = true;
                    self.cursor = Cursor::AtLeaf;
                    return Ok(());
                }
                let common = common as usize;
                let segment = top.segment;
                let old_nibble = segment.nibbles[common];
                let rest = &segment.nibbles[(common + 1)..(segment.len as usize)];
                let old_ref = if rest.is_empty() {
                    child
                } else {
                    let encoding = encode_short_node(interner, false, rest, child.key)?;
                    make_ref(encoding, interner, hasher)?
                };
                self.split(common, old_nibble, old_ref, value)?;
            }
            Cursor::DivergedLeaf { common } => {
                let top = self.top_mut().ok_or(())?;
                if top.kind != Kind::Leaf {
                    return Err(());
                }
                let old_value = top.value;
                let common = common as usize;
                let segment = top.segment;
                let old_nibble = segment.nibbles[common];
                let rest = &segment.nibbles[(common + 1)..(segment.len as usize)];
                let encoding = encode_short_node(interner, true, rest, old_value)?;
                let old_ref = make_ref(encoding, interner, hasher)?;
                self.split(common, old_nibble, old_ref, value)?;
            }
        }
        self.cursor = Cursor::AtLeaf;

        Ok(())
    }

    /// Replace the top line (leaf or extension with segment diverging at `common`) by
    /// [extension of common prefix ->] branch with the old node at `old_nibble` and a new leaf
    fn split(
        &mut self,
        common: usize,
        old_nibble: u8,
        old_ref: ChildRef<'a>,
        value: &'a [u8],
    ) -> Result<(), ()> {
        let key = self.key;
        let top_index = self.num_lines.checked_sub(1).ok_or(())?;
        let top = &mut self.lines[top_index];
        let depth = top.depth as usize;
        let new_nibble = key.nibble(depth + common);
        if new_nibble == old_nibble {
            return Err(());
        }
        let branch_index = if common == 0 {
            top_index
        } else {
            if top.kind != Kind::Leaf && top.kind != Kind::Extension {
                return Err(());
            }
            top.kind = Kind::Extension;
            top.segment.len = common as u8;
            top.child = ChildRef::PENDING;
            top.modified = true;
            top.validate()?;
            self.push_slot(depth + common, 0, true)?;
            top_index + 1
        };
        let branch = &mut self.lines[branch_index];
        branch.kind = Kind::Branch;
        branch.modified = true;
        branch.children = [ChildRef::EMPTY; 16];
        branch.children[old_nibble as usize] = old_ref;
        branch.children[new_nibble as usize] = ChildRef::PENDING;
        branch.num_children = 2;
        branch.validate()?;

        self.push_leaf(depth + common + 1, new_nibble, value)
    }

    /// Delete the leaf found by the last `seek`
    pub fn delete(&mut self) -> Result<(), ()> {
        if self.cursor != Cursor::AtLeaf {
            return Err(());
        }
        self.cursor = Cursor::Invalid;
        if self.num_lines == 0 {
            return Err(());
        }
        let (parents, top) = self.lines.split_at_mut(self.num_lines - 1);
        let leaf = &top[0];
        if leaf.kind != Kind::Leaf {
            return Err(());
        }
        match parents.last_mut() {
            None => {
                self.root = ChildRef::EMPTY;
            }
            Some(parent) => {
                if parent.kind != Kind::Branch {
                    return Err(());
                }
                parent.children[leaf.parent_slot as usize] = ChildRef::EMPTY;
                parent.num_children -= 1;
                parent.modified = true;
            }
        }
        self.num_lines -= 1;

        Ok(())
    }

    fn fold_top(
        &mut self,
        preimages_oracle: &mut impl PreimagesOracle,
        interner: &mut (impl Interner<'a> + 'a),
        hasher: &mut impl MiniDigest<HashOutput: core::ops::Deref<Target = [u8; 32]>>,
    ) -> Result<(), ()> {
        if self.num_lines == 0 {
            return Err(());
        }
        let (parents, top) = self.lines.split_at_mut(self.num_lines - 1);
        // the slots above the top are free and serve as scratch space
        let (line, scratch) = top.split_at_mut(1);
        let line = &line[0];
        let mut scratch_used = false;
        if line.modified == false {
            // parent still references it by the original key
            self.num_lines -= 1;
            return Ok(());
        }
        let folded = match line.kind {
            Kind::Empty => return Err(()),
            Kind::Leaf => Folded::Encoded(encode_short_node(
                interner,
                true,
                line.segment.as_slice(),
                line.value,
            )?),
            Kind::Extension => {
                if line.child.is_empty() {
                    Folded::Empty
                } else {
                    if line.child.is_pending() {
                        return Err(());
                    }
                    Folded::Encoded(encode_short_node(
                        interner,
                        false,
                        line.segment.as_slice(),
                        line.child.key,
                    )?)
                }
            }
            Kind::Branch => match line.num_children {
                0 => Folded::Empty,
                1 => {
                    let (nibble, child) = line
                        .children
                        .iter()
                        .enumerate()
                        .find(|(_, child)| child.is_empty() == false)
                        .ok_or(())?;
                    let nibble = nibble as u8;
                    let raw = resolve_child(*child, preimages_oracle, interner, hasher)?;
                    // parse the remaining child into the free slot above the top
                    let Some(collapsed) = scratch.first_mut() else {
                        return Err(());
                    };
                    collapsed.parse_into(raw)?;
                    scratch_used = true;
                    match collapsed.kind {
                        Kind::Empty => return Err(()),
                        Kind::Leaf => Folded::Merged {
                            is_leaf: true,
                            segment: Segment::with_prefix(&[nibble], collapsed.segment.as_slice())?,
                            payload: collapsed.value,
                        },
                        Kind::Extension => Folded::Merged {
                            is_leaf: false,
                            segment: Segment::with_prefix(&[nibble], collapsed.segment.as_slice())?,
                            payload: collapsed.child.key,
                        },
                        Kind::Branch => Folded::Merged {
                            is_leaf: false,
                            segment: Segment::with_prefix(&[nibble], &[])?,
                            payload: child.key,
                        },
                    }
                }
                _ => Folded::Encoded(encode_branch(interner, &line.children)?),
            },
        };
        let line_parent_slot = line.parent_slot;

        match parents.last_mut() {
            None => {
                self.root = match folded {
                    Folded::Empty => ChildRef::EMPTY,
                    Folded::Encoded(encoding) => make_ref(encoding, interner, hasher)?,
                    Folded::Merged {
                        is_leaf,
                        segment,
                        payload,
                    } => {
                        let encoding =
                            encode_short_node(interner, is_leaf, segment.as_slice(), payload)?;
                        make_ref(encoding, interner, hasher)?
                    }
                };
            }
            Some(parent) => {
                parent.modified = true;
                match parent.kind {
                    Kind::Branch => {
                        let slot = &mut parent.children[line_parent_slot as usize];
                        debug_assert!(slot.is_pending() || slot.is_empty() == false);
                        match folded {
                            Folded::Empty => {
                                *slot = ChildRef::EMPTY;
                                parent.num_children -= 1;
                            }
                            Folded::Encoded(encoding) => {
                                *slot = make_ref(encoding, interner, hasher)?;
                            }
                            Folded::Merged {
                                is_leaf,
                                segment,
                                payload,
                            } => {
                                let encoding = encode_short_node(
                                    interner,
                                    is_leaf,
                                    segment.as_slice(),
                                    payload,
                                )?;
                                *slot = make_ref(encoding, interner, hasher)?;
                            }
                        }
                    }
                    Kind::Extension => match folded {
                        Folded::Empty => {
                            parent.child = ChildRef::EMPTY;
                        }
                        Folded::Encoded(encoding) => {
                            parent.child = make_ref(encoding, interner, hasher)?;
                        }
                        Folded::Merged {
                            is_leaf,
                            segment: merged_segment,
                            payload,
                        } => {
                            let joined = Segment::with_prefix(
                                parent.segment.as_slice(),
                                merged_segment.as_slice(),
                            )?;
                            parent.segment = joined;
                            if is_leaf {
                                parent.kind = Kind::Leaf;
                                parent.value = payload;
                            } else {
                                parent.child = ChildRef {
                                    key: payload,
                                    encoding: &[],
                                };
                            }
                        }
                    },
                    Kind::Leaf | Kind::Empty => return Err(()),
                }
            }
        }
        if scratch_used && self.num_lines + 1 > self.max_num_lines {
            self.max_num_lines = self.num_lines + 1;
        }
        self.num_lines -= 1;

        Ok(())
    }

    /// Fold everything and return the new root hash
    pub fn finalize(
        &mut self,
        preimages_oracle: &mut impl PreimagesOracle,
        interner: &mut (impl Interner<'a> + 'a),
        hasher: &mut impl MiniDigest<HashOutput: core::ops::Deref<Target = [u8; 32]>>,
    ) -> Result<[u8; 32], ()> {
        while self.num_lines > 0 {
            self.fold_top(preimages_oracle, interner, hasher)?;
        }
        self.cursor = Cursor::Invalid;
        if self.root.is_empty() {
            Ok(EMPTY_ROOT_HASH.as_u8_array())
        } else if self.root.key.len() == 33 {
            self.root.key[1..].try_into().map_err(|_| ())
        } else {
            hasher.update(self.root.key);
            Ok(*hasher.finalize_reset())
        }
    }
}
