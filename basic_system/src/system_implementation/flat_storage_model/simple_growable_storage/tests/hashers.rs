use super::*;

/// Every panic site of the implementations has a check, and every entry of the table is a
/// panic site.

#[test]
fn direct_hasher_is_used_by_fold() {
    let tree = TestingTree::<false>::new_in(Global);
    let key = WarmStorageKey {
        address: B160::ZERO,
        key: Bytes32::from_byte_fill(7),
    };
    let entries = [entry(&tree, key, Access::Write(Bytes32::from_byte_fill(9)))];
    let runs = diff_run(&tree, &entries, no_tamper);
    let _ = runs.accepted();
    assert_eq!(runs.reference.counts[Counter::DirectLeaf as usize], 0);
    assert_eq!(runs.reference.counts[Counter::DirectNode as usize], 0);
    assert!(runs.hardened.counts[Counter::DirectLeaf as usize] > 1);
    // the empty-subtree hashes come from the table, so these are all the fold's own: at least
    // the read hash at each of the 64 levels
    assert!(runs.hardened.counts[Counter::DirectNode as usize] >= 64);
}

pub(super) fn software_node(left: &Bytes32, right: &Bytes32) -> Bytes32 {
    let mut input = [0u8; 64];
    input[..32].copy_from_slice(left.as_u8_array_ref());
    input[32..].copy_from_slice(right.as_u8_array_ref());
    Bytes32::from_array(crypto::blake2_ext::Blake2s256::digest(input).into())
}

pub(super) fn software_leaf(leaf: &FlatStorageLeaf<64>) -> Bytes32 {
    let mut input = [0u8; 72];
    input[..32].copy_from_slice(leaf.key.as_u8_array_ref());
    input[32..64].copy_from_slice(leaf.value.as_u8_array_ref());
    input[64..].copy_from_slice(&leaf.next.to_le_bytes());
    Bytes32::from_array(crypto::blake2_ext::Blake2s256::digest(input).into())
}

#[test]
fn direct_hasher_known_answers() {
    let mut direct = DirectBlake2sStorageHasher::new();
    let mut leaf = FlatStorageLeaf::<64>::empty();
    assert_eq!(
        direct.hash_leaf(&leaf),
        hex_bytes("e3cdc93b3c2beb30f6a7c7cc45a32da012df9ae1be880e2c074885cb3f4e1e53")
    );
    leaf.next = 1u64 << 32;
    assert_eq!(
        direct.hash_leaf(&leaf),
        hex_bytes("0806c42e34965a5a3e80aa293a4f0fa1ebd378cb9cc55e0b19c4c34da515eadc")
    );
    assert_eq!(
        direct.hash_node(&Bytes32::ZERO, &Bytes32::ZERO),
        hex_bytes("ae09db7cd54f42b490ef09b6bc541af688e4959bb8c53f359a6f56e38ab454a3")
    );
    assert_eq!(
        direct.hash_node(&Bytes32::MAX, &Bytes32::MAX),
        hex_bytes("ee92d9d765aac654bc062ea8e24456bb1a73c86643bdac7ec4ff2efd81721063")
    );
    assert_eq!(
        direct.hash_node(&Bytes32::ZERO, &Bytes32::MAX),
        hex_bytes("1c130a470ac3ba94f7978f499885c60ae19621c958b5aae65e2b39c96abc6f57")
    );
}

#[test]
fn direct_hasher_edge_grid_and_interleaving() {
    let mut inputs = vec![Bytes32::ZERO, Bytes32::MAX];
    for index in 0..32 {
        for bit in [0x01, 0x80] {
            let mut bytes = [0u8; 32];
            bytes[index] = bit;
            inputs.push(Bytes32::from_array(bytes));
        }
    }
    for index in 0..8 {
        let mut bytes = [0u8; 32];
        bytes[4 * index..4 * index + 4].fill(0xff);
        inputs.push(Bytes32::from_array(bytes));
    }
    let next_values = [
        0,
        1,
        (1u64 << 32) - 1,
        1u64 << 32,
        (1u64 << 32) + 1,
        1u64 << 63,
        u64::MAX,
    ];
    let mut direct = DirectBlake2sStorageHasher::new();
    let mut reference = Blake2sStorageHasher::new();
    for (i, left) in inputs.iter().enumerate() {
        for right in &inputs {
            let expected = software_node(left, right);
            assert_eq!(direct.hash_node(left, right), expected);
            assert_eq!(reference.hash_node(left, right), expected);
        }
        for &next in &next_values {
            let leaf = FlatStorageLeaf::<64> {
                key: *left,
                value: inputs[(i + 1) % inputs.len()],
                next,
            };
            let expected = software_leaf(&leaf);
            assert_eq!(direct.hash_leaf(&leaf), expected);
            assert_eq!(reference.hash_leaf(&leaf), expected);
        }
        for bit in 0..64 {
            let leaf = FlatStorageLeaf::<64> {
                key: *left,
                value: inputs[(i + 2) % inputs.len()],
                next: 1u64 << bit,
            };
            assert_eq!(direct.hash_leaf(&leaf), software_leaf(&leaf));
        }
    }
    let empty_direct =
        compute_empty_hashes::<64, DirectBlake2sStorageHasher, Global>(&mut direct, Global);
    let empty_reference =
        compute_empty_hashes::<64, Blake2sStorageHasher, Global>(&mut reference, Global);
    assert_eq!(empty_direct, empty_reference);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]
    #[test]
    fn direct_hasher_matches_reference(
        left in any::<[u8; 32]>(),
        right in any::<[u8; 32]>(),
        next in any::<u64>(),
    ) {
        let left = Bytes32::from_array(left);
        let right = Bytes32::from_array(right);
        let leaf = FlatStorageLeaf::<64> { key: left, value: right, next };
        let mut direct = DirectBlake2sStorageHasher::new();
        let mut reference = Blake2sStorageHasher::new();
        prop_assert_eq!(direct.hash_node(&left, &right), reference.hash_node(&left, &right));
        prop_assert_eq!(direct.hash_node(&left, &right), software_node(&left, &right));
        prop_assert_eq!(direct.hash_leaf(&leaf), reference.hash_leaf(&leaf));
        prop_assert_eq!(direct.hash_leaf(&leaf), software_leaf(&leaf));
    }
}

/// The table holds the same hash constants as the recurrence, with either hasher of the fold, as
/// the host tree, and as an independent blake2s.
#[test]
fn blake2s_empty_subtree_hashes_match_the_recurrence() {
    let table = &BLAKE2S_EMPTY_SUBTREE_HASHES;
    let mut reference = Blake2sStorageHasher::new();
    let computed = compute_empty_hashes::<64, Blake2sStorageHasher, Global>(&mut reference, Global);
    assert_eq!(*computed, *table);
    let mut direct = DirectBlake2sStorageHasher::new();
    let computed =
        compute_empty_hashes::<64, DirectBlake2sStorageHasher, Global>(&mut direct, Global);
    assert_eq!(*computed, *table);
    // the host tree keeps them from the root down, with the empty leaf last
    let tree = TestingTree::<false>::new_in(Global);
    for (height, hash) in table.as_slice().iter().enumerate() {
        assert_eq!(tree.empty_hashes[TESTING_TREE_HEIGHT - height], *hash);
    }
    assert_eq!(table[0], software_leaf(&FlatStorageLeaf::empty()));
    for [below, above] in table.array_windows::<2>() {
        assert_eq!(*above, software_node(below, below));
    }
}
