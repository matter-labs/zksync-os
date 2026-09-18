use super::*;
use crate::system_implementation::ethereum_storage_model::mpt::stack_trie::{StackMPT, TrieKey};
use crate::system_implementation::ethereum_storage_model::mpt::tests::reth_trie::random_updates::generate_test_data;
use alloy_primitives::{B256, U256};
use reth_trie_common::{HashBuilder, Nibbles};

// Reference trie builder that keeps every node, so the stack trie can unfold them

fn rlp_list(items: &[Vec<u8>]) -> Vec<u8> {
    let payload: Vec<u8> = items.concat();
    let mut out = Vec::new();
    if payload.len() <= 55 {
        out.push(0xc0 + payload.len() as u8);
    } else if payload.len() < 256 {
        out.extend_from_slice(&[0xf8, payload.len() as u8]);
    } else {
        out.extend_from_slice(&[0xf9, (payload.len() >> 8) as u8, payload.len() as u8]);
    }
    out.extend_from_slice(&payload);
    out
}

fn compact(nibbles: &[u8], is_leaf: bool) -> Vec<u8> {
    let mut out = Vec::new();
    let odd = nibbles.len() % 2 == 1;
    let mut first = if is_leaf { 0x20 } else { 0x00 };
    let rest = if odd {
        first |= 0x10 | nibbles[0];
        &nibbles[1..]
    } else {
        nibbles
    };
    out.push(first);
    for pair in rest.chunks(2) {
        out.push((pair[0] << 4) | pair[1]);
    }
    out
}

fn node_ref(encoding: Vec<u8>, store: &mut BTreeMap<Bytes32, Vec<u8>>) -> Vec<u8> {
    if encoding.len() >= 32 {
        let hash = *Keccak256::digest(&encoding);
        store.insert(Bytes32::from_array(hash), encoding);
        let mut out = vec![0x80 + 32];
        out.extend_from_slice(&hash);
        out
    } else {
        encoding
    }
}

/// `entries` are (64 nibbles, full RLP item of the value), sorted by nibbles
fn build_node(
    entries: &[(Vec<u8>, Vec<u8>)],
    depth: usize,
    store: &mut BTreeMap<Bytes32, Vec<u8>>,
) -> Vec<u8> {
    assert!(entries.is_empty() == false);
    if entries.len() == 1 {
        let (nibbles, value) = &entries[0];
        return rlp_list(&[
            rlp_encode_short_slice(&compact(&nibbles[depth..], true)),
            value.clone(),
        ]);
    }
    let first = &entries[0].0;
    let mut common = 0;
    while depth + common < 64
        && entries
            .iter()
            .all(|(n, _)| n[depth + common] == first[depth + common])
    {
        common += 1;
    }
    if common > 0 {
        let child = build_node(entries, depth + common, store);
        let child_ref = node_ref(child, store);
        return rlp_list(&[
            rlp_encode_short_slice(&compact(&first[depth..depth + common], false)),
            child_ref,
        ]);
    }
    let mut items = Vec::with_capacity(17);
    let mut start = 0;
    for nibble in 0..16u8 {
        let end = start
            + entries[start..]
                .iter()
                .take_while(|(n, _)| n[depth] == nibble)
                .count();
        if end == start {
            items.push(vec![0x80]);
        } else {
            let child = build_node(&entries[start..end], depth + 1, store);
            items.push(node_ref(child, store));
        }
        start = end;
    }
    assert_eq!(start, entries.len());
    items.push(vec![0x80]);
    rlp_list(&items)
}

fn build_reference_trie(
    state: &BTreeMap<B256, U256>,
    store: &mut BTreeMap<Bytes32, Vec<u8>>,
) -> [u8; 32] {
    let entries: Vec<(Vec<u8>, Vec<u8>)> = state
        .iter()
        .filter(|(_, v)| v.is_zero() == false)
        .map(|(k, v)| {
            (
                byte_path_to_path_digits(k),
                rlp_encode_short_slice(&alloy_rlp::encode_fixed_size(v)),
            )
        })
        .collect();
    if entries.is_empty() {
        return EMPTY_ROOT_HASH.as_u8_array();
    }
    let root = build_node(&entries, 0, store);
    let hash = *Keccak256::digest(&root);
    store.insert(Bytes32::from_array(hash), root);
    hash
}

fn reth_root(state: &BTreeMap<B256, U256>) -> [u8; 32] {
    let mut hb = HashBuilder::default();
    for (key, value) in state.iter() {
        if value.is_zero() == false {
            hb.add_leaf(Nibbles::unpack(key), &alloy_rlp::encode_fixed_size(value));
        }
    }
    hb.root().0
}

