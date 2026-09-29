use super::*;

#[test]
fn test_create() {
    let tree = TestingTree::<false>::new_in(Global);
    let mut hasher = Blake2sStorageHasher::new();

    // Test reference hash values.
    assert_eq!(
        tree.empty_hashes[TESTING_TREE_HEIGHT],
        hex_bytes("0xe3cdc93b3c2beb30f6a7c7cc45a32da012df9ae1be880e2c074885cb3f4e1e53")
    );
    assert_eq!(
        [
            *tree
                .hashes
                .0
                .get(&(TESTING_TREE_HEIGHT as u32))
                .unwrap()
                .get(&0)
                .unwrap(),
            *tree
                .hashes
                .0
                .get(&(TESTING_TREE_HEIGHT as u32))
                .unwrap()
                .get(&1)
                .unwrap(),
        ],
        [
            hex_bytes("0x9903897e51baa96a5ea51b4c194d3e0c6bcf20947cce9fd646dfb4bf754c8d28"),
            hex_bytes("0xb35299e7564e05e335094c02064bccf83d58745b417874b1fee3f523ec2007a9"),
        ]
    );
    assert_eq!(
        tree.empty_hashes[TESTING_TREE_HEIGHT - 1],
        hex_bytes("0xc45bfaf4bb5d0fee27d3178b8475155a07a1fa8ada9a15133a9016f7d0435f0f")
    );
    assert_eq!(
        tree.empty_hashes[1],
        hex_bytes("0xb720fe53e6bd4e997d967b8649e10036802a4fd3aca6d7dcc43ed9671f41cb31")
    );
    assert_eq!(
        *tree.root(),
        hex_bytes("0x90a83ead2ba2194fbbb0f7cd2a017e36cfb4891513546d943a7282c2844d4b6b")
    );

    let start_guard_proof = tree.get(&Bytes32::ZERO);
    let ReadValueWithProof::Existing { proof } = start_guard_proof else {
        panic!()
    };
    assert!(tree.verify_proof(&mut hasher, &proof.existing));
    assert!(proof.existing.leaf.key == Bytes32::ZERO);
    let end_guard_proof = tree.get(&Bytes32::MAX);
    let ReadValueWithProof::Existing { proof } = end_guard_proof else {
        panic!()
    };
    assert!(tree.verify_proof(&mut hasher, &proof.existing));
    assert!(proof.existing.leaf.key == Bytes32::MAX);

    // Check that mutating a Merkle path in the proof invalidates it.
    let mut mutated_proof = proof.existing;
    *mutated_proof.path.last_mut().unwrap() = Bytes32::zero();
    assert!(!tree.verify_proof(&mut hasher, &mutated_proof));
}

#[test]
fn test_new_in_with_leaves_links_next_positions() {
    let mut leaves = Vec::new_in(Global);
    leaves.push((Bytes32::from_byte_fill(0x01), Bytes32::from_byte_fill(0x10)));
    leaves.push((Bytes32::from_byte_fill(0x02), Bytes32::from_byte_fill(0x20)));

    let tree = TestingTree::<false>::new_in_with_leaves(Global, leaves);

    let start = tree.leaves.get(&0).expect("start guard missing");
    let first = tree.leaves.get(&2).expect("first leaf missing");
    let second = tree.leaves.get(&3).expect("second leaf missing");
    let end = tree.leaves.get(&1).expect("end guard missing");

    assert_eq!(start.next, 2);
    assert_eq!(first.next, 3);
    assert_eq!(second.next, 1);
    assert_eq!(end.next, 1);
}

