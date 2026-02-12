use alloy::primitives::{address, Address, B256, U256};
use reth_revm::{bytecode::Bytecode, db::CacheDB, DatabaseRef};
use std::collections::{HashMap, HashSet};
use zksync_os_interface::types::{AccountDiff, StorageWrite};

use crate::bytecode_hash::{calculate_bytecode_hash, EMPTY_BYTE_CODE_HASH};

const ACCOUNT_PROPERTIES_STORAGE_ADDRESS: Address =
    address!("0000000000000000000000000000000000008003");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AccountSnap {
    nonce: u64,
    balance: U256,
    bytecode_hash: B256,
}

/// Storage mismatch between ZKsync OS and REVM block execution
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StorageMismatch {
    pub addr: Address,
    pub slot: B256,
    // None indicates no storage update occurred in REVM
    pub revm_value: Option<B256>,
    // None indicates no storage update occurred in ZKsync OS
    pub zk_value: Option<B256>,
}

/// Generic pair of optional values (REVM / ZKsync OS) for a field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValuePair<T> {
    pub revm: Option<T>,
    pub zk: Option<T>,
}

/// All account discrepancies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountMismatch {
    pub addr: Address,
    pub nonce: Option<ValuePair<u64>>,
    pub balance: Option<ValuePair<U256>>,
    pub bytecode_hash: Option<ValuePair<B256>>,
}

/// Full comparison result.
#[derive(Debug, Default)]
pub struct CompareReport {
    pub storage: Vec<StorageMismatch>,
    pub accounts: Vec<AccountMismatch>,
}

