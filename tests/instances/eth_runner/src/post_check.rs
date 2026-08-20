use crate::prestate::*;
use crate::receipts::TransactionReceipt;
use alloy::consensus::{Eip658Value, Receipt, ReceiptWithBloom};
use alloy::hex;
use alloy::primitives::{Bloom, Log};
use alloy::rlp::Encodable as _;
use forward_system::run::output::BlockOutput;
use rig::basic_bootloader::bootloader::block_flow::zk::zk_block_tx_tree_root_in_place;
use rig::crypto::{blake2s::Blake2s256, MiniDigest};
use rig::forward_system::run::convert_alloy::FromAlloy;
use rig::log::{error, info};
use ruint::aliases::{B160, B256, U256};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use zk_ee::utils::Bytes32;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[allow(dead_code)]
pub enum PostCheckError {
    InvalidTx { id: TxId, msg: String },
    TxShouldHaveFailed { id: TxId },
    IncorrectLogs { id: TxId },
    GasMismatch { id: TxId },
    BadTransactionsRoot,
    BadReceiptsRoot,
    Internal { msg: String },
}

macro_rules! error_internal {
    ($($arg:tt)*) => {{
        let __msg = format!($($arg)*);
        error!("{}", __msg);
        return Err(PostCheckError::Internal { msg: __msg });
    }};
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[allow(dead_code)]
pub enum TxId {
    Hash(String),
    Index(usize),
}

/// Account code as a byte slice, treating `None` and an empty `Bytes`
/// identically: the ZKsync OS side leaves an account with no code as `None`
/// while the reference/on-chain side can carry an explicit empty `Bytes`; both
/// mean "no code" and must compare equal.
fn code_as_slice(code: &Option<alloy::primitives::Bytes>) -> &[u8] {
    code.as_ref().map(|c| c.as_ref()).unwrap_or(&[])
}

impl DiffTrace {
    fn collect_diffs(self, prestate_cache: &Cache) -> HashMap<B160, AccountState> {
        let mut updates: HashMap<B160, (Option<usize>, AccountState)> = HashMap::new();
        self.result.iter().enumerate().for_each(|(idx, item)| {
            item.result.post.iter().for_each(|(address, account)| {
                let (last_updated_at, entry) = updates.entry(address.0).or_default();
                *last_updated_at = Some(idx);
                account
                    .balance
                    .into_iter()
                    .for_each(|bal| entry.balance = Some(bal));
                account
                    .nonce
                    .into_iter()
                    .for_each(|x| entry.nonce = Some(x));
                // A code set (contract deploy or EIP-7702 delegation) is
                // reported in `post`. A code CLEAR (e.g. an EIP-7702 delegation
                // removed within the block) is reported by the tracer OMITTING
                // `code` from `post` — but `post` ALSO omits `code` when it is
                // simply UNCHANGED (e.g. a delegated EOA just sends a tx), so
                // "code absent in post" is ambiguous and cannot be resolved from
                // the per-tx trace alone. We therefore aggregate only explicit
                // code sets here; a possibly-stale delegation is reconciled
                // against the authoritative on-chain code in `check_storage_writes`.
                account
                    .code
                    .clone()
                    .into_iter()
                    .for_each(|x| entry.code = Some(x));

                // Populate storage slot clears (slots present in pre but
                // absent in post). Write 0 to them.
                if let Some(pre_account) = item.result.pre.get(address) {
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
                if let Some(storage) = account.storage.as_ref() {
                    let entry_storage = entry.storage.get_or_insert_default();
                    storage.iter().for_each(|(key, value)| {
                        entry_storage.insert(*key, *value);
                    })
                }
            });
            // Add account clears
            item.result.pre.iter().for_each(|(address, _)| {
                // We consider a selfdestruct either when an account is in "pre" but never
                // updated, or if it in pre for transaction after its last update.
                if !updates.contains_key(&address.0)
                    || updates[&address.0]
                        .0
                        .is_some_and(|last_update| last_update < idx)
                {
                    let acc = AccountState {
                        balance: Some(U256::ZERO),
                        ..Default::default()
                    };
                    updates.insert(address.0, (Some(idx), acc));
                }
            })
        });

        // Filter out empty diffs
        // These can be empty because their value is the same as in the initial tree
        // or the post state was empty. Note that if the account was selfdestructed,
        // the address shouldn't be present in the post state. This is just a strange
        // case where the logs add an empty entry for accounts that haven't been
        // modified.

        updates.retain(|address, (_, account)| {
            if let Some(storage) = account.storage.as_mut() {
                storage.retain(|key, new_val| match prestate_cache.get_slot(address, key) {
                    None => *new_val != B256::ZERO,
                    Some(initial) => *new_val != initial,
                })
            }
            if account.storage.as_ref().is_some_and(|s| s.is_empty()) {
                account.storage = None
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

        updates.into_iter().map(|(k, (_, v))| (k, v)).collect()
    }

    pub fn check_storage_writes(
        self,
        output: BlockOutput,
        prestate_cache: Cache,
        endpoint: Option<&str>,
        block_number: u64,
        parent_block_hash: Option<[u8; 32]>,
    ) -> Result<(), PostCheckError> {
        let diffs = self.collect_diffs(&prestate_cache);
        let zksync_os_diffs = zksync_os_output_into_account_state(output, &prestate_cache)?;

        // Reference => ZKsync OS check:
        for (address, account) in diffs.iter() {
            let zk_account = match zksync_os_diffs.get(address) {
                Some(v) => v,
                None => {
                    error_internal!(
                        "ZKsync OS must have write for account {} {:?}",
                        hex::encode(address.to_be_bytes_vec()),
                        account
                    )
                }
            };
            if let Some(bal) = account.balance {
                if Some(bal) != zk_account.balance {
                    error_internal!(
                        "Balance for {} is {:?} but expected {:?}.\n  Difference: {:?}",
                        hex::encode(address.to_be_bytes_vec()),
                        zk_account.balance,
                        bal,
                        zk_account.balance.unwrap_or(U256::ZERO).abs_diff(bal),
                    )
                };
            }
            if let Some(nonce) = account.nonce {
                if nonce != zk_account.nonce.unwrap() {
                    error_internal!(
                        "Nonce for address {} differed. ZKsync OS: {:?}, reference: {:?}",
                        hex::encode(address.to_be_bytes_vec()),
                        zk_account.nonce.unwrap(),
                        nonce
                    )
                }
            }
            // Compare code content treating "no code" uniformly (`None` on the
            // ZKsync OS side vs an empty `Bytes` reference both mean empty).
            //
            // The trace-reconstructed reference code can be a STALE EIP-7702
            // delegation: the per-tx diff omits `code` from `post` both when a
            // delegation is cleared and when it is left unchanged, so a
            // delegation set earlier in the block and cleared later (or replaced)
            // leaves an intermediate designator in `account.code`. On a mismatch
            // we therefore reconcile against the authoritative on-chain code via
            // `eth_getCode`: if ZKsync OS matches the real chain, the reference
            // was merely ambiguous and this is not a divergence; otherwise it is
            // a genuine divergence and we report it.
            if account.code.is_some()
                && code_as_slice(&account.code) != code_as_slice(&zk_account.code)
            {
                let address_hex = hex::encode(address.to_be_bytes_vec());
                let on_chain_code = match endpoint {
                    Some(ep) => match crate::live_run::rpc::get_code(
                        ep,
                        &format!("0x{address_hex}"),
                        block_number,
                    ) {
                        Ok(code) => Some(code),
                        Err(e) => error_internal!(
                            "Failed to resolve on-chain code for address {address_hex} at block {block_number}: {e}"
                        ),
                    },
                    None => None,
                };
                match &on_chain_code {
                    // ZKsync OS matches the real chain; the trace-derived
                    // reference was an ambiguous EIP-7702 delegation, not a
                    // divergence.
                    Some(code) if code_as_slice(&zk_account.code) == code.as_ref() => {}
                    Some(code) => error_internal!(
                        "Code for address {} diverges from on-chain state. ZKsync OS: {}, on-chain: {}",
                        address_hex,
                        hex::encode(zk_account.code.as_ref().unwrap_or_default()),
                        hex::encode(code)
                    ),
                    // No endpoint to reconcile against (e.g. single-run from
                    // fixtures): fall back to the trace-derived reference.
                    None => error_internal!(
                        "Code for address {} differed. ZKsync OS: {}, reference: {}",
                        address_hex,
                        hex::encode(zk_account.code.as_ref().unwrap_or_default()),
                        hex::encode(account.code.as_ref().unwrap_or_default())
                    ),
                }
            }
            if let Some(storage) = &account.storage {
                for (key, value) in storage {
                    let zksync_os_value = match zk_account.storage.as_ref().unwrap().get(key) {
                        Some(v) => v,
                        None => {
                            error_internal!(
                                "Should have value for slot {} at address {}",
                                key,
                                hex::encode(address.to_be_bytes_vec())
                            )
                        }
                    };
                    if value != zksync_os_value {
                        error_internal!(
                          "Value for slot {} at address {} differed. ZKsync OS: {:?}, reference: {:?}",
                          key,
                          hex::encode(address.to_be_bytes_vec()),
                          zksync_os_value, value)
                    }
                }

                for (k, v) in zk_account.storage.as_ref().unwrap().iter() {
                    // In the diff trace, slot clearing is not present in post,
                    // so we have to allow the case when v == 0.
                    if !(v.as_uint().is_zero() || storage.contains_key(k)) {
                        error_internal!("Key {k:?} for {address:?} not present in reference")
                    }
                }
            }
        }

        // ZKsync OS => reference
        for (address, acc) in zksync_os_diffs.iter() {
            // Just check that it's part of the reference diffs,
            // all else should be checked already
            if !acc.is_empty() {
                match diffs.get(address) {
                    Some(_) => (),
                    None => {
                        // The per-tx `prestateTracer` diff does not report two
                        // kinds of legitimate ZKsync OS writes, so an unmatched
                        // ZKsync OS diff is not necessarily a divergence:
                        //  * selfdestruct — not reported in the traces at all; we
                        //    check the ZKsync OS diff is consistent with one.
                        //  * the EIP-2935 pre-block system call — a block-boundary
                        //    write to the history-storage contract that no per-tx
                        //    diff can contain.
                        if !zksync_os_diff_consistent_with_selfdestruct(
                            address,
                            acc,
                            &prestate_cache,
                        ) && !zksync_os_diff_is_eip2935_system_write(
                            address,
                            acc,
                            block_number,
                            parent_block_hash,
                        ) {
                            error_internal!(
                                "Reference must have write for account {} {:?}",
                                hex::encode(address.to_be_bytes_vec()),
                                acc
                            )
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

fn zksync_os_diff_consistent_with_selfdestruct(
    address: &B160,
    acc: &AccountState,
    prestate_cache: &Cache,
) -> bool {
    let diff_is_empty = acc.balance.is_none_or(|b| b.is_zero())
        && acc.nonce.is_none_or(|n| n == 0)
        && acc.code.as_ref().is_none_or(|c| c.is_empty())
        && acc.storage.as_ref().is_none_or(|s| s.is_empty());
    let pre = prestate_cache.0.get(address);
    let prestate_can_be_deployed = || {
        pre.is_none_or(|pre| {
            pre.storage.as_ref().is_none_or(|s| s.is_empty())
                && pre.code.as_ref().is_none_or(|c| c.is_empty())
                && pre.nonce.is_none_or(|n| n == 0)
        })
    };
    diff_is_empty && prestate_can_be_deployed()
}

/// EIP-2935 history-storage contract and ring-buffer size, mirroring
/// `basic_bootloader::bootloader::block_flow::eip_2935_historical_block_hash`.
const HISTORY_STORAGE_ADDRESS: B160 =
    B160::from_limbs([0x335B175320002935, 0x27F1C53A10CB7A02, 0x0000F908]);
const HISTORY_SERVE_WINDOW: u64 = 8191;

/// Returns true if `acc` is exactly the EIP-2935 pre-block system write the STF
/// performs at the start of every post-Pectra block: a single storage write to
/// `HISTORY_STORAGE_ADDRESS` at slot `(block_number - 1) % HISTORY_SERVE_WINDOW`
/// holding the parent block hash, with no other account change.
///
/// This write is a block-boundary *system call*, so it never appears in the
/// per-tx `prestateTracer` diff the reference is rebuilt from — leaving a
/// ZKsync OS storage diff with no matching reference entry that would otherwise
/// trip the "reference must have write" check. When the parent hash is known
/// (`expected_parent_hash`, from the block-hash oracle the harness already
/// fetched), the written value is validated against it so a genuinely wrong
/// value or slot is still reported; otherwise only the address and slot are
/// checked.
fn zksync_os_diff_is_eip2935_system_write(
    address: &B160,
    acc: &AccountState,
    block_number: u64,
    expected_parent_hash: Option<[u8; 32]>,
) -> bool {
    if *address != HISTORY_STORAGE_ADDRESS || block_number == 0 {
        return false;
    }
    // Only a single storage write is expected — nothing else changes.
    if acc.balance.is_some() || acc.nonce.is_some() || acc.code.is_some() {
        return false;
    }
    let Some(storage) = acc.storage.as_ref() else {
        return false;
    };
    if storage.len() != 1 {
        return false;
    }
    let expected_slot = U256::from((block_number - 1) % HISTORY_SERVE_WINDOW);
    let Some(value) = storage.get(&expected_slot) else {
        return false;
    };
    match expected_parent_hash {
        Some(parent) => value.to_be_bytes::<32>() == parent,
        None => true,
    }
}

fn zksync_os_output_into_account_state(
    output: BlockOutput,
    prestate_cache: &Cache,
) -> Result<HashMap<B160, AccountState>, PostCheckError> {
    use basic_system::system_implementation::flat_storage_model::AccountProperties;
    let mut updates: HashMap<B160, AccountState> = HashMap::new();
    let preimages: HashMap<[u8; 32], Vec<u8>> = HashMap::from_iter(
        output
            .published_preimages
            .into_iter()
            .map(|(key, value)| (key.0, value)),
    );
    for w in output.storage_writes {
        if rig::chain::is_account_properties_address(&B160::from_alloy(w.account)) {
            // populate account
            let address: [u8; 20] = w.account_key.as_slice()[12..].try_into().unwrap();
            let address = B160::from_be_bytes(address);
            let props = if w.value.is_zero() {
                // TODO: Account deleted, we need to check this somehow
                AccountProperties::default()
            } else {
                let encoded = match preimages.get(w.value.as_slice()) {
                    Some(x) => x.clone(),
                    None => {
                        error_internal!("Must contain preimage for account {address:#?}")
                    }
                };
                AccountProperties::decode(&encoded.try_into().unwrap())
            };
            let entry = updates.entry(address).or_default();
            entry.balance = Some(props.balance);
            entry.nonce = Some(props.nonce);
            if let Some(bytecode) = preimages.get(&props.bytecode_hash.as_u8_array()) {
                let owned: Vec<u8> = bytecode[..props.observable_bytecode_len as usize].to_owned();
                entry.code = Some(owned.into());
            }
        } else {
            // populate slot
            let address = w.account;
            let key = U256::from_be_bytes(w.account_key.0);
            let entry = updates.entry(B160::from_alloy(address)).or_default();
            let value = B256::from_be_bytes(w.value.0);
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
        if account.storage.as_ref().is_some_and(|s| s.is_empty()) {
            account.storage = None
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

    Ok(updates)
}

/// Reproduces the header `transactions_root`: a Blake2s simple Merkle tree over
/// the block's transaction hashes (in execution order), matching the ZKsync OS
/// `block_data` scheme.
fn compute_transactions_root_for_receipts(receipts: &[TransactionReceipt]) -> [u8; 32] {
    let mut leaves: Vec<Bytes32> = receipts
        .iter()
        .map(|receipt| Bytes32::from_array(receipt.transaction_hash.0))
        .collect();
    zk_block_tx_tree_root_in_place(&mut leaves).as_u8_array()
}

/// Reproduces a single ZK receipt-hash leaf independently of ZKsync OS:
/// `blake2s(type? || rlp([status, cumulative_gas_used, logs_bloom, [logs...]]))`,
/// matching `compute_receipt_hash`. The ZK path commits to a **zero** logs bloom
/// (the bloom is recomputable from the logs and would be wasted prover work), so
/// the leaf is built with a zero bloom here too. Status, cumulative gas and the
/// logs are taken from the reference receipt, giving a check that is independent
/// of the per-tx data ZKsync OS used to build the header.
fn compute_receipt_leaf(receipt: &TransactionReceipt) -> Bytes32 {
    let status = receipt.status == Some(U256::from(1u64));
    let cumulative_gas_used = zk_ee::utils::u256_to_u64_saturated(&receipt.cumulative_gas_used);
    // ZK/Ethereum-specific type bytes (e.g. 0x7e) all fit in one byte.
    let tx_type = receipt
        .tx_type
        .map_or(0u8, |t| zk_ee::utils::u256_to_u64_saturated(&t) as u8);

    let logs: Vec<Log> = receipt
        .logs
        .iter()
        .map(|l| Log::new_unchecked(l.address, l.topics.clone(), l.data.clone()))
        .collect();

    let receipt_with_bloom = ReceiptWithBloom {
        receipt: Receipt {
            status: Eip658Value::Eip658(status),
            cumulative_gas_used,
            logs,
        },
        logs_bloom: Bloom::ZERO,
    };

    let mut rlp = Vec::new();
    if tx_type != 0 {
        rlp.push(tx_type);
    }
    receipt_with_bloom.encode(&mut rlp);

    let mut hasher = Blake2s256::new();
    hasher.update(&rlp);
    Bytes32::from_array(hasher.finalize())
}

/// Reproduces the header `receipts_root`: a Blake2s simple Merkle tree over the
/// block's receipt-hash leaves (in execution order), matching the ZKsync OS
/// `block_data` scheme. Independent of ZKsync OS' own receipt encoding.
fn compute_receipts_root_for_receipts(receipts: &[TransactionReceipt]) -> [u8; 32] {
    let mut leaves: Vec<Bytes32> = receipts.iter().map(compute_receipt_leaf).collect();
    zk_block_tx_tree_root_in_place(&mut leaves).as_u8_array()
}

#[allow(clippy::too_many_arguments)]
pub fn post_check(
    output: BlockOutput,
    receipts: Vec<TransactionReceipt>,
    diff_trace: DiffTrace,
    prestate_cache: Cache,
    // RPC endpoint used to reconcile ambiguous EIP-7702 delegation code against
    // the authoritative on-chain state. `None` (e.g. single-run from local
    // fixtures) falls back to the trace-derived reference.
    endpoint: Option<&str>,
    block_number: u64,
    // Parent block hash (from the block-hash oracle the harness already fetched),
    // used to validate the EIP-2935 pre-block history-storage write that the per-tx
    // trace cannot express. `None` skips value validation of that write.
    parent_block_hash: Option<[u8; 32]>,
) -> Result<(), PostCheckError> {
    fn u256_to_usize(src: &U256) -> usize {
        zk_ee::utils::u256_to_u64_saturated(src) as usize
    }

    let reference_transactions_root = compute_transactions_root_for_receipts(&receipts);
    let zksync_os_transactions_root = output.header.inner().transactions_root.0;
    if reference_transactions_root != zksync_os_transactions_root {
        error!(
            "Transactions root mismatch, reference {}, got {}",
            hex::encode(reference_transactions_root),
            hex::encode(zksync_os_transactions_root)
        );
        return Err(PostCheckError::BadTransactionsRoot);
    }

    let reference_receipts_root = compute_receipts_root_for_receipts(&receipts);
    let zksync_os_receipts_root = output.header.inner().receipts_root.0;
    if reference_receipts_root != zksync_os_receipts_root {
        error!(
            "Receipts root mismatch, reference {}, got {}",
            hex::encode(reference_receipts_root),
            hex::encode(zksync_os_receipts_root)
        );
        return Err(PostCheckError::BadReceiptsRoot);
    }

    for (res, receipt) in output.tx_results.iter().zip(receipts.iter()) {
        let res = match res {
            Ok(res) => res,
            Err(e) => {
                error!(
                    "Transaction {} must be valid, failed with {:#?}",
                    receipt.transaction_hash, e
                );
                return Err(PostCheckError::InvalidTx {
                    id: TxId::Hash(receipt.transaction_hash.to_string()),
                    msg: format!("{e:#?}"),
                });
            }
        };
        if receipt.status == Some(alloy::primitives::U256::ONE) {
            if !res.is_success() {
                error!(
                    "Transaction {} should have succeeded",
                    receipt.transaction_index
                );
                return Err(PostCheckError::InvalidTx {
                    id: TxId::Index(u256_to_usize(&receipt.transaction_index)),
                    msg: "Should have succeeded".to_string(),
                });
            };
        } else if receipt.status == Some(alloy::primitives::U256::ZERO) && res.is_success() {
            error!(
                "Transaction {} should have failed",
                receipt.transaction_index
            );
            return Err(PostCheckError::TxShouldHaveFailed {
                id: TxId::Index(u256_to_usize(&receipt.transaction_index)),
            });
        }
        let gas_difference =
            zk_ee::utils::u256_to_u64_saturated(&receipt.gas_used).abs_diff(res.gas_used);
        // Check gas used
        if res.gas_used != zk_ee::utils::u256_to_u64_saturated(&receipt.gas_used) {
            error!(
                    "Transaction {} has a gas mismatch: ZKsync OS used {}, reference: {}\n  Difference:{}",
                    receipt.transaction_index, res.gas_used, receipt.gas_used,
                    gas_difference,
                );
            return Err(PostCheckError::GasMismatch {
                id: TxId::Index(u256_to_usize(&receipt.transaction_index)),
            });
        }
        // Logs check
        if res.logs.len() != receipt.logs.len() {
            error!(
                "Transaction {} has mismatch in number of logs",
                receipt.transaction_index
            );
            return Err(PostCheckError::IncorrectLogs {
                id: TxId::Index(u256_to_usize(&receipt.transaction_index)),
            });
        }
        for (l, r) in res.logs.iter().zip(receipt.logs.iter()) {
            let eq = r.is_equal_to_excluding_data(l);
            if !eq {
                error!("Not equal logs:\n {l:#?} \nand\n {r:?}");
                return Err(PostCheckError::IncorrectLogs {
                    id: TxId::Index(u256_to_usize(&receipt.transaction_index)),
                });
            }
            if r.data.to_vec() != l.data.data {
                error!(
                    "Data is not equal: we got {}, expected {}",
                    hex::encode(l.data.data.clone()),
                    hex::encode(r.data.clone())
                );
                return Err(PostCheckError::IncorrectLogs {
                    id: TxId::Index(u256_to_usize(&receipt.transaction_index)),
                });
            }
        }
    }

    diff_trace.check_storage_writes(
        output,
        prestate_cache,
        endpoint,
        block_number,
        parent_block_hash,
    )?;

    info!("All good!");
    Ok(())
}
