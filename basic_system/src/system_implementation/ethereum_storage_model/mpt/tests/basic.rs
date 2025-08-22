use super::*;

#[test]
fn insert_close_to_make_branch() {
    let mut interner = BoxInterner::with_capacity_in(1 << 26, Global);
    let mut hasher = Keccak256::new();
    let mut trie =
        EthereumMPT::new_in(EMPTY_ROOT_HASH.as_u8_array(), &mut interner, Global).unwrap();
    let mut preimages_oracle = BTreeMap::new();

    let path_1 = hex_path_to_path_digits("123");
    let value_1 = rlp_encode_short_slice(&[0xff]);
    let path_2 = hex_path_to_path_digits("128");
    let value_2 = rlp_encode_short_slice(&[0x11]);

    trie.insert(
        Path::new(&path_1),
        &value_1,
        &mut preimages_oracle,
        &mut interner,
        &mut hasher,
    )
    .unwrap();
    trie.insert(
        Path::new(&path_2),
        &value_2,
        &mut preimages_oracle,
        &mut interner,
        &mut hasher,
    )
    .unwrap();

    let _ = trie.recompute(&mut interner, &mut hasher);

    let v_1 = trie
        .get(
            Path::new(&path_1),
            &mut preimages_oracle,
            &mut interner,
            &mut hasher,
        )
        .unwrap();
    assert_eq!(v_1, rlp_parse_short_bytes(&value_1).unwrap());
    let v_2 = trie
        .get(
            Path::new(&path_2),
            &mut preimages_oracle,
            &mut interner,
            &mut hasher,
        )
        .unwrap();
    assert_eq!(v_2, rlp_parse_short_bytes(&value_2).unwrap());
    assert!(trie.root.is_extension());
}

#[test]
fn insert_close_to_make_extension_branch_leaf() {
    let mut interner = BoxInterner::with_capacity_in(1 << 26, Global);
    let mut hasher = Keccak256::new();
    let mut trie =
        EthereumMPT::new_in(EMPTY_ROOT_HASH.as_u8_array(), &mut interner, Global).unwrap();
    let mut preimages_oracle = BTreeMap::new();

    let path_1 = hex_path_to_path_digits("143");
    let value_1 = rlp_encode_short_slice(&[0xff]);
    let path_2 = hex_path_to_path_digits("158");
    let value_2 = rlp_encode_short_slice(&[0x11]);

    trie.insert(
        Path::new(&path_1),
        &value_1,
        &mut preimages_oracle,
        &mut interner,
        &mut hasher,
    )
    .unwrap();
    trie.insert(
        Path::new(&path_2),
        &value_2,
        &mut preimages_oracle,
        &mut interner,
        &mut hasher,
    )
    .unwrap();

    let _ = trie.recompute(&mut interner, &mut hasher);

    let v_1 = trie
        .get(
            Path::new(&path_1),
            &mut preimages_oracle,
            &mut interner,
            &mut hasher,
        )
        .unwrap();
    assert_eq!(v_1, rlp_parse_short_bytes(&value_1).unwrap());
    let v_2 = trie
        .get(
            Path::new(&path_2),
            &mut preimages_oracle,
            &mut interner,
            &mut hasher,
        )
        .unwrap();
    assert_eq!(v_2, rlp_parse_short_bytes(&value_2).unwrap());

    assert!(trie.root.is_extension());
}

#[test]
fn insert_compute_delete_compute() {
    let mut interner = BoxInterner::with_capacity_in(1 << 26, Global);
    let mut hasher = Keccak256::new();
    let mut trie =
        EthereumMPT::new_in(EMPTY_ROOT_HASH.as_u8_array(), &mut interner, Global).unwrap();
    let mut preimages_oracle = BTreeMap::new();

    let path_1 = hex_path_to_path_digits("143");
    let value_1 = rlp_encode_short_slice(&[0xff]);
    let path_2 = hex_path_to_path_digits("158");
    let value_2 = rlp_encode_short_slice(&[0x11]);

    trie.insert(
        Path::new(&path_1),
        &value_1,
        &mut preimages_oracle,
        &mut interner,
        &mut hasher,
    )
    .unwrap();
    trie.insert(
        Path::new(&path_2),
        &value_2,
        &mut preimages_oracle,
        &mut interner,
        &mut hasher,
    )
    .unwrap();

    let _ = trie.recompute(&mut interner, &mut hasher);

    trie.delete(
        Path::new(&path_1),
        &mut preimages_oracle,
        &mut interner,
        &mut hasher,
    )
    .unwrap();
    trie.delete(
        Path::new(&path_2),
        &mut preimages_oracle,
        &mut interner,
        &mut hasher,
    )
    .unwrap();

    let _ = trie.recompute(&mut interner, &mut hasher);
    assert!(trie.root.is_empty());
    assert_eq!(&trie.root(&mut hasher), EMPTY_ROOT_HASH.as_u8_ref());
}

#[test]
fn update_back_and_forth() {
    let mut interner = BoxInterner::with_capacity_in(1 << 26, Global);
    let mut hasher = Keccak256::new();
    let mut trie =
        EthereumMPT::new_in(EMPTY_ROOT_HASH.as_u8_array(), &mut interner, Global).unwrap();
    let mut preimages_oracle = BTreeMap::new();

    let path_1 = hex_path_to_path_digits("123");
    let value_1 = rlp_encode_short_slice(&[0xff]);
    let path_2 = hex_path_to_path_digits("128");
    let value_2 = rlp_encode_short_slice(&[0x11]);

    trie.insert(
        Path::new(&path_1),
        &value_1,
        &mut preimages_oracle,
        &mut interner,
        &mut hasher,
    )
    .unwrap();
    trie.insert(
        Path::new(&path_2),
        &value_2,
        &mut preimages_oracle,
        &mut interner,
        &mut hasher,
    )
    .unwrap();

    let _ = trie.recompute(&mut interner, &mut hasher);
    let initial_root = trie.root(&mut hasher);

    let value_1_tmp = rlp_encode_short_slice(&[0xff, 0xff]);
    let value_2_tmp = rlp_encode_short_slice(&[0x11, 0x11]);
    trie.update(Path::new(&path_1), &value_1_tmp, &mut interner, &mut hasher)
        .unwrap();
    trie.update(Path::new(&path_2), &value_2_tmp, &mut interner, &mut hasher)
        .unwrap();
    let _ = trie.recompute(&mut interner, &mut hasher);

    trie.update(Path::new(&path_1), &value_1, &mut interner, &mut hasher)
        .unwrap();
    trie.update(Path::new(&path_2), &value_2, &mut interner, &mut hasher)
        .unwrap();
    let _ = trie.recompute(&mut interner, &mut hasher);
    let final_root = trie.root(&mut hasher);
    assert_eq!(initial_root, final_root);

    let v_1 = trie
        .get(
            Path::new(&path_1),
            &mut preimages_oracle,
            &mut interner,
            &mut hasher,
        )
        .unwrap();
    assert_eq!(v_1, rlp_parse_short_bytes(&value_1).unwrap());
    let v_2 = trie
        .get(
            Path::new(&path_2),
            &mut preimages_oracle,
            &mut interner,
            &mut hasher,
        )
        .unwrap();
    assert_eq!(v_2, rlp_parse_short_bytes(&value_2).unwrap());
}