fn apply_with_stack_trie(
    initial_state: &BTreeMap<B256, U256>,
    final_state: &BTreeMap<B256, U256>,
) -> [u8; 32] {
    let mut store = BTreeMap::new();
    let initial_root = build_reference_trie(initial_state, &mut store);
    assert_eq!(initial_root, reth_root(initial_state));

    let mut interner = BoxInterner::with_capacity_in(1 << 26, Global);
    let mut hasher = Keccak256::new();
    let mut trie = StackMPT::new_in(Global);
    trie.set_root(&initial_root, &mut interner).unwrap();

    for (key, v_i) in initial_state.iter() {
        let v_f = final_state[key];
        let trie_key = TrieKey::from_hash(&key.0);
        let existing = trie
            .seek(trie_key, &mut store, &mut interner, &mut hasher)
            .unwrap();
        if v_i.is_zero() {
            assert!(existing.is_none(), "key {key:?} must be absent");
        } else {
            assert_eq!(
                existing.unwrap(),
                alloy_rlp::encode_fixed_size(v_i).as_slice(),
                "wrong initial value for {key:?}"
            );
        }
        if v_i == &v_f {
            continue;
        }
        if v_f.is_zero() {
            trie.delete().unwrap();
        } else {
            let item = rlp_encode_short_slice(&alloy_rlp::encode_fixed_size(&v_f));
            trie.set(&item, &mut interner, &mut hasher).unwrap();
        }
    }

    trie.finalize(&mut store, &mut interner, &mut hasher)
        .unwrap()
}

#[test]
fn stack_trie_random_updates() {
    for size in [1usize, 2, 3, 10, 100, 1000, 10000] {
        let (initial_state, final_state) = generate_test_data(size);
        let our_root = apply_with_stack_trie(&initial_state, &final_state);
        assert_eq!(our_root, reth_root(&final_state), "size {size}");
    }
}

fn key_from_hex(prefix: &str) -> B256 {
    let mut s = String::from(prefix);
    while s.len() < 64 {
        s.push('0');
    }
    B256::from_slice(&hex::decode(s).unwrap())
}

fn run_scenario(initial: &[(&str, u64)], finals: &[(&str, u64)]) {
    println!("scenario: initial {initial:?}, final {finals:?}");
    let mut initial_state = BTreeMap::new();
    let mut final_state = BTreeMap::new();
    for (k, v) in initial.iter() {
        initial_state.insert(key_from_hex(k), U256::from(*v));
        final_state.insert(key_from_hex(k), U256::from(*v));
    }
    for (k, v) in finals.iter() {
        final_state.insert(key_from_hex(k), U256::from(*v));
        initial_state.entry(key_from_hex(k)).or_insert(U256::ZERO);
    }
    let our_root = apply_with_stack_trie(&initial_state, &final_state);
    assert_eq!(
        our_root,
        reth_root(&final_state),
        "scenario: initial {initial:?}, final {finals:?}"
    );
}

#[test]
fn stack_trie_deletion_collapses() {
    // branch under extension, delete one of two leaves: branch collapses into the extension as a leaf
    run_scenario(&[("abc1", 1), ("abc2", 2), ("f", 3)], &[("abc1", 0)]);
    // delete both leaves of a branch under an extension: whole subtree vanishes
    run_scenario(
        &[("abc1", 1), ("abc2", 2), ("f", 3)],
        &[("abc1", 0), ("abc2", 0)],
    );
    // delete both and insert something else under the same extension
    run_scenario(
        &[("abc1", 1), ("abc2", 2), ("f", 3)],
        &[("abc1", 0), ("abc2", 0), ("abd", 7)],
    );
    // remaining child is an extension: ext + ext merge
    run_scenario(
        &[
            ("abc1", 1),
            ("abc2", 2),
            ("abd11", 3),
            ("abd12", 4),
            ("f", 5),
        ],
        &[("abc1", 0), ("abc2", 0)],
    );
    // remaining child is a branch
    run_scenario(&[("ab1", 1), ("ab2", 2), ("ac", 3), ("f", 5)], &[("ac", 0)]);
    // remaining child is the untouched left sibling
    run_scenario(&[("ab1", 1), ("ac", 3), ("f", 5)], &[("ac", 0)]);
    // root branch collapses
    run_scenario(&[("1", 1), ("2", 2)], &[("2", 0)]);
    // root becomes empty
    run_scenario(&[("1", 1), ("2", 2)], &[("1", 0), ("2", 0)]);
    // empty trie gets a leaf
    run_scenario(&[], &[("1", 1)]);
    // single leaf root gets split
    run_scenario(&[("1", 1)], &[("2", 1)]);
    // split inside an extension
    run_scenario(&[("abc1", 1), ("abc2", 2), ("f", 3)], &[("ab9", 4)]);
    // split at the first nibble of an extension
    run_scenario(&[("abc1", 1), ("abc2", 2), ("f", 3)], &[("b", 4)]);
    // the branch a deletion happened in receives an insert afterwards
    run_scenario(
        &[("abc1", 1), ("abc2", 2), ("f", 3)],
        &[("abc1", 0), ("abc3", 9)],
    );
    // update in the remaining subtree after a delete
    run_scenario(
        &[("abc1", 1), ("abc21", 2), ("abc22", 3), ("f", 4)],
        &[("abc1", 0), ("abc22", 5)],
    );
}

