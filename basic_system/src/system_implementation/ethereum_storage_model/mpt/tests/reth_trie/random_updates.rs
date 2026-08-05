#![allow(missing_docs, unreachable_pub)]
use std::collections::BTreeMap;

use crate::system_implementation::ethereum_storage_model::mpt::tests::*;
use crate::system_implementation::ethereum_storage_model::mpt::Path;
use alloy_primitives::{B256, U256};
use proptest::{prelude::*, strategy::ValueTree, test_runner::TestRunner};
use reth_trie_common::{HashBuilder, Nibbles};
use std::alloc::Global;

#[test]
fn test_ordered() {
    test_versus_randmomized_reth_trie_ordered(10);
    test_versus_randmomized_reth_trie_ordered(100);
    test_versus_randmomized_reth_trie_ordered(1000);
    test_versus_randmomized_reth_trie_ordered(10000);
}

fn test_versus_randmomized_reth_trie_ordered(size: usize) {
    let (initial_state, final_state) = generate_test_data(size);
    // NOTE: here we insert 0s, that is not really a case in real Ethereum case
    let initial_root = {
        let mut hb = HashBuilder::default();
        for (key, value) in initial_state.iter() {
            hb.add_leaf(Nibbles::unpack(key), &alloy_rlp::encode_fixed_size(value));
        }
        hb.root()
    };
    let final_root = {
        let mut hb = HashBuilder::default();
        for (key, value) in final_state.iter() {
            hb.add_leaf(Nibbles::unpack(key), &alloy_rlp::encode_fixed_size(value));
        }
        hb.root()
    };

    assert_ne!(initial_root, final_root);
    let initial_our_root = {
        let mut interner = BoxInterner::with_capacity_in(1 << 26, Global);
        use crypto::MiniDigest;
        let mut hasher = crypto::sha3::Keccak256::new();
        let mut trie =
            EthereumMPT::new_in(EMPTY_ROOT_HASH.as_u8_array(), &mut interner, Global).unwrap();
        let mut preimages_oracle = BTreeMap::new();

        for (key, value) in initial_state.iter() {
            let path_digits = byte_path_to_path_digits(key);
            let path = Path::new(&path_digits);
            let encoded_value = alloy_rlp::encode_fixed_size(value);
            let pre_encoded_value = rlp_encode_short_slice(&encoded_value);
            trie.insert(
                path,
                &pre_encoded_value,
                &mut preimages_oracle,
                &mut interner,
                &mut hasher,
            )
            .unwrap();

            trie.ensure_linked();
        }
        trie.recompute(&mut interner, &mut hasher).unwrap();

        trie.root(&mut hasher)
    };
    assert_eq!(initial_root, initial_our_root);

    let final_our_root = {
        let mut interner = BoxInterner::with_capacity_in(1 << 26, Global);
        use crypto::MiniDigest;
        let mut hasher = crypto::sha3::Keccak256::new();
        let mut trie =
            EthereumMPT::new_in(EMPTY_ROOT_HASH.as_u8_array(), &mut interner, Global).unwrap();
        let mut preimages_oracle = BTreeMap::new();

        for (key, value) in final_state.iter() {
            let path_digits = byte_path_to_path_digits(key);
            let path = Path::new(&path_digits);
            let encoded_value = alloy_rlp::encode_fixed_size(value);
            let pre_encoded_value = rlp_encode_short_slice(&encoded_value);
            trie.insert(
                path,
                &pre_encoded_value,
                &mut preimages_oracle,
                &mut interner,
                &mut hasher,
            )
            .unwrap();

            trie.ensure_linked();
        }
        trie.recompute(&mut interner, &mut hasher).unwrap();

        trie.root(&mut hasher)
    };
    assert_eq!(final_root, final_our_root);

    // and now - complex updates
    let mut interner = BoxInterner::with_capacity_in(1 << 26, Global);
    use crypto::MiniDigest;
    let mut hasher = crypto::sha3::Keccak256::new();
    let mut trie =
        EthereumMPT::new_in(EMPTY_ROOT_HASH.as_u8_array(), &mut interner, Global).unwrap();
    let mut preimages_oracle = BTreeMap::new();

    for (key, value) in initial_state.iter() {
        let path_digits = byte_path_to_path_digits(key);
        let path = Path::new(&path_digits);
        let encoded_value = alloy_rlp::encode_fixed_size(value);
        let pre_encoded_value = rlp_encode_short_slice(&encoded_value);
        if value.is_zero() == false {
            trie.insert(
                path,
                &pre_encoded_value,
                &mut preimages_oracle,
                &mut interner,
                &mut hasher,
            )
            .unwrap();

            trie.ensure_linked();
        }
    }
    trie.recompute(&mut interner, &mut hasher).unwrap();

    let mut hb = HashBuilder::default();

    // update/insert/delete
    for ((k_i, v_i), (k_f, v_f)) in initial_state.iter().zip(final_state.iter()) {
        assert_eq!(k_i, k_f);

        // rebuild reference just from scratch
        if v_f.is_zero() == false {
            hb.add_leaf(Nibbles::unpack(k_i), &alloy_rlp::encode_fixed_size(v_f));
        }

        let path_digits = byte_path_to_path_digits(k_i);
        let path = Path::new(&path_digits);
        let encoded_value = alloy_rlp::encode_fixed_size(v_f);
        let pre_encoded_value = rlp_encode_short_slice(&encoded_value);

        if v_i.is_zero() == false {
            // we inserted before, so let's update or delete
            if v_f.is_zero() {
                // println!("Delete {}", hex::encode(k_i));
                trie.delete(path, &mut preimages_oracle, &mut interner, &mut hasher)
                    .unwrap();
            } else if v_i != v_f {
                // println!("Update {}", hex::encode(k_i));
                trie.update(path, &pre_encoded_value, &mut interner, &mut hasher)
                    .unwrap();
            }
        } else {
            if v_f.is_zero() == false {
                // println!("Insert {}", hex::encode(k_i));
                trie.insert(
                    path,
                    &pre_encoded_value,
                    &mut preimages_oracle,
                    &mut interner,
                    &mut hasher,
                )
                .unwrap();
            }
        }

        trie.ensure_linked();
    }

    let expected_root = hb.root();
    trie.recompute(&mut interner, &mut hasher).unwrap();
    let our_root = trie.root(&mut hasher);

    assert_eq!(our_root, expected_root);
}

