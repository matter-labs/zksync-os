use rig::Chain;
use ruint::{
    aliases::{B160, B256, U256},
    Bits,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[repr(transparent)]
#[derive(PartialEq, Eq, Clone, Copy, Debug, Deserialize, Serialize)]
pub struct BitsOrd<const BITS: usize, const LIMBS: usize>(pub Bits<BITS, LIMBS>);

#[allow(clippy::non_canonical_partial_ord_impl)]
impl<const BITS: usize, const LIMBS: usize> PartialOrd for BitsOrd<BITS, LIMBS> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.0.as_limbs().partial_cmp(other.0.as_limbs())
    }
}

impl<const BITS: usize, const LIMBS: usize> Ord for BitsOrd<BITS, LIMBS> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.0.as_limbs().cmp(other.0.as_limbs())
    }
}

impl<const BITS: usize, const LIMBS: usize> From<Bits<BITS, LIMBS>> for BitsOrd<BITS, LIMBS> {
    fn from(value: Bits<BITS, LIMBS>) -> Self {
        Self(value)
    }
}

impl<const BITS: usize, const LIMBS: usize> From<&Bits<BITS, LIMBS>> for &BitsOrd<BITS, LIMBS> {
    fn from(value: &Bits<BITS, LIMBS>) -> Self {
        unsafe { &*(value as *const _ as *const _) }
    }
}

pub type BitsOrd160 = BitsOrd<{ B160::BITS }, { B160::LIMBS }>;

#[derive(Debug, Deserialize, Clone)]
pub struct PrestateTrace {
    pub result: Vec<PrestateItem>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PrestateItem {
    pub result: BTreeMap<BitsOrd160, AccountState>,
}

// Note: we need both prestate and diff traces, as the diff trace "pre"
// section doesn't include all touched slots, only non-zero ones.
// This means that we cannot construct an initial state only from
// the pre side of the diff trace.

#[derive(Debug, Deserialize, Clone)]
pub struct DiffTrace {
    pub result: Vec<DiffItem>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DiffItem {
    pub result: StateDiff,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Clone)]
pub struct StateDiff {
    pub pre: BTreeMap<BitsOrd160, AccountState>,
    pub post: BTreeMap<BitsOrd160, AccountState>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct AccountState {
    pub balance: Option<U256>,
    pub nonce: Option<u64>,
    pub code: Option<alloy::primitives::Bytes>,
    pub storage: Option<BTreeMap<U256, B256>>,
}

impl AccountState {
    pub fn is_empty(&self) -> bool {
        self.balance.is_none_or(|x| x.is_zero())
            && self.nonce.is_none_or(|x| x == 0)
            && self.code.as_ref().is_none_or(|s| s.is_empty())
            && self.storage.as_ref().is_none_or(|s| s.is_empty())
    }
}

#[derive(Default)]
pub struct Cache(pub HashMap<B160, AccountState>);

impl Cache {
    pub fn get_slot(&self, address: &B160, slot: &U256) -> Option<B256> {
        let el = self.0.get(address)?;
        el.storage.as_ref().and_then(|s| s.get(slot).cloned())
    }

    pub fn get_balance(&self, address: &B160) -> Option<U256> {
        let el = self.0.get(address)?;
        el.balance
    }

    pub fn get_nonce(&self, address: &B160) -> Option<u64> {
        let el = self.0.get(address)?;
        el.nonce
    }

    pub fn get_code(&self, address: &B160) -> Option<alloy::primitives::Bytes> {
        let el = self.0.get(address)?;
        el.code.clone()
    }

    fn filter_pre_account_state(
        &mut self,
        address: B160,
        new_account_state: AccountState,
    ) -> AccountState {
        let cache_el = self.0.entry(address).or_default();
        if cache_el.balance.is_none() {
            // Balance not touched yet
            cache_el.balance = new_account_state.balance;
        }
        if cache_el.nonce.is_none() {
            // Nonce not touched yet
            cache_el.nonce = new_account_state.nonce;
        }
        if cache_el.code.is_none() {
            // Code not touched yet
            cache_el.code = new_account_state.code;
        }
        if let Some(new_storage) = new_account_state.storage {
            new_storage.into_iter().for_each(|(key, value)| {
                let storage = cache_el.storage.get_or_insert_default();
                if let std::collections::btree_map::Entry::Vacant(e) = storage.entry(key) {
                    // Slot not touched yet
                    e.insert(value);
                }
            })
        }
        cache_el.clone()
    }
}

pub fn populate_prestate<const RANDOMIZED_TREE: bool>(
    chain: &mut Chain<RANDOMIZED_TREE>,
    ps: PrestateTrace,
) -> Cache {
    let mut cache = Cache::default();
    ps.result.into_iter().for_each(|item| {
        item.result.into_iter().for_each(|(address, account)| {
            let account = cache.filter_pre_account_state(address.0, account);
            // Set account properties
            chain.set_account_properties(
                address.0,
                account.balance,
                account.nonce,
                account.code.map(|b| b.to_vec()),
            );
            // Set storage slots
            if let Some(storage) = account.storage {
                storage
                    .into_iter()
                    .for_each(|(key, value)| chain.set_storage_slot(address.0, key, value))
            }
        });
    });
    cache
}
