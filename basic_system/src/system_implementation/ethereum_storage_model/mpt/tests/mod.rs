mod basic;
mod prestate;
mod reth_trie;
mod serialization;

use alloy::primitives::U256;
use crypto::sha3::Keccak256;
use crypto::MiniDigest;
use ruint::aliases::B160;
use std::collections::{BTreeSet, HashMap};
use std::{alloc::Global, collections::BTreeMap};
use zk_ee::utils::Bytes32;

use super::*;
use crate::system_implementation::ethereum_storage_model::mpt::BoxInterner;

use self::prestate::*;
use self::serialization::*;

pub(crate) fn path_char_to_digit(c: u8) -> u8 {
    match c {
        b'A'..=b'F' => c - b'A' + 10,
        b'a'..=b'f' => c - b'a' + 10,
        b'0'..=b'9' => c - b'0',
        _ => {
            unreachable!()
        }
    }
}

pub(crate) fn byte_path_to_path_digits(byte_path: &[u8; 32]) -> Vec<u8> {
    hex::encode(byte_path)
        .as_bytes()
        .iter()
        .map(|el| path_char_to_digit(*el))
        .collect()
}

pub(crate) fn hex_path_to_path_digits(hex: &str) -> Vec<u8> {
    hex.as_bytes()
        .iter()
        .map(|el| path_char_to_digit(*el))
        .collect()
}

