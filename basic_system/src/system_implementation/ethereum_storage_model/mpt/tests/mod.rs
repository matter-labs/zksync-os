mod serialization;

use crypto::MiniDigest;
use num_bigint::BigUint;
use num_traits::Zero;
use zk_ee::utils::Bytes32;
use std::collections::BTreeSet;
use std::{alloc::Global, collections::BTreeMap};

use super::*;
use crate::system_implementation::ethereum_storage_model::mpt::BoxInterner;

use self::serialization::*;

fn decode_address_data<'a>(
    mut data: &'a [u8],
) -> [RLPSlice<'a>; 4] {
    let b0 = consume(&mut data, 1).unwrap();
    let b0 = b0[0];
    // we can not make any conclusion based on the first byte. At best we can make a decision that it's a list,
    // but not even the number of elements in it...
    if b0 < 0xc0 {
        panic!();
    }
    if b0 < 0xf8 {
        // list of unknown(!) length, even though the concatenation is short. Yes, we can not make a decision about
        // validity until we parse the full encoding, but at least let's reject some trivial cases
        let expected_len = b0 - 0xc0;
        if data.len() != expected_len as usize {
            panic!();
        }
        // either it's a leaf/extension that is a list of two, or branch
        let mut result = [RLPSlice::empty(); 4];
        for dst in result.iter_mut() {
            // and itself it must be a string, not a list
            *dst = RLPSlice::parse(&mut data).unwrap();
        }
        if data.is_empty() == false {
            panic!();
        }

        result
    } else {
        // list of large length. But we do not expect it "too large"
        let length_encoding_length = (b0 - 0xf7) as usize;
        let length_encoding_bytes = consume(&mut data, length_encoding_length).unwrap();
        if length_encoding_bytes.len() > 2 {
            panic!();
        }
        let mut be_bytes = [0u8; 4];
        be_bytes[(4 - length_encoding_bytes.len())..].copy_from_slice(length_encoding_bytes);
        let length = u32::from_be_bytes(be_bytes) as usize;
        if data.len() != length {
            panic!();
        }
        let mut result = [RLPSlice::empty(); 4];
        for dst in result.iter_mut() {
            // and itself it must be a string, not a list, and can not be longer than 32 bytes
            *dst = RLPSlice::parse(&mut data).unwrap()
        }
        if data.is_empty() == false {
            panic!()
        }

        result
    }
}

// pub(crate) fn read_test_vectors() -> BTreeMap<Vec<u8>, (AccountProof, AccountProof)> {
//     let paths = std::fs::read_dir("./proofs").unwrap();

//     let mut mapping: BTreeMap<Vec<u8>, BTreeMap<u64, AccountProof>> = BTreeMap::new();

//     for path in paths {
//         if let Ok(path) = path {
//             let pp = path.path();
//             let p = pp.strip_prefix("./proofs/").unwrap();
//             let full_name = p
//                 .file_name()
//                 .unwrap()
//                 .to_string_lossy()
//                 .to_owned()
//                 .into_owned();
//             let t = full_name.strip_suffix(".json").unwrap();
//             let mut it = t.split("_");
//             let key = it.next().unwrap();
//             let key = hex::decode(key).unwrap();
//             let block_number = it.next().unwrap();
//             let block_number = u64::from_str_radix(block_number, 16).unwrap();

//             let content = std::fs::File::open(path.path()).unwrap();
//             let data: TestJsonResponse<AccountProof> = serde_json::from_reader(content).unwrap();
//             let entry: &mut BTreeMap<u64, AccountProof> = mapping.entry(key).or_default();
//             entry.insert(block_number, data.result);
//         }
//     }

//     let mut result = BTreeMap::new();
//     for (key, values) in mapping.into_iter() {
//         assert_eq!(values.len(), 2);
//         let mut it = values.into_iter();
//         let (_, a) = it.next().unwrap();
//         let (_, b) = it.next().unwrap();
//         result.insert(key, (a, b));
//     }