#[test]
fn stack_trie_from_execution_witness() {
    let data = read_execution_witness();
    let mut interner = BoxInterner::with_capacity_in(1 << 26, Global);
    let mut hasher = Keccak256::new();
    let (prestate, diffs) = decode_prestate_and_diffs();

    let account_proofs_at_block_end = std::fs::File::open("./account_proofs.json").unwrap();
    let account_proofs_at_block_end: HashMap<B160, AccountProof> =
        serde_json::from_reader(account_proofs_at_block_end).unwrap();

    let ParsedWitness {
        mut oracle,
        addresses_to_trie_pos,
        initial_root,
        ..
    } = data;

    let (initial_state, final_state) = compute_initial_and_final_states(prestate, diffs);

    // initial storage roots of the accounts from the accounts trie
    let mut initial_storage_roots = BTreeMap::new();
    let mut accounts_trie = StackMPT::new_in(Global);
    accounts_trie
        .set_root(&initial_root.clone().try_into().unwrap(), &mut interner)
        .unwrap();
    let mut sorted_addresses: Vec<_> = initial_state
        .0
        .keys()
        .map(|address_key| {
            let address = address_key.0.to_be_bytes_vec();
            (
                TrieKey::from_hash(addresses_to_trie_pos[&address].as_u8_array_ref()),
                *address_key,
            )
        })
        .collect();
    sorted_addresses.sort();
    for (key, address) in sorted_addresses.iter() {
        let existing = accounts_trie
            .seek(*key, &mut oracle, &mut interner, &mut hasher)
            .unwrap();
        if let Some(account_data) = existing {
            let data = decode_address_data(account_data);
            initial_storage_roots.insert(*address, data[2].data().to_vec());
        }
    }
    let unchanged_root = accounts_trie
        .finalize(&mut oracle, &mut interner, &mut hasher)
        .unwrap();
    assert_eq!(unchanged_root.as_slice(), initial_root.as_slice());

    let mut checked = 0;
    for (address, final_account) in final_state.0.iter() {
        let initial_storage = initial_state
            .0
            .get(address)
            .cloned()
            .unwrap_or_default()
            .storage
            .unwrap_or_default();
        let final_storage = final_account.storage.clone().unwrap_or_default();
        let Some(root) = initial_storage_roots.get(address) else {
            // initially empty account, nothing to verify against
            continue;
        };
        let root: [u8; 32] = root.as_slice().try_into().unwrap();

        let mut updates: Vec<(TrieKey, U256, U256)> = final_storage
            .iter()
            .map(|(k, v_f)| {
                let hash = *Keccak256::digest(k.to_be_bytes::<32>());
                let v_i = initial_storage
                    .get(k)
                    .map(|v| v.into_inner())
                    .unwrap_or(U256::ZERO);
                (TrieKey::from_hash(&hash), v_i, v_f.into_inner())
            })
            .collect();
        updates.sort_by(|a, b| a.0.cmp(&b.0));
        let any_mutation = updates.iter().any(|(_, i, f)| i != f);

        let mut trie = StackMPT::new_in(Global);
        interner.reset();
        trie.set_root(&root, &mut interner).unwrap();
        for (key, v_i, v_f) in updates.iter() {
            let existing = trie
                .seek(*key, &mut oracle, &mut interner, &mut hasher)
                .unwrap();
            if v_i.is_zero() {
                assert!(existing.is_none());
            } else {
                assert_eq!(
                    rlp_parse_short_bytes(existing.unwrap()).unwrap(),
                    v_i.to_be_bytes_trimmed_vec().as_slice(),
                    "address 0x{}",
                    hex::encode(address.0.to_be_bytes_vec())
                );
            }
            if v_i == v_f {
                continue;
            }
            if v_f.is_zero() {
                trie.delete().unwrap();
            } else {
                let item =
                    rlp_encode_short_slice(&rlp_encode_short_slice(&v_f.to_be_bytes_trimmed_vec()));
                trie.set(&item, &mut interner, &mut hasher).unwrap();
            }
        }
        let new_root = trie
            .finalize(&mut oracle, &mut interner, &mut hasher)
            .unwrap();
        if any_mutation {
            assert_ne!(new_root, root);
        } else {
            assert_eq!(new_root, root);
        }
        let account_proof = &account_proofs_at_block_end[&address.0];
        assert_eq!(
            new_root,
            account_proof.storage_hash.0,
            "account storage root diverged for address 0x{:040x}",
            address.0.into_inner()
        );
        checked += 1;
    }
    assert!(checked > 0);
}
