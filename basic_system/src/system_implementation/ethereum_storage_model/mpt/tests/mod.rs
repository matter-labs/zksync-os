mod serialization;

use crypto::MiniDigest;
use num_bigint::BigUint;
use num_traits::Zero;
use std::collections::BTreeSet;
use std::{alloc::Global, collections::BTreeMap};

use crate::system_implementation::ethereum_storage_model::mpt::trie::{
    rlp_parse_short_bytes, EthereumMPT,
};
use crate::system_implementation::ethereum_storage_model::mpt::BoxInterner;

use self::serialization::*;

pub(crate) fn read_test_vectors() -> BTreeMap<Vec<u8>, (AccountProof, AccountProof)> {
    let paths = std::fs::read_dir("./proofs").unwrap();

    // let mut

    let mut mapping: BTreeMap<Vec<u8>, BTreeMap<u64, AccountProof>> = BTreeMap::new();

    for path in paths {
        if let Ok(path) = path {
            let pp = path.path();
            let p = pp.strip_prefix("./proofs/").unwrap();
            let full_name = p
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_owned()
                .into_owned();
            let t = full_name.strip_suffix(".json").unwrap();
            let mut it = t.split("_");
            let key = it.next().unwrap();
            let key = hex::decode(key).unwrap();
            let block_number = it.next().unwrap();
            let block_number = u64::from_str_radix(block_number, 16).unwrap();

            let content = std::fs::File::open(path.path()).unwrap();
            let data: TestJsonResponse = serde_json::from_reader(content).unwrap();
            let entry: &mut BTreeMap<u64, AccountProof> = mapping.entry(key).or_default();
            entry.insert(block_number, data.result);
        }
    }

    let mut result = BTreeMap::new();
    for (key, values) in mapping.into_iter() {
        assert_eq!(values.len(), 2);
        let mut it = values.into_iter();
        let (_, a) = it.next().unwrap();
        let (_, b) = it.next().unwrap();
        result.insert(key, (a, b));
    }

    result
}

fn encode_integer_as_rlp_slice(value: &BigUint) -> Vec<u8> {
    assert!(value.is_zero() == false);
    let be_encoding = value.to_bytes_be();
    assert!(be_encoding.len() <= 32);
    if be_encoding.len() == 1 && be_encoding[0] < 0x80 {
        return be_encoding
    } else {
        let mut result = vec![0x80 + (be_encoding.len()) as u8];
        result.extend(be_encoding);

        result
    }
}

fn rlp_encode_short_slice(slice: &[u8]) -> Vec<u8> {
    if slice.len() == 1 {
        return slice.to_vec();
    }
    assert!(slice.len() <= 55);
    let mut result = vec![0x80 + (slice.len() as u8)];
    result.extend_from_slice(slice);

    result
}

fn encode_integer_as_raw_terminal_value(value: &BigUint) -> Vec<u8> {
    rlp_encode_short_slice(&encode_integer_as_rlp_slice(value))
}

