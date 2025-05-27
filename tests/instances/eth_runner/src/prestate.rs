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
        self.balance.is_none()
            && self.nonce.is_none()
            && self.code.is_none()
            && self.storage.as_ref().is_none_or(|s| s.is_empty())
    }
}

// True  when already touched
#[derive(Default)]
pub struct CacheElement {
    balance: bool,
    nonce: bool,
    code: bool,
    storage: HashMap<U256, B256>,
}

#[derive(Default)]
pub struct Cache(HashMap<B160, CacheElement>);

impl Cache {
    pub fn get_slot(&self, address: &B160, slot: &U256) -> Option<B256> {
        let el = self.0.get(address)?;
        el.storage.get(slot).cloned()
    }

    fn filter_pre_account_state(
        &mut self,
        address: B160,
        new_account_state: AccountState,
    ) -> AccountState {
        let cache_el = self.0.entry(address).or_default();
        let balance = if !cache_el.balance {
            // Balance not touched yet
            cache_el.balance = true;
            new_account_state.balance
        } else {
            None
        };
        let nonce = if !cache_el.nonce {
            // Nonce not touched yet
            cache_el.nonce = true;
            new_account_state.nonce
        } else {
            None
        };
        let code = if !cache_el.code {
            // Code not touched yet
            cache_el.code = true;
            new_account_state.code
        } else {
            None
        };
        let mut storage = BTreeMap::<U256, B256>::new();
        if let Some(new_storage) = new_account_state.storage {
            new_storage.into_iter().for_each(|(key, value)| {
                if let std::collections::hash_map::Entry::Vacant(e) = cache_el.storage.entry(key) {
                    // Slot not touched yet
                    e.insert(value);
                    storage.insert(key, value);
                }
            })
        }
        let storage = if storage.is_empty() {
            None
        } else {
            Some(storage)
        };
        AccountState {
            balance,
            nonce,
            code,
            storage,
        }
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
