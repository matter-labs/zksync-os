//! Fixed-height binary Merkle tree built from a full set of leaves.
//!
//! Missing right-hand nodes at each level are filled with the corresponding
//! empty-subtree hash, which is equivalent to padding the leaf layer up to
//! the full `2^height` capacity with the empty leaf.

use crate::utils::Bytes32;
use crypto::MiniDigest;

/// Returns the root of a fixed-height binary Merkle tree whose leaves are
/// `nodes`, padded up to `2^height` with the empty leaf.
///
/// `nodes` is the leaf slice and is **overwritten** as scratch while folding;
/// on return its contents are meaningless (only the returned root matters).
///
/// `empty_subtree_hashes[i]` is the root of an empty subtree of height `i`
/// (index `0` = empty leaf, index `height` = all-empty root), and its length
/// sets `height = len - 1`. Empty input returns `empty_subtree_hashes[height]`.
///
/// Precondition: `nodes.len() <= 2^height` (call sites bound leaves well below
/// capacity, so this is not reachable from input).
pub fn merkle_root_in_place<H>(nodes: &mut [Bytes32], empty_subtree_hashes: &[Bytes32]) -> Bytes32
where
    H: MiniDigest<HashOutput = [u8; 32]>,
{
    let height = empty_subtree_hashes.len() - 1;

    let mut count = nodes.len();
    if count == 0 {
        return empty_subtree_hashes[height];
    }

    let mut hasher = H::new();
    #[allow(clippy::needless_range_loop)]
    for level in 0..height {
        let pairs = count.div_ceil(2);
        for i in 0..pairs {
            hasher.update(nodes[i * 2].as_u8_ref());
            if i * 2 + 1 < count {
                hasher.update(nodes[i * 2 + 1].as_u8_ref());
            } else {
                hasher.update(empty_subtree_hashes[level].as_u8_ref());
            }
            nodes[i] = Bytes32::from_array(hasher.finalize_reset());
        }
        count = pairs;
    }

    nodes[0]
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    use crypto::blake2s::Blake2s256;
    use crypto::sha3::Keccak256;

    /// Test-only recomputation of the empty-subtree hashes. Production code uses
    /// hardcoded tables; the tests regenerate the recurrence to lock those tables.
    fn empty_subtree_hashes<H>(empty_leaf: Bytes32, height: usize) -> Vec<Bytes32>
    where
        H: MiniDigest<HashOutput = [u8; 32]>,
    {
        let mut hashes = Vec::with_capacity(height + 1);
        hashes.push(empty_leaf);

        let mut hasher = H::new();
        for level in 1..=height {
            let prev = hashes[level - 1];
            hasher.update(prev.as_u8_ref());
            hasher.update(prev.as_u8_ref());
            hashes.push(Bytes32::from_array(hasher.finalize_reset()));
        }

        hashes
    }

    fn hash_pair<H>(left: Bytes32, right: Bytes32) -> Bytes32
    where
        H: MiniDigest<HashOutput = [u8; 32]>,
    {
        let mut hasher = H::new();
        hasher.update(left.as_u8_ref());
        hasher.update(right.as_u8_ref());
        Bytes32::from_array(hasher.finalize())
    }

    /// Straightforward reference: pad the leaf layer to `2^height` with the
    /// empty leaf and fold a complete binary tree. Equivalent to the (now
    /// removed) incremental tree's `reference_root` and to its `root()`.
    fn reference_root<H>(leaves: &[Bytes32], empty_leaf: Bytes32, height: usize) -> Bytes32
    where
        H: MiniDigest<HashOutput = [u8; 32]>,
    {
        let width = 1usize << height;
        assert!(leaves.len() <= width);

        let mut level = Vec::with_capacity(width);
        level.extend_from_slice(leaves);
        level.resize(width, empty_leaf);

        let mut w = width;
        while w > 1 {
            for i in 0..(w / 2) {
                level[i] = hash_pair::<H>(level[i * 2], level[i * 2 + 1]);
            }
            w /= 2;
        }

        level[0]
    }

    fn blake_leaf(value: u32) -> Bytes32 {
        Bytes32::from_array(Blake2s256::digest(value.to_le_bytes()))
    }

    #[test]
    fn matches_full_padded_tree_blake2s() {
        // Transitively proves equivalence with the removed incremental tree,
        // which was validated against the same full-padded reference.
        const HEIGHT: usize = 8;
        let empty = empty_subtree_hashes::<Blake2s256>(Bytes32::ZERO, HEIGHT);

        let all_leaves: Vec<Bytes32> = (0..(1u32 << HEIGHT)).map(blake_leaf).collect();
        for count in 0..=all_leaves.len() {
            let mut scratch = all_leaves[..count].to_vec();
            let got = merkle_root_in_place::<Blake2s256>(&mut scratch, &empty);
            let expected =
                reference_root::<Blake2s256>(&all_leaves[..count], Bytes32::ZERO, HEIGHT);
            assert_eq!(got, expected, "mismatch at count {count}");
        }
    }

    #[test]
    fn empty_and_single_leaf_edge_cases() {
        const HEIGHT: usize = 4;
        let empty = empty_subtree_hashes::<Blake2s256>(Bytes32::ZERO, HEIGHT);

        let mut none: Vec<Bytes32> = Vec::new();
        assert_eq!(
            merkle_root_in_place::<Blake2s256>(&mut none, &empty),
            empty[HEIGHT]
        );

        let leaf = blake_leaf(42);
        let mut one = alloc::vec![leaf];
        assert_eq!(
            merkle_root_in_place::<Blake2s256>(&mut one, &empty),
            reference_root::<Blake2s256>(&[leaf], Bytes32::ZERO, HEIGHT)
        );
    }

    #[test]
    fn keccak_empty_hashes_match_logs_table() {
        // Locks the L2->L1 logs root: the generic empty-hash recurrence over the
        // empty-log leaf must reproduce the hardcoded table used historically.
        use crate::common_structs::logs_storage::{
            L2_TO_L1_LOG_EMPTY_SUBTREE_HASHES, L2_TO_L1_LOG_SERIALIZE_SIZE,
            L2_TO_L1_LOG_TREE_HEIGHT,
        };

        let empty_leaf = Bytes32::from_array(Keccak256::digest([0u8; L2_TO_L1_LOG_SERIALIZE_SIZE]));
        let generic = empty_subtree_hashes::<Keccak256>(empty_leaf, L2_TO_L1_LOG_TREE_HEIGHT);

        let table: Vec<Bytes32> = L2_TO_L1_LOG_EMPTY_SUBTREE_HASHES
            .iter()
            .map(|h| Bytes32::from_array(*h))
            .collect();

        assert_eq!(generic, table);
    }
}