#[test]
fn parse_pre_states() {
    let map = read_test_vectors();
    let mut interner = BoxInterner::with_capacity_in(1 << 20, Global);
    for (address, (initial_state, final_state)) in map.iter() {
        if initial_state.storage_proof.is_empty() {
            continue;
        }
        // dbg!(hex::encode(address));

        if hex::encode(address) != "14fee680690900ba0cccfc76ad70fd1b95d10e16" {
            continue;
        }

        let mut trie = EthereumMPT::new_in(Global);
        let mut initial_values = BTreeMap::new();
        for el in initial_state.storage_proof.iter() {
            let key_like = &el.key;
            assert_eq!(key_like.trie_pos_digits.len(), 64);
            // println!(
            //     "Checking proofs for key {}, expected value 0x{:x}",
            //     hex::encode(&key_like.key),
            //     &el.value
            // );
            // println!(
            //     "Trie position is {}",
            //     std::str::from_utf8(&key_like.trie_pos_digits_string).unwrap()
            // );
            if el.value.is_zero() == false {
                initial_values.insert(el.key.clone(), el.value.clone());
            }

            let proof = el.proof.iter().map(|el| &el[..]);
            let val = trie
                .insert_proof(&key_like.trie_pos_digits, proof, &mut interner)
                .unwrap();
            if val.is_empty() {
                assert!(el.value.is_zero());
            } else {
                let be_integer_encoding = rlp_parse_short_bytes(val).unwrap();
                let returned = BigUint::from_bytes_be(be_integer_encoding);
                assert_eq!(
                    &returned, &el.value,
                    "parsed 0x{:x}, expected 0x{:x}",
                    &returned, &el.value
                );
            }
        }
        let mut updates = BTreeMap::new();
        let mut deletes = BTreeSet::new();
        let mut inserts = BTreeMap::new();
        for el in final_state.storage_proof.iter() {
            let key_like = &el.key;
            if let Some(initial) = initial_values.get(&el.key) {
                if el.value.is_zero() {
                    // println!("Will delete value for key {}", hex::encode(&key_like.key),);
                    // println!(
                    //     "Trie position is {}",
                    //     std::str::from_utf8(&key_like.trie_pos_digits_string).unwrap()
                    // );
                    deletes.insert(el.key.clone());
                } else if initial != &el.value {
                    // println!(
                    //     "Will update new value for key {}, new value 0x{:x}",
                    //     hex::encode(&key_like.key),
                    //     &el.value
                    // );
                    // println!(
                    //     "Trie position is {}",
                    //     std::str::from_utf8(&key_like.trie_pos_digits_string).unwrap()
                    // );
                    updates.insert(el.key.clone(), el.value.clone());
                }
            } else {
                if el.value.is_zero() == false {
                    // println!(
                    //     "Will insert new value for key {}, inserted value 0x{:x}",
                    //     hex::encode(&key_like.key),
                    //     &el.value
                    // );
                    // println!(
                    //     "Trie position is {}",
                    //     std::str::from_utf8(&key_like.trie_pos_digits_string).unwrap()
                    // );
                    inserts.insert(el.key.clone(), el.value.clone());
                }
            }
        }
        let mut new_root = &[][..];

        // if updates.is_empty() == false && deletes.is_empty() && inserts.is_empty() {

        // } else {
        //     continue;
        // }

        if deletes.is_empty() {
            continue;
        }

        if updates.is_empty() == false || deletes.is_empty() == false && inserts.is_empty() {

        } else {
            continue;
        }

        dbg!(hex::encode(address));
        let mut hasher = crypto::sha3::Keccak256::new();

        // perform updates
        if updates.is_empty() && inserts.is_empty() && deletes.is_empty() {
            // nothing
        } else {
            for (k, v) in updates.iter() {
                println!(
                    "Will update new value for key {}, new value 0x{:x}",
                    hex::encode(&k.key),
                    &v
                );
                println!(
                    "Trie position is {}",
                    std::str::from_utf8(&k.trie_pos_digits_string).unwrap()
                );
                let new_value = encode_integer_as_raw_terminal_value(&v);
                new_root = trie.update(&k.trie_pos_digits, &new_value, &mut interner, &mut hasher).unwrap();
            }
            for k in deletes.iter() {
                println!("Will delete value for key {}", hex::encode(&k.key),);
                println!(
                    "Trie position is {}",
                    std::str::from_utf8(&k.trie_pos_digits_string).unwrap()
                );
                new_root = trie.delete(&k.trie_pos_digits, &mut interner, &mut hasher).unwrap();
            }
        }

        // recheck the proofs
        let mut final_trie = EthereumMPT::new_in(Global);
        for el in final_state.storage_proof.iter() {
            let key_like = &el.key;
            assert_eq!(key_like.trie_pos_digits.len(), 64);
            // println!(
            //     "Checking proofs for key {}, expected value 0x{:x}",
            //     hex::encode(&key_like.key),
            //     &el.value
            // );
            // println!(
            //     "Trie position is {}",
            //     std::str::from_utf8(&key_like.trie_pos_digits_string).unwrap()
            // );
            if el.value.is_zero() == false {
                initial_values.insert(el.key.clone(), el.value.clone());
            }
            let proof = el.proof.iter().map(|el| &el[..]);
            let val = final_trie
                .insert_proof(&key_like.trie_pos_digits, proof, &mut interner)
                .unwrap();
            if val.is_empty() {
                assert!(el.value.is_zero());
            } else {
                let be_integer_encoding = rlp_parse_short_bytes(val).unwrap();
                let returned = BigUint::from_bytes_be(be_integer_encoding);
                assert_eq!(
                    &returned, &el.value,
                    "parsed 0x{:x}, expected 0x{:x}",
                    &returned, &el.value
                );
            }
        }
        // compare roots
        if updates.is_empty() == false || deletes.is_empty() == false && inserts.is_empty() {
            dbg!(hex::encode(new_root));
            dbg!(hex::encode(final_trie.root()));
            dbg!(hex::encode(crypto::sha3::Keccak256::digest(final_trie.root())));
            if new_root.len() == 33 {
                assert_eq!(&new_root[1..], &crypto::sha3::Keccak256::digest(final_trie.root()));
            } else {
                assert_eq!(&new_root[1..], final_trie.root());
            }
            // dbg!(hex::encode(new_root));
            // dbg!(hex::encode(final_trie.root()));
            // dbg!(hex::encode(crypto::sha3::Keccak256::digest(final_trie.root())));
            // todo!()
        } else {
            // nothing for now
        }
    }
}