//     result
// }

struct ParsedWitness {
    oracle: BTreeMap<Bytes32, Vec<u8>>,
    addresses_to_trie_pos: BTreeMap<Vec<u8>, Bytes32>,
    all_storage_trie_pos: BTreeMap<Vec<u8>, Bytes32>,
    // keys_to_trie_pos: BTreeMap<Vec<u8>, Bytes32>,
    // trie_pos_to_key: BTreeMap<Bytes32, Vec<u8>>,
    initial_root: Vec<u8>,
}

fn read_execution_witness() -> ParsedWitness {
    let content = std::fs::File::open("./block_witness_15c7f5c.json").unwrap();
    let result: TestJsonResponse<alloy_rpc_types_debug::ExecutionWitness> = serde_json::from_reader(content).unwrap();
    let result = result.result;

    let mut oracle = BTreeMap::new();
    let mut addresses_to_trie_pos = BTreeMap::new();
    let mut all_storage_trie_pos = BTreeMap::new();

    // make an oracle
    for el in result.state.iter() {
        // assert!(el.len() >= 32);
        let hash = crypto::sha3::Keccak256::digest(el);
        // dbg!(hex::encode(&hash));
        let existing = oracle.insert(Bytes32::from_array(hash), el.to_vec());        
        assert!(existing.is_none());
    }

    for el in result.keys.iter() {
        if el.len() == 20 {
            // println!("address");
            let hash = crypto::sha3::Keccak256::digest(el);
            let _ = oracle.insert(Bytes32::from_array(hash), el.to_vec());      
            addresses_to_trie_pos.insert(el.to_vec(), Bytes32::from_array(hash)); 
        } else if el.len() == 32 {
            // println!("storage key");
            let hash = crypto::sha3::Keccak256::digest(el);
            let _ = oracle.insert(Bytes32::from_array(hash), el.to_vec());      
            all_storage_trie_pos.insert(el.to_vec(), Bytes32::from_array(hash)); 
        } else {
            panic!("unknown length {}", el.len())
        }      
    }

    for el in result.codes.iter() {
        let hash = crypto::sha3::Keccak256::digest(el);
        // dbg!(hex::encode(&hash));       
        let existing = oracle.insert(Bytes32::from_array(hash), el.to_vec());        
        assert!(existing.is_none());
    }

    // dbg!(hex::encode(&result.headers[0]));

    let initial_root = hex::decode("3d7cc711f7fe1d2e4cdcc5c4763d18612b976736dbd7631e9c232aa592e2f011").unwrap();

    ParsedWitness {
        oracle,
        addresses_to_trie_pos,
        all_storage_trie_pos,
        initial_root,
    }
}

