use crate::prestate::*;
use crate::receipts::TransactionReceipt;
use alloy::hex;
use rig::forward_system::run::BatchOutput;
use ruint::aliases::{B160, B256, U256};
use std::collections::HashMap;

impl DiffTrace {
    fn collect_diffs(self, prestate_cache: &Cache, miner: B160) -> HashMap<B160, AccountState> {
        let mut updates: HashMap<B160, AccountState> = HashMap::new();
        self.result.into_iter().for_each(|item| {
            item.result.post.into_iter().for_each(|(address, account)| {
                if address.0 != miner {
                    let entry = updates.entry(address.0).or_default();
                    account
                        .balance
                        .into_iter()
                        .for_each(|bal| entry.balance = Some(bal));
                    account
                        .nonce
                        .into_iter()
                        .for_each(|x| entry.nonce = Some(x));
                    account.code.into_iter().for_each(|x| entry.code = Some(x));

                    // Populate storage slot clears (slots present in pre but
                    // absent in post). Write 0 to them.
                    if let Some(pre_account) = item.result.pre.get(&address) {
                        if let Some(pre_storage) = pre_account.storage.as_ref() {
                            let cleared_keys = pre_storage.keys().filter(|k| {
                                account
                                    .storage
                                    .as_ref()
                                    .is_none_or(|post_storage| !post_storage.contains_key(k))
                            });
                            let entry_storage = entry.storage.get_or_insert_default();
                            cleared_keys.into_iter().for_each(|key| {
                                entry_storage.insert(*key, B256::ZERO);
                            })
                        }
                    }

                    // Populate storage slot writes
                    if let Some(storage) = account.storage {
                        let entry_storage = entry.storage.get_or_insert_default();
                        storage.into_iter().for_each(|(key, value)| {
                            entry_storage.insert(key, value);
                        })
                    }
                }
            })
        });
        // Filter out empty diffs
        // These can be empty because their value is the same as in the initial tree
        // or the post state was empty. Note that if the account was selfdestructed,
        // the address shouldn't be present in the post state. This is just a strange
        // case where the logs add an empty entry for accounts that haven't been
        // modified.

        // TODO: account for selfdestruct
        updates.retain(|address, account| {
            if let Some(storage) = account.storage.as_mut() {
                storage.retain(|key, new_val| match prestate_cache.get_slot(address, key) {
                    None => *new_val != B256::ZERO,
                    Some(initial) => *new_val != initial,
                })
            }
            if account.balance == prestate_cache.get_balance(address) {
                account.balance = None
            }
            if account.nonce == prestate_cache.get_nonce(address) {
                account.nonce = None
            }
            if account.code == prestate_cache.get_code(address) {
                account.code = None
            }
            !account.is_empty()
        });

        updates
    }

    pub fn check_storage_writes(self, output: BatchOutput, prestate_cache: Cache, miner: B160) {
        let diffs = self.collect_diffs(&prestate_cache, miner);
        let zksync_os_diffs = zksync_os_output_into_account_state(output, &prestate_cache);

        // Reference => ZKsync OS check:
        diffs.iter().for_each(|(address, account)| {
            let zk_account = zksync_os_diffs.get(address).unwrap_or_else(|| {
                panic!(
                    "ZKsync OS must have write for account {} {:?}",
                    hex::encode(address.to_be_bytes_vec()),
                    account
                )
            });
            if let Some(bal) = account.balance {
                // Balance might differ due to refunds and access list gas charging
                if bal != zk_account.balance.unwrap() {
                    println!(
                        "Balance for {} is {:?} but expected {:?}.\n  Difference: {:?}",
                        hex::encode(address.to_be_bytes_vec()),
                        zk_account.balance.unwrap(),
                        bal,
                        zk_account.balance.unwrap().abs_diff(bal),
                    )
                };
            }
            if let Some(nonce) = account.nonce {
                assert_eq!(nonce, zk_account.nonce.unwrap());
            }
            if account.code.is_some() {
                assert_eq!(&account.code, &zk_account.code);
            }
            if let Some(storage) = &account.storage {
                for (key, value) in storage {
                    let zksync_os_value = zk_account
                        .storage
                        .as_ref()
                        .unwrap()
                        .get(key)
                        .unwrap_or_else(|| {
                            panic!(
                                "Should have value for slot {} at address {}",
                                key,
                                hex::encode(address.to_be_bytes_vec())
                            )
                        });
                    assert_eq!(value, zksync_os_value);
                }

                zk_account
                    .storage
                    .as_ref()
                    .unwrap()
                    .iter()
                    .for_each(|(k, v)| {
                        // In the diff trace, slot clearing is not present in post,
                        // so we have to allow the case when v == 0.
                        assert!(
                            v.as_uint().is_zero() || storage.contains_key(k),
                            "Key {:?} for {:?} not present in reference",
                            k,
                            address
                        )
                    })
            }
        });

        // ZKsync OS => reference
        zksync_os_diffs.iter().for_each(|(address, acc)| {
            // Just check that it's part of the reference diffs,
            // all else should be checked already
            if address != &miner && !acc.is_empty() {
                diffs.get(address).unwrap_or_else(|| {
                    panic!(
                        "Reference must have write for account {} {:?}",
                        hex::encode(address.to_be_bytes_vec()),
                        acc
                    )
                });
            }
        });
    }
}