#[test]
fn test_insert() {
    let mut tree = TestingTree::<false>::new_in(Global);
    let mut hasher = Blake2sStorageHasher::new();

    let initial_root = *tree.root();
    let next_available = tree.next_free_slot;
    let key_to_insert = Bytes32::from_byte_fill(0x01);
    let value_to_insert = Bytes32::from_byte_fill(0x10);
    let new_leaf_proof = tree.insert(&key_to_insert, &value_to_insert);
    let new_root = *tree.root();

    assert_eq!(
        new_root,
        hex_bytes("0x08da20879eebed16fbd14e50b427bb97c8737aa860e6519877757e238df83a15")
    );

    let WriteValueWithProof::New {
        proof:
            NewWriteProof {
                previous,
                next,
                new_insert,
            },
    } = new_leaf_proof
    else {
        panic!()
    };
    assert!(new_insert.leaf == FlatStorageLeaf::empty());
    assert!(verify_proof_for_root(&mut hasher, &previous, &initial_root));
    let insert_pos = new_insert.index;
    assert!(previous.index < next_available);
    assert!(previous.leaf.key < key_to_insert);
    let mut previous = previous;
    previous.leaf.next = insert_pos;
    let mut new_intermediate = recompute_root_from_proof(&mut hasher, &previous);
    assert!(verify_proof_for_root(&mut hasher, &next, &new_intermediate));
    assert!(next.index < next_available);
    assert!(next.leaf.key > key_to_insert);
    new_intermediate = recompute_root_from_proof(&mut hasher, &next);
    assert!(verify_proof_for_root(
        &mut hasher,
        &new_insert,
        &new_intermediate
    ));
    assert!(new_insert.index == next_available);

    let mut new_insert = new_insert;
    new_insert.leaf.key = key_to_insert;
    new_insert.leaf.value = value_to_insert;
    new_insert.leaf.next = next.index;
    new_intermediate = recompute_root_from_proof(&mut hasher, &new_insert);
    assert_eq!(new_intermediate, new_root);
}

#[test]
fn test_insert_many_and_update() {
    let mut tree = TestingTree::<false>::new_in(Global);
    let mut hasher = Blake2sStorageHasher::new();

    let next_available = tree.next_free_slot;
    let key_to_insert_0 = Bytes32::from_byte_fill(0x01);
    let value_to_insert_0 = Bytes32::from_byte_fill(0x10);
    let _ = tree.insert(&key_to_insert_0, &value_to_insert_0);
    let key_to_insert_1 = Bytes32::from_byte_fill(0x02);
    let value_to_insert_1 = Bytes32::from_byte_fill(0x20);
    let _ = tree.insert(&key_to_insert_1, &value_to_insert_1);

    let initial_root = *tree.root();
    let value_to_insert = Bytes32::from_byte_fill(0x33);
    let existing_leaf_proof = tree.insert(&key_to_insert_0, &value_to_insert);
    let new_root = *tree.root();

    assert_eq!(
        initial_root,
        hex_bytes("0xf227612db17b44a5c9a2ebd0e4ff2dbe91aa05f3198d09f0bcfd6ef16c1d28c8")
    );
    assert_eq!(
        new_root,
        hex_bytes("0x81a600569c2cda27c7ae4773255acc70ac318a49404fa1035a7734a3aaa82589")
    );

    let WriteValueWithProof::Existing {
        proof: ExistingWriteProof { existing },
    } = existing_leaf_proof
    else {
        panic!()
    };
    assert!(existing.leaf.key == key_to_insert_0);
    assert!(existing.leaf.value == value_to_insert_0);
    assert!(existing.index == next_available);
    assert!(verify_proof_for_root(&mut hasher, &existing, &initial_root));
    let mut existing = existing;
    existing.leaf.value = value_to_insert;
    assert!(verify_proof_for_root(&mut hasher, &existing, &new_root));
}

#[test]
fn test_key_ordering() {
    let mut tree = TestingTree::<false>::new_in(Global);
    let key_to_insert_0 = to_be_bytes(0xc0ffeefe);
    let value_to_insert_0 = Bytes32::from_byte_fill(0x10);
    let key_to_insert_1 = to_be_bytes(0xdeadbeef);
    let value_to_insert_1 = Bytes32::from_byte_fill(0x20);
    assert!(key_to_insert_0 < key_to_insert_1);

    let _ = tree.insert(&key_to_insert_0, &value_to_insert_0);
    let _ = tree.insert(&key_to_insert_1, &value_to_insert_1);
    assert_eq!(tree.leaves.get(&0).unwrap().next, 2);
    assert_eq!(tree.leaves.get(&2).unwrap().next, 3);
    assert_eq!(tree.leaves.get(&3).unwrap().next, 1);
    assert_eq!(
        *tree.root(),
        hex_bytes("0xc90465eddad7cc858a2fbf61013d7051c143887a887e5a7a19344ac32151b207")
    );
}