fn encode_integer_as_rlp_slice(value: &BigUint) -> Vec<u8> {
    assert!(value.is_zero() == false);
    let be_encoding = value.to_bytes_be();
    assert!(be_encoding.len() <= 32);
    if be_encoding.len() == 1 && be_encoding[0] < 0x80 {
        return be_encoding;
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
fn test_from_execution_witness() {
    let data = read_execution_witness();
    let mut interner = BoxInterner::with_capacity_in(1 << 20, Global);
    let mut hasher = crypto::sha3::Keccak256::new();
    let ParsedWitness {
        oracle,
        addresses_to_trie_pos,
        all_storage_trie_pos,
        initial_root 
    } = data;

    let mut trie = EthereumMPT::new_in(&initial_root, &mut interner, Global).unwrap();

    let mut state_roots = BTreeMap::new();
    let mut oracle = oracle;

    for (address, key) in addresses_to_trie_pos.iter() {
        assert_eq!(address.len(), 20);
        // ignore precompiles
        if address.iter().filter(|el| **el != 0).count() < 2 {
            continue;
        }
        let trie_pos_digits_string = hex::encode(key.as_u8_array_ref()).as_bytes().to_vec();
        let trie_pos_digits: Vec<_> = trie_pos_digits_string
            .iter()
            .map(|el| path_char_to_digit(*el))
            .collect();
        let path = Path::new(&trie_pos_digits);
        // if hex::encode(address) == "000000000022d473030f116ddee9f6b43ac78ba3" {
        //     println!("DEBUG");
        // }
        if let Ok(account_data) = trie.access_initial_value(
            path,
            &mut oracle,
            &mut interner,
            &mut hasher
        ) {
            if account_data.is_empty() {
                println!("Account 0x{} is empty", hex::encode(address));
                state_roots.insert(address.clone(), EMPTY_ROOT_HASH.as_u8_array_ref().to_vec());
            } else {
                let data = decode_address_data(account_data);
                println!("Data for address 0x{}", hex::encode(address));
                println!("Nonce = 0x{}", hex::encode(data[0].data()));
                println!("Balance = 0x{}", hex::encode(data[1].data()));
                println!("Storage root = 0x{}", hex::encode(data[2].data()));
                println!("Code hash = 0x{}", hex::encode(data[3].data()));
                state_roots.insert(address.clone(), data[2].data().to_vec());
            }
        } else {
            println!("Failed to get account data for address 0x{}", hex::encode(address));
        }
    }

    // for (address, (initial_state, final_state)) in map.iter() {
    //     if initial_state.storage_proof.is_empty() {
    //         continue;
    //     }
    //     // dbg!(hex::encode(address));

    //     if hex::encode(address) != "14fee680690900ba0cccfc76ad70fd1b95d10e16" {
    //         continue;
    //     }

    //     let mut trie = EthereumMPT::new_in(Global);
    //     let mut initial_values = BTreeMap::new();
    //     for el in initial_state.storage_proof.iter() {
    //         let key_like = &el.key;
    //         assert_eq!(key_like.trie_pos_digits.len(), 64);
    //         // println!(
    //         //     "Checking proofs for key {}, expected value 0x{:x}",
    //         //     hex::encode(&key_like.key),
    //         //     &el.value
    //         // );
    //         // println!(
    //         //     "Trie position is {}",
    //         //     std::str::from_utf8(&key_like.trie_pos_digits_string).unwrap()
    //         // );
    //         if el.value.is_zero() == false {
    //             initial_values.insert(el.key.clone(), el.value.clone());
    //         }

    //         let proof = el.proof.iter().map(|el| &el[..]);
    //         let val = trie
    //             .insert_proof(&key_like.trie_pos_digits, proof, &mut interner)
    //             .unwrap();
    //         if val.is_empty() {
    //             assert!(el.value.is_zero());
    //         } else {
    //             let be_integer_encoding = rlp_parse_short_bytes(val).unwrap();
    //             let returned = BigUint::from_bytes_be(be_integer_encoding);
    //             assert_eq!(
    //                 &returned, &el.value,
    //                 "parsed 0x{:x}, expected 0x{:x}",
    //                 &returned, &el.value
    //             );
    //         }
    //     }
    //     let mut updates = BTreeMap::new();
    //     let mut deletes = BTreeSet::new();
    //     let mut inserts = BTreeMap::new();
    //     for el in final_state.storage_proof.iter() {
    //         let key_like = &el.key;
    //         if let Some(initial) = initial_values.get(&el.key) {
    //             if el.value.is_zero() {
    //                 // println!("Will delete value for key {}", hex::encode(&key_like.key),);
    //                 // println!(
    //                 //     "Trie position is {}",
    //                 //     std::str::from_utf8(&key_like.trie_pos_digits_string).unwrap()
    //                 // );
    //                 deletes.insert(el.key.clone());
    //             } else if initial != &el.value {
    //                 // println!(
    //                 //     "Will update new value for key {}, new value 0x{:x}",
    //                 //     hex::encode(&key_like.key),
    //                 //     &el.value
    //                 // );
    //                 // println!(
    //                 //     "Trie position is {}",
    //                 //     std::str::from_utf8(&key_like.trie_pos_digits_string).unwrap()
    //                 // );
    //                 updates.insert(el.key.clone(), el.value.clone());
    //             }
    //         } else {
    //             if el.value.is_zero() == false {
    //                 // println!(
    //                 //     "Will insert new value for key {}, inserted value 0x{:x}",
    //                 //     hex::encode(&key_like.key),
    //                 //     &el.value
    //                 // );
    //                 // println!(
    //                 //     "Trie position is {}",
    //                 //     std::str::from_utf8(&key_like.trie_pos_digits_string).unwrap()
    //                 // );
    //                 inserts.insert(el.key.clone(), el.value.clone());
    //             }
    //         }
    //     }
    //     let mut new_root = &[][..];

    //     // if updates.is_empty() == false && deletes.is_empty() && inserts.is_empty() {

    //     // } else {
    //     //     continue;
    //     // }

    //     if deletes.is_empty() {
    //         continue;
    //     }

    //     if updates.is_empty() == false || deletes.is_empty() == false && inserts.is_empty() {
    //     } else {
    //         continue;
    //     }

    //     dbg!(hex::encode(address));
    //     let mut hasher = crypto::sha3::Keccak256::new();

    //     // perform updates
    //     if updates.is_empty() && inserts.is_empty() && deletes.is_empty() {
    //         // nothing
    //     } else {
    //         for (k, v) in updates.iter() {
    //             println!(
    //                 "Will update new value for key {}, new value 0x{:x}",
    //                 hex::encode(&k.key),
    //                 &v
    //             );
    //             println!(
    //                 "Trie position is {}",
    //                 std::str::from_utf8(&k.trie_pos_digits_string).unwrap()
    //             );
    //             let new_value = encode_integer_as_raw_terminal_value(&v);
    //             new_root = trie
    //                 .update(&k.trie_pos_digits, &new_value, &mut interner, &mut hasher)
    //                 .unwrap();
    //         }
    //         for k in deletes.iter() {
    //             println!("Will delete value for key {}", hex::encode(&k.key),);
    //             println!(
    //                 "Trie position is {}",
    //                 std::str::from_utf8(&k.trie_pos_digits_string).unwrap()
    //             );
    //             new_root = trie
    //                 .delete(&k.trie_pos_digits, &mut interner, &mut hasher)
    //                 .unwrap();
    //         }
    //     }

    //     // recheck the proofs
    //     let mut final_trie = EthereumMPT::new_in(Global);
    //     for el in final_state.storage_proof.iter() {
    //         let key_like = &el.key;
    //         assert_eq!(key_like.trie_pos_digits.len(), 64);
    //         // println!(
    //         //     "Checking proofs for key {}, expected value 0x{:x}",
    //         //     hex::encode(&key_like.key),
    //         //     &el.value
    //         // );
    //         // println!(
    //         //     "Trie position is {}",
    //         //     std::str::from_utf8(&key_like.trie_pos_digits_string).unwrap()
    //         // );
    //         if el.value.is_zero() == false {
    //             initial_values.insert(el.key.clone(), el.value.clone());
    //         }
    //         let proof = el.proof.iter().map(|el| &el[..]);
    //         let val = final_trie
    //             .insert_proof(&key_like.trie_pos_digits, proof, &mut interner)
    //             .unwrap();
    //         if val.is_empty() {
    //             assert!(el.value.is_zero());
    //         } else {
    //             let be_integer_encoding = rlp_parse_short_bytes(val).unwrap();
    //             let returned = BigUint::from_bytes_be(be_integer_encoding);
    //             assert_eq!(
    //                 &returned, &el.value,
    //                 "parsed 0x{:x}, expected 0x{:x}",
    //                 &returned, &el.value
    //             );
    //         }
    //     }
    //     // compare roots
    //     if updates.is_empty() == false || deletes.is_empty() == false && inserts.is_empty() {
    //         dbg!(hex::encode(new_root));
    //         dbg!(hex::encode(final_trie.root()));
    //         dbg!(hex::encode(crypto::sha3::Keccak256::digest(
    //             final_trie.root()
    //         )));
    //         if new_root.len() == 33 {
    //             assert_eq!(
    //                 &new_root[1..],
    //                 &crypto::sha3::Keccak256::digest(final_trie.root())
    //             );
    //         } else {
    //             assert_eq!(&new_root[1..], final_trie.root());
    //         }
    //         // dbg!(hex::encode(new_root));
    //         // dbg!(hex::encode(final_trie.root()));
    //         // dbg!(hex::encode(crypto::sha3::Keccak256::digest(final_trie.root())));
    //         // todo!()
    //     } else {
    //         // nothing for now
    //     }
    // }
}

// #[test]
// fn parse_pre_states() {
//     let map = read_test_vectors();
//     let mut interner = BoxInterner::with_capacity_in(1 << 20, Global);
//     for (address, (initial_state, final_state)) in map.iter() {
//         if initial_state.storage_proof.is_empty() {
//             continue;
//         }
//         // dbg!(hex::encode(address));

//         if hex::encode(address) != "14fee680690900ba0cccfc76ad70fd1b95d10e16" {
//             continue;
//         }

//         let mut trie = EthereumMPT::new_in(Global);
//         let mut initial_values = BTreeMap::new();
//         for el in initial_state.storage_proof.iter() {
//             let key_like = &el.key;
//             assert_eq!(key_like.trie_pos_digits.len(), 64);
//             // println!(
//             //     "Checking proofs for key {}, expected value 0x{:x}",
//             //     hex::encode(&key_like.key),
//             //     &el.value
//             // );
//             // println!(
//             //     "Trie position is {}",
//             //     std::str::from_utf8(&key_like.trie_pos_digits_string).unwrap()
//             // );
//             if el.value.is_zero() == false {
//                 initial_values.insert(el.key.clone(), el.value.clone());
//             }

//             let proof = el.proof.iter().map(|el| &el[..]);
//             let val = trie
//                 .insert_proof(&key_like.trie_pos_digits, proof, &mut interner)
//                 .unwrap();
//             if val.is_empty() {
//                 assert!(el.value.is_zero());
//             } else {
//                 let be_integer_encoding = rlp_parse_short_bytes(val).unwrap();
//                 let returned = BigUint::from_bytes_be(be_integer_encoding);
//                 assert_eq!(
//                     &returned, &el.value,
//                     "parsed 0x{:x}, expected 0x{:x}",
//                     &returned, &el.value
//                 );
//             }
//         }
//         let mut updates = BTreeMap::new();
//         let mut deletes = BTreeSet::new();
//         let mut inserts = BTreeMap::new();
//         for el in final_state.storage_proof.iter() {
//             let key_like = &el.key;
//             if let Some(initial) = initial_values.get(&el.key) {
//                 if el.value.is_zero() {
//                     // println!("Will delete value for key {}", hex::encode(&key_like.key),);
//                     // println!(
//                     //     "Trie position is {}",
//                     //     std::str::from_utf8(&key_like.trie_pos_digits_string).unwrap()
//                     // );
//                     deletes.insert(el.key.clone());
//                 } else if initial != &el.value {
//                     // println!(
//                     //     "Will update new value for key {}, new value 0x{:x}",
//                     //     hex::encode(&key_like.key),
//                     //     &el.value
//                     // );
//                     // println!(
//                     //     "Trie position is {}",
//                     //     std::str::from_utf8(&key_like.trie_pos_digits_string).unwrap()
//                     // );
//                     updates.insert(el.key.clone(), el.value.clone());
//                 }
//             } else {
//                 if el.value.is_zero() == false {
//                     // println!(
//                     //     "Will insert new value for key {}, inserted value 0x{:x}",
//                     //     hex::encode(&key_like.key),
//                     //     &el.value
//                     // );
//                     // println!(
//                     //     "Trie position is {}",
//                     //     std::str::from_utf8(&key_like.trie_pos_digits_string).unwrap()
//                     // );
//                     inserts.insert(el.key.clone(), el.value.clone());
//                 }
//             }
//         }
//         let mut new_root = &[][..];

//         // if updates.is_empty() == false && deletes.is_empty() && inserts.is_empty() {

//         // } else {
//         //     continue;
//         // }

//         if deletes.is_empty() {
//             continue;
//         }

//         if updates.is_empty() == false || deletes.is_empty() == false && inserts.is_empty() {
//         } else {
//             continue;
//         }

//         dbg!(hex::encode(address));
//         let mut hasher = crypto::sha3::Keccak256::new();

//         // perform updates
//         if updates.is_empty() && inserts.is_empty() && deletes.is_empty() {
//             // nothing
//         } else {
//             for (k, v) in updates.iter() {
//                 println!(
//                     "Will update new value for key {}, new value 0x{:x}",
//                     hex::encode(&k.key),
//                     &v
//                 );
//                 println!(
//                     "Trie position is {}",
//                     std::str::from_utf8(&k.trie_pos_digits_string).unwrap()
//                 );
//                 let new_value = encode_integer_as_raw_terminal_value(&v);
//                 new_root = trie
//                     .update(&k.trie_pos_digits, &new_value, &mut interner, &mut hasher)
//                     .unwrap();
//             }
//             for k in deletes.iter() {
//                 println!("Will delete value for key {}", hex::encode(&k.key),);
//                 println!(
//                     "Trie position is {}",
//                     std::str::from_utf8(&k.trie_pos_digits_string).unwrap()
//                 );
//                 new_root = trie
//                     .delete(&k.trie_pos_digits, &mut interner, &mut hasher)
//                     .unwrap();
//             }
//         }

//         // recheck the proofs
//         let mut final_trie = EthereumMPT::new_in(Global);
//         for el in final_state.storage_proof.iter() {
//             let key_like = &el.key;
//             assert_eq!(key_like.trie_pos_digits.len(), 64);
//             // println!(
//             //     "Checking proofs for key {}, expected value 0x{:x}",
//             //     hex::encode(&key_like.key),
//             //     &el.value
//             // );
//             // println!(
//             //     "Trie position is {}",
//             //     std::str::from_utf8(&key_like.trie_pos_digits_string).unwrap()
//             // );
//             if el.value.is_zero() == false {
//                 initial_values.insert(el.key.clone(), el.value.clone());
//             }
//             let proof = el.proof.iter().map(|el| &el[..]);
//             let val = final_trie
//                 .insert_proof(&key_like.trie_pos_digits, proof, &mut interner)
//                 .unwrap();
//             if val.is_empty() {
//                 assert!(el.value.is_zero());
//             } else {
//                 let be_integer_encoding = rlp_parse_short_bytes(val).unwrap();
//                 let returned = BigUint::from_bytes_be(be_integer_encoding);
//                 assert_eq!(
//                     &returned, &el.value,
//                     "parsed 0x{:x}, expected 0x{:x}",
//                     &returned, &el.value
//                 );
//             }
//         }
//         // compare roots
//         if updates.is_empty() == false || deletes.is_empty() == false && inserts.is_empty() {
//             dbg!(hex::encode(new_root));
//             dbg!(hex::encode(final_trie.root()));
//             dbg!(hex::encode(crypto::sha3::Keccak256::digest(
//                 final_trie.root()
//             )));
//             if new_root.len() == 33 {
//                 assert_eq!(
//                     &new_root[1..],
//                     &crypto::sha3::Keccak256::digest(final_trie.root())
//                 );
//             } else {
//                 assert_eq!(&new_root[1..], final_trie.root());
//             }
//             // dbg!(hex::encode(new_root));
//             // dbg!(hex::encode(final_trie.root()));
//             // dbg!(hex::encode(crypto::sha3::Keccak256::digest(final_trie.root())));
//             // todo!()
//         } else {
//             // nothing for now
//         }
//     }
// }