fn decode_address_data<'a>(mut data: &'a [u8]) -> [RLPSlice<'a>; 4] {
    let b0 = consume(&mut data, 1).unwrap();
    let b0 = b0[0];
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

fn decode_prestate_and_diffs() -> (prestate::PrestateTrace, prestate::DiffTrace) {
    let prestate =
        serde_json::from_reader(std::fs::File::open("./prestatetrace.json").unwrap()).unwrap();
    let diffs = serde_json::from_reader(std::fs::File::open("./difftrace.json").unwrap()).unwrap();

    (prestate, diffs)
}

struct ParsedWitness {
    oracle: BTreeMap<Bytes32, Vec<u8>>,
    addresses_to_trie_pos: BTreeMap<Vec<u8>, Bytes32>,
    all_storage_trie_pos: BTreeMap<Vec<u8>, Bytes32>,
    coinbase: [u8; 20],
    initial_root: Vec<u8>,
}

fn read_execution_witness() -> ParsedWitness {
    let content = std::fs::File::open("./block_witness_15c7f5c.json").unwrap();

    let result: TestJsonResponse<alloy_rpc_types_debug::ExecutionWitness> =
        serde_json::from_reader(content).unwrap();
    let result = result.result;

    let mut headers = alloy::rlp::Rlp::new(&result.headers[0]).unwrap();
    let _ = headers.get_next::<[u8; 32]>().unwrap();
    let _ = headers.get_next::<[u8; 32]>().unwrap();
    let coinbase = headers.get_next::<[u8; 20]>().unwrap().unwrap();
    let initial_root = headers.get_next::<[u8; 32]>().unwrap().unwrap();

    let mut oracle = BTreeMap::new();
    let mut addresses_to_trie_pos = BTreeMap::new();
    let mut all_storage_trie_pos = BTreeMap::new();

    // make an oracle
    for el in result.state.iter() {
        let hash = crypto::sha3::Keccak256::digest(el);
        oracle.insert(Bytes32::from_array(hash), el.to_vec());
    }

    for el in result.keys.iter() {
        if el.len() == 20 {
            let hash = crypto::sha3::Keccak256::digest(el);
            oracle.insert(Bytes32::from_array(hash), el.to_vec());
            addresses_to_trie_pos.insert(el.to_vec(), Bytes32::from_array(hash));
        } else if el.len() == 32 {
            let hash = crypto::sha3::Keccak256::digest(el);
            oracle.insert(Bytes32::from_array(hash), el.to_vec());
            all_storage_trie_pos.insert(el.to_vec(), Bytes32::from_array(hash));
        } else {
            panic!("unknown length {}", el.len())
        }
    }

    for el in result.codes.iter() {
        let hash = crypto::sha3::Keccak256::digest(el);
        oracle.insert(Bytes32::from_array(hash), el.to_vec());
    }

    ParsedWitness {
        oracle,
        addresses_to_trie_pos,
        all_storage_trie_pos,
        coinbase,
        initial_root: initial_root.to_vec(),
    }
}

fn rlp_encode_short_slice(slice: &[u8]) -> Vec<u8> {
    if slice.len() == 1 {
        if slice[0] < 0x80 {
            return slice.to_vec();
        }
    }

    if slice.len() <= 55 {
        let mut result = vec![0x80 + (slice.len() as u8)];
        result.extend_from_slice(slice);

        result
    } else {
        assert!(slice.len() < 256);
        let mut result = vec![0xb8, (slice.len() as u8)];
        result.extend_from_slice(slice);

        result
    }
}

#[test]
fn test_from_execution_witness() {
    let data = read_execution_witness();
    let mut interner = BoxInterner::with_capacity_in(1 << 26, Global);
    let mut hasher = crypto::sha3::Keccak256::new();
    let (prestate, diffs) = decode_prestate_and_diffs();

    let account_proofs_at_block_end = std::fs::File::open("./account_proofs.json").unwrap();
    let account_proofs_at_block_end: HashMap<B160, AccountProof> =
        serde_json::from_reader(account_proofs_at_block_end).unwrap();

    let ParsedWitness {
        oracle,
        addresses_to_trie_pos,
        initial_root,
        coinbase,
        all_storage_trie_pos,
    } = data;
    let _ = all_storage_trie_pos;

    let mut trie =
        EthereumMPT::new_in(initial_root.try_into().unwrap(), &mut interner, Global).unwrap();

    let mut initial_state_roots = BTreeMap::new();
    let mut encoded_accounts = BTreeMap::new();
    let mut oracle = oracle;
    let mut new_state_roots = BTreeMap::new();
    let mut initially_empty_accounts = BTreeSet::new();

    let (initial_state, final_state) = compute_initial_and_final_states(prestate, diffs);

    for (address_key, account_state) in initial_state.0.iter() {
        let address = address_key.0.to_be_bytes_vec();
        assert_eq!(address.len(), 20);
        let key = addresses_to_trie_pos[&address];
        let trie_pos_digits = byte_path_to_path_digits(key.as_u8_array_ref());
        let path = Path::new(&trie_pos_digits);
        if let Ok(account_data) = trie.get(path, &mut oracle, &mut interner, &mut hasher) {
            if account_data.is_empty() {
                initially_empty_accounts.insert(*address_key);
                if let Some(storage) = account_state.storage.as_ref() {
                    if storage.is_empty() == false {
                        for (_, v) in storage.iter() {
                            // particularity of our prestate
                            assert!(v.into_inner().is_zero());
                        }
                    }
                }
            } else {
                encoded_accounts.insert(*address_key, account_data.to_vec());
                let data = decode_address_data(account_data);
                initial_state_roots.insert(address_key.clone(), data[2].data().to_vec());

                if let Some(nonce) = account_state.nonce.as_ref() {
                    let encoding = nonce.to_be_bytes();
                    let expected = data[0].data();
                    if expected != &encoding[(8 - expected.len())..] {
                        println!("Account 0x{}: prestate nonce is dirty: storage has 0x{}, prestate has 0x{:x}", hex::encode(&address), hex::encode(data[0].data()), nonce);
                    }
                }
                // There are fee-related divergences
                if let Some(balance) = account_state.balance.as_ref() {
                    if &address != &coinbase {
                        // unclear why
                        if &balance.to_be_bytes_trimmed_vec() != data[1].data() {
                            println!(
                                "Account 0x{}: prestate balance is dirty",
                                hex::encode(&address)
                            );
                        }
                    }
                }
                if let Some(code) = account_state.code.as_ref() {
                    assert_eq!(&Keccak256::digest(code), data[3].data());
                }

                if let Some(storage) = account_state.storage.as_ref() {
                    if data[2].data().is_empty() || data[2].data() == EMPTY_ROOT_HASH.as_u8_ref() {
                        assert!(storage.is_empty())
                    } else {
                        assert!(storage.is_empty() == false);
                    }
                }
            }
        } else {
            panic!(
                "Failed to get account data for address 0x{}",
                hex::encode(address)
            );
        }
    }

    let mut account_storage_tries = BTreeMap::new();

    for (address, root) in initial_state_roots.into_iter() {
        let initial_storage = initial_state.0.get(&address).unwrap();
        let mut storage_trie =
            EthereumMPT::new_in(root.try_into().unwrap(), &mut interner, Global).unwrap();

        if let Some(storage) = initial_storage.storage.as_ref() {
            for (k, v) in storage {
                if storage_trie.root(&mut hasher) == EMPTY_ROOT_HASH.as_u8_array() {
                    assert!(v.into_inner().is_zero());
                }
                let key = crypto::sha3::Keccak256::digest(&k.to_be_bytes::<32>());
                let trie_pos_digits = byte_path_to_path_digits(&key);
                let path = Path::new(&trie_pos_digits);
                if let Ok(slot_value) =
                    storage_trie.get(path, &mut oracle, &mut interner, &mut hasher)
                {
                    let integer_encoding = if slot_value.len() > 0 {
                        rlp_parse_short_bytes(slot_value).unwrap()
                    } else {
                        slot_value
                    };
                    assert_eq!(integer_encoding, &v.into_inner().to_be_bytes_trimmed_vec());
                } else {
                    panic!(
                        "For address 0x{}: failed to get slot 0x{:x} (key 0x{})",
                        hex::encode(address.0.to_be_bytes_vec()),
                        k,
                        hex::encode(key)
                    );
                }
            }
        }
        let existing = account_storage_tries.insert(address, storage_trie);
        assert!(existing.is_none());
    }

    let mut accounts_with_unchanged_state = BTreeSet::new();
    let mut left_empty_untouched_accounts = BTreeSet::new();
    // let mut expected_account_proofs = HashMap::new();

    let mut initial_state_t = initial_state.clone();
    for (address, final_storage) in final_state.0.iter() {
        let _ = initial_state_t.0.remove(address).unwrap();

        let mut updates = BTreeMap::new();
        let mut deletes = BTreeMap::new();
        let mut inserts = BTreeMap::new();

        let initial_storage = initial_state
            .0
            .get(address)
            .cloned()
            .unwrap_or_default()
            .storage
            .unwrap_or_default();
        let final_storage = final_storage.storage.clone().unwrap_or_default();

        for (k, final_value) in final_storage.into_iter() {
            let key = Keccak256::digest(k.to_be_bytes::<32>());
            if let Some(initial_value) = initial_storage.get(&k) {
                if initial_value.into_inner().is_zero() == false {
                    if final_value.into_inner().is_zero() {
                        deletes.insert(key, k);
                    } else {
                        if initial_value.into_inner() != final_value.into_inner() {
                            updates.insert(key, (k, (*initial_value, final_value)));
                        }
                    }
                } else {
                    // potentially insert
                    if final_value.into_inner().is_zero() == false {
                        inserts.insert(key, (k, final_value));
                    }
                }
            } else {
                if final_value.into_inner().is_zero() == false {
                    inserts.insert(key, (k, final_value));
                }
            }
        }
        let reads_only = updates.is_empty() && deletes.is_empty() && inserts.is_empty();

        if updates.is_empty() == false {
            // updates
            let storage_trie = account_storage_tries.get_mut(address).unwrap();
            assert!(storage_trie.root.is_empty() == false);
            assert!(storage_trie.root(&mut hasher) != EMPTY_ROOT_HASH.as_u8_array());

            for (k, (_plain_k, (old_v, new_v))) in updates.iter() {
                assert!(old_v.into_inner().is_zero() == false);
                assert!(new_v.into_inner().is_zero() == false);
                assert!(old_v.into_inner() != new_v.into_inner());
                let trie_pos_digits = byte_path_to_path_digits(k);
                let path = Path::new(&trie_pos_digits);
                let value_be = new_v.into_inner().to_be_bytes_trimmed_vec();
                let new_value = rlp_encode_short_slice(&rlp_encode_short_slice(&value_be));
                storage_trie
                    .update(path, &new_value, &mut interner, &mut hasher)
                    .unwrap();
            }
        }
        if deletes.is_empty() == false {
            // deletes
            let storage_trie = account_storage_tries.get_mut(address).unwrap();
            assert!(storage_trie.root.is_empty() == false);
            assert!(storage_trie.root(&mut hasher) != EMPTY_ROOT_HASH.as_u8_array());
            for (k, _plain_k) in deletes.iter() {
                let trie_pos_digits = byte_path_to_path_digits(k);
                let path = Path::new(&trie_pos_digits);
                storage_trie
                    .delete(path, &mut oracle, &mut interner, &mut hasher)
                    .unwrap();
            }
        }
        if inserts.is_empty() == false {
            // inserts
            let storage_trie = account_storage_tries.entry(*address).or_insert_with(|| {
                EthereumMPT::new_in(EMPTY_ROOT_HASH.as_u8_array(), &mut interner, Global).unwrap()
            });
            for (k, (_plain_k, new_v)) in inserts.iter() {
                let trie_pos_digits = byte_path_to_path_digits(k);
                let path = Path::new(&trie_pos_digits);
                let value_be = new_v.into_inner().to_be_bytes_trimmed_vec();
                let new_value = rlp_encode_short_slice(&rlp_encode_short_slice(&value_be));
                storage_trie
                    .insert(path, &new_value, &mut oracle, &mut interner, &mut hasher)
                    .unwrap();
            }
        }
        if reads_only && initially_empty_accounts.contains(address) {
            // will leave empty as-is
            left_empty_untouched_accounts.insert(*address);
        } else {
            let storage_trie = account_storage_tries.get_mut(address).unwrap();
            storage_trie.ensure_linked();
            let old_root = storage_trie.root(&mut hasher);
            storage_trie.recompute(&mut interner, &mut hasher).unwrap();
            let new_root = storage_trie.root(&mut hasher);
            if reads_only {
                assert_eq!(old_root, new_root);
                accounts_with_unchanged_state.insert(*address);
            } else {
                assert_ne!(old_root, new_root);
            }

            let account_proof = &account_proofs_at_block_end[&address.0];
            assert_eq!(
                new_root,
                account_proof.storage_hash.0,
                "account storage root diverged for address 0x{:040x}",
                address.0.into_inner()
            );

            new_state_roots.insert(*address, storage_trie.root(&mut hasher).to_vec());
        }
    }

    let _ = new_state_roots;

    // folding everything back will not work, as there are other dirty values that we do not know
}