fn generate_test_data(size: usize) -> (BTreeMap<B256, U256>, BTreeMap<B256, U256>) {
    let mut initial_state = Vec::with_capacity(size);
    let mut final_state = Vec::with_capacity(size);
    let mut runner = TestRunner::deterministic();
    for _ in 0..size {
        let is_read = any::<bool>().new_tree(&mut runner).unwrap().current();
        if is_read {
            let key = any::<B256>().new_tree(&mut runner).unwrap().current();
            let value = any::<U256>().new_tree(&mut runner).unwrap().current();
            initial_state.push((key, value));
            final_state.push((key, value));
        } else {
            let is_insert = any::<bool>().new_tree(&mut runner).unwrap().current();
            let is_delete = any::<bool>().new_tree(&mut runner).unwrap().current();
            if is_insert {
                let key = any::<B256>().new_tree(&mut runner).unwrap().current();
                let final_value = any::<U256>().new_tree(&mut runner).unwrap().current();
                initial_state.push((key, U256::ZERO));
                final_state.push((key, final_value));
            } else if is_delete {
                let key = any::<B256>().new_tree(&mut runner).unwrap().current();
                let initial_value = any::<U256>().new_tree(&mut runner).unwrap().current();
                initial_state.push((key, initial_value));
                final_state.push((key, U256::ZERO));
            } else {
                let key = any::<B256>().new_tree(&mut runner).unwrap().current();
                let initial_value = any::<U256>().new_tree(&mut runner).unwrap().current();
                let final_value = any::<U256>().new_tree(&mut runner).unwrap().current();
                initial_state.push((key, initial_value));
                final_state.push((key, final_value));
            }
        }
    }
    let initial_storage = BTreeMap::from_iter(initial_state.into_iter());

    let final_storage = BTreeMap::from_iter(final_state.into_iter());

    (initial_storage, final_storage)
}