impl CompareReport {
    pub fn build<DB>(
        cache_db: &CacheDB<DB>,
        zksync_storage_writes: &[StorageWrite],
        zksync_account_diffs: &[AccountDiff],
    ) -> Result<CompareReport, anyhow::Error>
    where
        DB: DatabaseRef,
        DB::Error: std::error::Error + Send + Sync + 'static,
    {
        // internal maps keyed by (addr, slot)
        let revm_storage = build_revm_storage_map(cache_db)?;
        let zk_storage = build_zk_storage_map(zksync_storage_writes);

        let revm_accounts = build_revm_accounts(cache_db)?;
        let zk_accounts = build_zk_accounts(zksync_account_diffs);

        let storage_report = compare_storage(&revm_storage, &zk_storage);
        let account_report = compare_accounts(&revm_accounts, &zk_accounts);

        Ok(CompareReport {
            storage: storage_report,
            accounts: account_report,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.storage.is_empty() && self.accounts.is_empty()
    }

    /// Print a structured summary via `tracing`
    /// - INFO when everything matches
    /// - WARN + INFO details when mismatches exist
    pub fn log_tracing(&self, _max_show: usize) {
        // TODO: implement logging
        /*
        if self.is_empty() {
            tracing::info!(
                storage_mismatches = 0,
                account_mismatches = 0,
                "State diffs match"
            );
            return;
        }

        tracing::warn!(
            storage_mismatches = self.storage.len(),
            account_mismatches = self.accounts.len(),
            "State diffs do not match"
        );

        // STORAGE
        tracing::info!(total = self.storage.len(), "=== STORAGE DIFFS ===");
        for m in self.storage.iter().take(max_show) {
            match (m.revm_value, m.zk_value) {
                (Some(r), Some(z)) if r != z => {
                    tracing::info!(
                        addr = ?m.addr,
                        slot = ?m.slot,
                        revm = ?r,
                        zk = ?z,
                        "storage value mismatch"
                    );
                }
                (Some(r), None) => {
                    tracing::info!(
                        addr = ?m.addr,
                        slot = ?m.slot,
                        revm = ?r,
                        zk = "none",
                        "storage missing in zksync"
                    );
                }
                (None, Some(z)) => {
                    tracing::info!(
                        addr = ?m.addr,
                        slot = ?m.slot,
                        revm = "none",
                        zk = ?z,
                        "storage missing in revm"
                    );
                }
                _ => {}
            }
        }
        if self.storage.len() > max_show {
            tracing::info!(
                remaining = self.storage.len() - max_show,
                "additional storage mismatches not shown"
            );
        }

        // ACCOUNTS
        tracing::info!(total = self.accounts.len(), "=== ACCOUNT DIFFS ===");
        for m in self.accounts.iter().take(max_show) {
            // Header per account
            tracing::info!(addr = ?m.addr, "account mismatch");

            if let Some(p) = m.nonce {
                match (p.revm, p.zk) {
                    (Some(r), Some(z)) if r != z => {
                        tracing::info!(addr = ?m.addr, revm = r, zk = z, "nonce mismatch");
                    }
                    (Some(r), None) => {
                        tracing::info!(addr = ?m.addr, revm = r, zk = "none", "nonce missing in zksync");
                    }
                    (None, Some(z)) => {
                        tracing::info!(addr = ?m.addr, revm = "none", zk = z, "nonce missing in revm");
                    }
                    _ => {}
                }
            }
            if let Some(p) = m.balance {
                match (p.revm, p.zk) {
                    (Some(r), Some(z)) if r != z => {
                        tracing::info!(addr = ?m.addr, revm = ?r, zk = ?z, "balance mismatch");
                    }
                    (Some(r), None) => {
                        tracing::info!(addr = ?m.addr, revm = ?r, zk = "none", "balance missing in zksync");
                    }
                    (None, Some(z)) => {
                        tracing::info!(addr = ?m.addr, revm = "none", zk = ?z, "balance missing in revm");
                    }
                    _ => {}
                }
            }
            if let Some(p) = m.bytecode_hash {
                match (p.revm, p.zk) {
                    (Some(r), Some(z)) if !code_hash_equivalent(r, z) => {
                        tracing::info!(addr = ?m.addr, revm = ?r, zk = ?z, "bytecode hash mismatch");
                    }
                    (Some(r), None) => {
                        tracing::info!(addr = ?m.addr, revm = ?r, zk = "none", "codehash missing in zksync");
                    }
                    (None, Some(z)) => {
                        tracing::info!(addr = ?m.addr, revm = "none", zk = ?z, "codehash missing in revm");
                    }
                    _ => {}
                }
            }
        }
        if self.accounts.len() > max_show {
            tracing::info!(
                remaining = self.accounts.len() - max_show,
                "additional account mismatches not shown"
            );
        }

        */
    }
}

fn build_revm_storage_map<DB>(
    cache_db: &CacheDB<DB>,
) -> Result<HashMap<(Address, B256), B256>, anyhow::Error>
where
    DB: DatabaseRef,
    DB::Error: std::error::Error + Send + Sync + 'static,
{
    let mut map = HashMap::new();

    for (addr, account) in &cache_db.cache.accounts {
        if *addr == ACCOUNT_PROPERTIES_STORAGE_ADDRESS {
            continue;
        }
        for (slot_key, slot_val) in &account.storage {
            let prev = cache_db.db.storage_ref(*addr, *slot_key)?;
            if prev != *slot_val {
                map.insert((*addr, B256::from(*slot_key)), B256::from(*slot_val));
            }
        }
    }
    Ok(map)
}

fn build_zk_storage_map(zksync_storage_writes: &[StorageWrite]) -> HashMap<(Address, B256), B256> {
    let mut map = HashMap::new();
    for w in zksync_storage_writes {
        if w.account == ACCOUNT_PROPERTIES_STORAGE_ADDRESS {
            continue;
        }
        map.insert((w.account, w.account_key), w.value); // latest write wins
    }
    map
}

fn build_revm_accounts<DB>(
    cache_db: &CacheDB<DB>,
) -> Result<HashMap<Address, AccountSnap>, anyhow::Error>
where
    DB: DatabaseRef,
    DB::Error: std::error::Error + Send + Sync + 'static,
{
    let mut map = HashMap::new();

    for (addr, acc) in &cache_db.cache.accounts {
        let bytecode_hash = if let Some(code) = acc.info.code.as_ref() {
            if code.is_empty() {
                B256::ZERO
            } else {
                match code {
                    Bytecode::LegacyAnalyzed(legacy_code) => calculate_bytecode_hash(legacy_code),
                    _ => {
                        return Err(anyhow::anyhow!(
                            "EIP-7702 bytecode is not supported on Consistency Checker"
                        ));
                    }
                }
            }
        } else {
            B256::ZERO
        };

        let prev_account = cache_db.db.basic_ref(*addr)?.unwrap_or_default();
        let changed = prev_account.nonce != acc.info.nonce
            || prev_account.balance != acc.info.balance
            || prev_account.code_hash != acc.info.code_hash;
        if changed {
            map.insert(
                *addr,
                AccountSnap {
                    nonce: acc.info.nonce,
                    balance: acc.info.balance,
                    bytecode_hash,
                },
            );
        }
    }
    Ok(map)
}

fn build_zk_accounts(zksync_account_diffs: &[AccountDiff]) -> HashMap<Address, AccountSnap> {
    let mut map = HashMap::new();
    for d in zksync_account_diffs {
        map.insert(
            d.address,
            AccountSnap {
                nonce: d.nonce,
                balance: d.balance,
                bytecode_hash: d.bytecode_hash,
            },
        );
    }
    map
}

fn compare_storage(
    revm: &HashMap<(Address, B256), B256>,
    zk: &HashMap<(Address, B256), B256>,
) -> Vec<StorageMismatch> {
    let mut mismatches = Vec::new();

    // Keys present in REVM (diff or missing in ZK)
    for (&(addr, slot), &revm_v) in revm {
        match zk.get(&(addr, slot)) {
            Some(&zk_v) if zk_v != revm_v => mismatches.push(StorageMismatch {
                addr,
                slot,
                revm_value: Some(revm_v),
                zk_value: Some(zk_v),
            }),
            None => mismatches.push(StorageMismatch {
                addr,
                slot,
                revm_value: Some(revm_v),
                zk_value: None,
            }),
            _ => {}
        }
    }

    // Keys present in ZKsync OS but missing in REVM
    for (&(addr, slot), &zk_v) in zk {
        if !revm.contains_key(&(addr, slot)) {
            mismatches.push(StorageMismatch {
                addr,
                slot,
                revm_value: None,
                zk_value: Some(zk_v),
            });
        }
    }

    mismatches
}

fn compare_accounts(
    revm: &HashMap<Address, AccountSnap>,
    zk: &HashMap<Address, AccountSnap>,
) -> Vec<AccountMismatch> {
    let mut mismatches = Vec::new();

    // Iterate over the union of addresses
    let mut all = HashSet::new();
    all.extend(revm.keys().copied());
    all.extend(zk.keys().copied());

    for addr in all {
        match (revm.get(&addr), zk.get(&addr)) {
            (Some(r), Some(z)) => {
                // Only emit fields that actually differ
                let mut any = false;
                let mut nonce = None;
                let mut balance = None;
                let mut bytecode_hash = None;

                if r.nonce != z.nonce {
                    nonce = Some(ValuePair {
                        revm: Some(r.nonce),
                        zk: Some(z.nonce),
                    });
                    any = true;
                }
                if r.balance != z.balance {
                    balance = Some(ValuePair {
                        revm: Some(r.balance),
                        zk: Some(z.balance),
                    });
                    any = true;
                }
                if !code_hash_equivalent(r.bytecode_hash, z.bytecode_hash) {
                    bytecode_hash = Some(ValuePair {
                        revm: Some(r.bytecode_hash),
                        zk: Some(z.bytecode_hash),
                    });
                    any = true;
                }

                if any {
                    mismatches.push(AccountMismatch {
                        addr,
                        nonce,
                        balance,
                        bytecode_hash,
                    });
                }
            }
            (Some(r), None) => {
                // Present only in REVM: emit full snapshot with zk=None
                mismatches.push(AccountMismatch {
                    addr,
                    nonce: Some(ValuePair {
                        revm: Some(r.nonce),
                        zk: None,
                    }),
                    balance: Some(ValuePair {
                        revm: Some(r.balance),
                        zk: None,
                    }),
                    bytecode_hash: Some(ValuePair {
                        revm: Some(r.bytecode_hash),
                        zk: None,
                    }),
                });
            }
            (None, Some(z)) => {
                // Present only in ZK: emit full snapshot with revm=None
                mismatches.push(AccountMismatch {
                    addr,
                    nonce: Some(ValuePair {
                        revm: None,
                        zk: Some(z.nonce),
                    }),
                    balance: Some(ValuePair {
                        revm: None,
                        zk: Some(z.balance),
                    }),
                    bytecode_hash: Some(ValuePair {
                        revm: None,
                        zk: Some(z.bytecode_hash),
                    }),
                });
            }
            (None, None) => unreachable!("address in union but missing from both maps"),
        }
    }

    mismatches
}

#[inline]
fn code_hash_equivalent(a: B256, b: B256) -> bool {
    a == b
        || (a == EMPTY_BYTE_CODE_HASH && b == B256::ZERO)
        || (a == B256::ZERO && b == EMPTY_BYTE_CODE_HASH)
}
