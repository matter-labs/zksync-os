use ruint::{
    aliases::{B160, B256, U256},
    Bits,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PrestateTrace {
    pub result: Vec<PrestateItem>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PrestateItem {
    pub result: BTreeMap<BitsOrd160, AccountState>,
}

// Note: we need both prestate and diff traces, as the diff trace "pre"
// section doesn't include all touched slots, only non-zero ones.
// This means that we cannot construct an initial state only from
// the pre side of the diff trace.

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DiffTrace {
    pub result: Vec<DiffItem>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DiffItem {
    pub result: StateDiff,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct StateDiff {
    pub pre: BTreeMap<BitsOrd160, AccountState>,
    pub post: BTreeMap<BitsOrd160, AccountState>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct AccountState {
    pub balance: Option<U256>,
    pub nonce: Option<u64>,
    pub code: Option<alloy::primitives::Bytes>,
    pub storage: Option<BTreeMap<U256, B256>>,
}

#[derive(Clone, Default)]
pub struct Cache(pub BTreeMap<BitsOrd160, AccountState>);

impl Cache {
    fn filter_pre_account_state(&mut self, address: B160, new_account_state: AccountState) {
        let cache_el = self.0.entry(address.into()).or_default();
        if cache_el.balance.is_none() && cache_el.nonce.is_none() && cache_el.code.is_none() {
            // Balance not touched yet
            cache_el.balance = new_account_state.balance;

            // Nonce not touched yet
            // Tracer omits nonce when it's 0, we need to fill it in
            cache_el.nonce = Some(new_account_state.nonce.unwrap_or(0));

            // Code not touched yet
            cache_el.code = new_account_state.code;
        }
        if let Some(new_storage) = new_account_state.storage {
            new_storage.into_iter().for_each(|(key, value)| {
                let storage = cache_el.storage.get_or_insert_default();
                // only first touch
                if let std::collections::btree_map::Entry::Vacant(e) = storage.entry(key) {
                    // Slot not touched yet
                    e.insert(value);
                }
            })
        }
    }

    fn update_account_state(&mut self, diff: DiffItem) {
        let pre = diff.result.pre;
        let mut post = diff.result.post;
        for (address, pre) in pre.into_iter() {
            let post = post.remove(&address).unwrap_or_default();
            // if format!("0x{:x}", address.0.into_inner()) == "0x957c7fa189a408e78543113412f6ae1a9b4022c4"
            // {
            //     dbg!(&pre.storage);
            //     dbg!(&post.storage);
            // }
            if let Some(cache_el) = self.0.get_mut(&address) {
                if let Some(balance) = pre.balance {
                    assert_eq!(
                        balance,
                        *cache_el.balance.get_or_insert_with(|| { balance })
                    );
                }
                if let Some(nonce) = pre.nonce {
                    assert_eq!(nonce, *cache_el.nonce.get_or_insert_with(|| { nonce }));
                }
                if let Some(code) = pre.code.as_ref() {
                    assert_eq!(code, cache_el.code.get_or_insert_with(|| { code.clone() }));
                }

                if let Some(balance) = post.balance {
                    cache_el.balance = Some(balance);
                }
                if let Some(nonce) = post.nonce {
                    cache_el.nonce = Some(nonce);
                }
                if let Some(code) = post.code {
                    cache_el.code = Some(code);
                }

                let initial_state = pre.storage.unwrap_or_default();
                let mut final_state = post.storage.unwrap_or_default();

                let cache_storage = cache_el.storage.get_or_insert_default();
                // updates/deletes
                for (slot, _) in initial_state {
                    let final_value = final_state.remove(&slot).unwrap_or_default();
                    cache_storage.insert(slot, final_value);
                }
                // and inserts
                for (slot, final_value) in final_state.into_iter() {
                    cache_storage.insert(slot, final_value);
                }
            } else {
                panic!(
                    "Missing initial state for address 0x{:040x}",
                    address.0.into_inner()
                );
            }
        }

        // and all new addresses from post
        for (address, post) in post.into_iter() {
            // if format!("0x{:x}", address.0.into_inner()) == "0x957c7fa189a408e78543113412f6ae1a9b4022c4"
            // {
            //     dbg!(&post.storage);
            // }
            let cache_el = self.0.entry(address).or_default();
            if let Some(balance) = post.balance {
                cache_el.balance = Some(balance);
            }
            if let Some(nonce) = post.nonce {
                cache_el.nonce = Some(nonce);
            }
            if let Some(code) = post.code {
                cache_el.code = Some(code);
            }
            let cache_storage = cache_el.storage.get_or_insert_default();
            for (slot, final_value) in post.storage.unwrap_or_default() {
                cache_storage.insert(slot, final_value);
            }
        }
    }
}

pub fn compute_post_state(cache: &mut Cache, full_block_diffs: DiffTrace) {
    full_block_diffs.result.into_iter().for_each(|item| {
        cache.update_account_state(item);
    });
}

pub fn compute_initial_and_final_states(
    ps: PrestateTrace,
    full_block_diffs: DiffTrace,
) -> (Cache, Cache) {
    let mut initial_state = Cache::default();
    ps.result.into_iter().for_each(|item| {
        item.result.into_iter().for_each(|(address, account)| {
            initial_state.filter_pre_account_state(address.0, account.clone());
        });
    });
    let mut final_state = initial_state.clone();
    compute_post_state(&mut final_state, full_block_diffs);

    (initial_state, final_state)
}