fn zksync_os_output_into_account_state(
    output: BatchOutput,
    prestate_cache: &Cache,
) -> HashMap<B160, AccountState> {
    use basic_system::system_implementation::flat_storage_model::AccountProperties;
    let mut updates: HashMap<B160, AccountState> = HashMap::new();
    let preimages: HashMap<[u8; 32], Vec<u8>> = HashMap::from_iter(
        output
            .published_preimages
            .into_iter()
            .map(|(key, value, _)| (key.as_u8_array(), value)),
    );
    for w in output.storage_writes {
        if rig::chain::is_account_properties_address(&w.account) {
            // populate account
            let address: [u8; 20] = w.account_key.as_u8_array()[12..].try_into().unwrap();
            let address = B160::from_be_bytes(address);
            if address != system_hooks::addresses_constants::BOOTLOADER_FORMAL_ADDRESS {
                let props = if w.value.is_zero() {
                    // TODO: Account deleted, we need to check this somehow
                    AccountProperties::default()
                } else {
                    let encoded = preimages
                        .get(&w.value.as_u8_array())
                        .unwrap_or_else(|| {
                            panic!("Must contain preimage for account {:#?}", address)
                        })
                        .clone();
                    AccountProperties::decode(&encoded.try_into().unwrap())
                };
                let entry = updates.entry(address).or_default();
                entry.balance = Some(props.balance);
                entry.nonce = Some(props.nonce);
                if let Some(bytecode) = preimages.get(&props.bytecode_hash.as_u8_array()) {
                    let owned = bytecode.clone();
                    entry.code = Some(owned.into());
                }
            }
        } else {
            // populate slot
            let address = w.account;
            let key = U256::from_be_bytes(w.account_key.as_u8_array());
            let entry = updates.entry(address).or_default();
            let value = B256::from_be_bytes(w.value.as_u8_array());
            entry.storage.get_or_insert_default().insert(key, value);
        }
    }

    // Filter out empty diffs
    updates.retain(|address, account| {
        if let Some(storage) = account.storage.as_mut() {
            storage.retain(|key, new_val| match prestate_cache.get_slot(address, key) {
                None => *new_val != B256::ZERO,
                Some(initial) => *new_val != initial,
            })
        }
        if account.balance == prestate_cache.get_balance(address) {
            account.balance = None
        }
        if account.nonce == prestate_cache.get_nonce(address) {
            account.nonce = None
        }
        if account.code == prestate_cache.get_code(address) {
            account.code = None
        }
        !account.is_empty()
    });

    updates
}

// EVM refunds are only done in SSTORE, and they
// can be of only 3 different values: 19900, 2800 and 4800.
// However, gas refunds are capped at 20% of the total gas used.
// Therefore, we use the following heuristic to check if a difference
// in gas used is a refund:
//  (∃a,b,c s.t. gas_difference = a * 19900 + b * 2800 + c * 4800) ∨
//   zk_sync_os_gas_used / 5 = gas_difference
pub fn consistent_with_refund(zksync_os_gas_used: u64, gas_difference: u64) -> bool {
    fn has_refund_decomposition(x: u64) -> bool {
        if x % 100 != 0 {
            return false;
        }

        let x = x / 100; // reduce the equation: 199a + 28b + 48c = x
        for a in 0..=x / 199 {
            let rem = x - 199 * a;
            if rem % 4 != 0 {
                continue;
            }

            let r = rem / 4; // now checking 7b + 12c = r

            // Try all possible c values (small loop)
            for c in 0..=r / 12 {
                let rem2 = r - 12 * c;
                if rem2 % 7 == 0 {
                    return true;
                }
            }
        }
        false
    }
    has_refund_decomposition(gas_difference) || zksync_os_gas_used / 5 == gas_difference
}

pub fn post_check(
    output: BatchOutput,
    receipts: Vec<TransactionReceipt>,
    diff_trace: DiffTrace,
    prestate_cache: Cache,
    miner: B160,
) {
    output
        .tx_results
        .iter()
        .zip(receipts.iter())
        .for_each(|(res, receipt)| {
            let res = res.clone().unwrap_or_else(|e| {
                panic!(
                    "Transaction {} must be valid, failed with {:#?}",
                    receipt.transaction_hash, e
                )
            });
            if receipt.status == Some(alloy::primitives::U256::ONE) {
                assert!(
                    res.is_success(),
                    "Transaction {} should have succeeded",
                    receipt.transaction_index
                );
            } else if receipt.status == Some(alloy::primitives::U256::ZERO) {
                assert!(
                    !res.is_success(),
                    "Transaction {} should have failed",
                    receipt.transaction_index
                )
            }
            let gas_difference =    zk_ee::utils::u256_to_u64_saturated(&receipt.gas_used).abs_diff(res.gas_used);
            // Check gas used
            if res.gas_used != zk_ee::utils::u256_to_u64_saturated(&receipt.gas_used) {
                println!(
                    "Transaction {} has a gas mismatch: ZKsync OS used {}, reference: {}\n  Difference:{}",
                    receipt.transaction_index, res.gas_used, receipt.gas_used,
                    gas_difference,
                );
                assert!(consistent_with_refund(res.gas_used, gas_difference), "Gas difference should be consistent with refund")
            }
            // Logs check
            assert_eq!(
                res.logs.len(),
                receipt.logs.len(),
                "Transaction {} has mismatch in number of logs",
                receipt.transaction_index
            );
            assert!(res.logs.iter().zip(receipt.logs.iter()).all(|(l, r)| {
                let eq = r.is_equal_to_excluding_data(l);
                if !eq {
                    println!("Not equal logs:\n {:#?} \nand\n {:?}", l, r)
                }
                if r.data.to_vec() != l.data {
                    // We allow data to be different, as it can sometimes depend on
                    // gas, which is not 100% equivalent (access lists)
                    println!(
                        "Data is not equal: we got {}, expected {}",
                        hex::encode(l.data.clone()),
                        hex::encode(r.data.clone())
                    );
                }

                eq
            }))
        });

    diff_trace.check_storage_writes(output, prestate_cache, miner);

    println!("All good!")
}
