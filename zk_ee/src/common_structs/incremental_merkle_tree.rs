use core::marker::PhantomData;

use crate::utils::Bytes32;
use crypto::MiniDigest;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IncrementalMerkleTreeFull;

/// Append-only fixed-depth incremental Merkle tree over already-hashed leaves.
///
/// The tree keeps only the current length and a frontier, not all leaves or
/// internal nodes. The frontier is the binary decomposition of `len`: if bit
/// `i` of `len` is set, `frontier[i]` is the root of the rightmost complete
/// subtree of size `2^i` in the appended prefix. Appending a leaf updates this
/// frontier with the same carry logic as incrementing a binary counter.
///
pub struct IncrementalMerkleTree<'a, const DEPTH: usize, H> {
    len: u64,
    frontier: [Bytes32; DEPTH],
    empty_hashes: &'a [Bytes32; DEPTH],
    completed_root: Bytes32,
    _marker: PhantomData<H>,
}

impl<'a, const DEPTH: usize, H> IncrementalMerkleTree<'a, DEPTH, H>
where
    H: MiniDigest<HashOutput = [u8; 32]>,
{
    /// Precomputes empty subtree hashes for this tree depth and hasher.
    ///
    /// `empty_hashes[i]` is the root of an empty subtree of height `i`, so
    /// `empty_hashes[0]` is the empty leaf hash. For every following level,
    /// `empty_hashes[i] = H(empty_hashes[i - 1] || empty_hashes[i - 1])`.
    pub fn empty_hashes() -> [Bytes32; DEPTH] {
        assert!(DEPTH < u64::BITS as usize);

        let mut empty_hashes = [Bytes32::ZERO; DEPTH];
        let mut level = 1;
        while level < DEPTH {
            empty_hashes[level] = Self::hash_pair(empty_hashes[level - 1], empty_hashes[level - 1]);
            level += 1;
        }

        empty_hashes
    }

    /// Creates an empty tree borrowing a precomputed empty-hash table.
    ///
    /// Multiple trees with the same `DEPTH` and hasher can share one table.
    /// The caller must pass hashes generated for this exact hasher.
    pub fn new(empty_hashes: &'a [Bytes32; DEPTH]) -> Self {
        assert!(DEPTH < u64::BITS as usize);

        Self {
            len: 0,
            frontier: [Bytes32::ZERO; DEPTH],
            empty_hashes,
            completed_root: Bytes32::ZERO,
            _marker: PhantomData,
        }
    }

    pub fn len(&self) -> u64 {
        self.len
    }

    /// Appends one already-hashed leaf and returns its zero-based index.
    ///
    /// The frontier is the compact representation of the already-filled prefix
    /// of the tree. Level `i` stores the root of the rightmost complete subtree
    /// of size `2^i`, and that slot is meaningful exactly when bit `i` of
    /// `len` is set.
    ///
    /// Appending mirrors binary increment. A zero bit means the carried node
    /// becomes the new frontier entry at that level. A one bit means the
    /// frontier already has a complete left sibling there, so we hash
    /// `frontier[level] || carried_node` and carry the parent to the next level.
    pub fn append(&mut self, leaf_hash: Bytes32) -> Result<u64, IncrementalMerkleTreeFull> {
        if self.len >= Self::capacity() {
            return Err(IncrementalMerkleTreeFull);
        }

        let index = self.len;
        let mut node = leaf_hash;
        let mut size = self.len;
        let mut level = 0;
        while level < DEPTH {
            // First zero bit in the old length: the carried node now closes the
            // prefix decomposition at this level.
            if size & 1 == 0 {
                self.frontier[level] = node;
                self.len += 1;
                return Ok(index);
            }

            // One bit in the old length: merge the existing complete subtree
            // with the carried node and continue the carry.
            node = Self::hash_pair(self.frontier[level], node);
            size >>= 1;
            level += 1;
        }

        self.completed_root = node;
        self.len += 1;

        Ok(index)
    }

    pub fn root(&self) -> Bytes32 {
        if DEPTH == 0 {
            if self.len == 0 {
                return Bytes32::ZERO;
            }

            return self.completed_root;
        }

        if self.len == Self::capacity() {
            return self.completed_root;
        }

        let mut node = self.empty_hashes[0];
        let mut size = self.len;
        let mut level = 0;
        while level < DEPTH {
            if size & 1 == 1 {
                node = Self::hash_pair(self.frontier[level], node);
            } else {
                node = Self::hash_pair(node, self.empty_hashes[level]);
            }

            size >>= 1;
            level += 1;
        }

        node
    }

    fn capacity() -> u64 {
        1u64 << DEPTH
    }

    fn hash_pair(left: Bytes32, right: Bytes32) -> Bytes32 {
        let mut hasher = H::new();
        hasher.update(left.as_u8_ref());
        hasher.update(right.as_u8_ref());
        Bytes32::from_array(hasher.finalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    use crypto::blake2s::Blake2s256;

    fn leaf(value: u8) -> Bytes32 {
        Bytes32::from_array(Blake2s256::digest([value]))
    }

    fn reference_root<const DEPTH: usize>(leaves: &[Bytes32]) -> Bytes32 {
        if DEPTH == 0 {
            return leaves.first().copied().unwrap_or(Bytes32::ZERO);
        }

        let mut level = Vec::new();
        level.resize(1usize << DEPTH, Bytes32::ZERO);
        level[..leaves.len()].copy_from_slice(leaves);

        let mut width = 1usize << DEPTH;
        while width > 1 {
            for i in 0..(width / 2) {
                level[i] = IncrementalMerkleTree::<'_, DEPTH, Blake2s256>::hash_pair(
                    level[i * 2],
                    level[i * 2 + 1],
                );
            }
            width /= 2;
        }

        level[0]
    }

    #[test]
    fn empty_root_matches_full_tree() {
        let empty_hashes = IncrementalMerkleTree::<'_, 4, Blake2s256>::empty_hashes();
        let tree = IncrementalMerkleTree::<'_, 4, Blake2s256>::new(&empty_hashes);

        assert_eq!(tree.len(), 0);
        assert_eq!(tree.root(), reference_root::<4>(&[]));
    }

    #[test]
    fn root_matches_full_tree_after_each_append() {
        let empty_hashes = IncrementalMerkleTree::<'_, 4, Blake2s256>::empty_hashes();
        let mut tree = IncrementalMerkleTree::<'_, 4, Blake2s256>::new(&empty_hashes);
        let mut leaves = Vec::new();

        for i in 0..16 {
            let leaf = leaf(i as u8);
            assert_eq!(tree.append(leaf), Ok(i));
            leaves.push(leaf);
            assert_eq!(tree.root(), reference_root::<4>(&leaves));
        }

        assert_eq!(tree.append(leaf(16)), Err(IncrementalMerkleTreeFull));
    }

    #[test]
    fn depth_zero_tree_accepts_one_leaf() {
        let empty_hashes = IncrementalMerkleTree::<'_, 0, Blake2s256>::empty_hashes();
        let mut tree = IncrementalMerkleTree::<'_, 0, Blake2s256>::new(&empty_hashes);

        assert_eq!(tree.root(), Bytes32::ZERO);
        assert_eq!(tree.append(leaf(7)), Ok(0));
        assert_eq!(tree.root(), leaf(7));
        assert_eq!(tree.append(leaf(8)), Err(IncrementalMerkleTreeFull));
    }

    #[test]
    fn empty_hashes_can_be_shared_between_trees() {
        let empty_hashes = IncrementalMerkleTree::<'_, 4, Blake2s256>::empty_hashes();
        let mut first = IncrementalMerkleTree::<'_, 4, Blake2s256>::new(&empty_hashes);
        let mut second = IncrementalMerkleTree::<'_, 4, Blake2s256>::new(&empty_hashes);

        assert_eq!(first.append(leaf(1)), Ok(0));
        assert_eq!(second.append(leaf(2)), Ok(0));

        assert_eq!(first.root(), reference_root::<4>(&[leaf(1)]));
        assert_eq!(second.root(), reference_root::<4>(&[leaf(2)]));
    }
}
