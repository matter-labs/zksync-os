use rig::Chain;
use ruint::{
    aliases::{B160, B256, U256},
    Bits,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

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

impl AccountState {
    pub fn is_empty(&self) -> bool {
        self.balance.is_none()
            && self.nonce.is_none()
            && self.code.as_ref().is_none()
            && self.storage.as_ref().is_none()
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
        // Tracer omits nonce when it's 0, we need to fill it in
        Some(el.nonce.unwrap_or(0))
    }

    pub fn get_code(&self, address: &B160) -> Option<alloy::primitives::Bytes> {
        let el = self.0.get(address)?;
        Some(el.code.clone().unwrap_or_default())
    }

    fn filter_pre_account_state(
        &mut self,
        address: B160,
        new_account_state: AccountState,
    ) -> AccountState {
        let cache_el = self.0.entry(address).or_default();
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
                if let std::collections::btree_map::Entry::Vacant(e) = storage.entry(key) {
                    // Slot not touched yet
                    e.insert(value);
                }
            })
        }
        cache_el.clone()
    }
}

/// Accounts created during the block (absent at block start). Identified from
/// the diff trace: an account whose first appearance (in tx order) is in a tx's
/// `post` but not its `pre` was created mid-block. The prestate tracer omits
/// such accounts from their creating tx and first reports them in a later tx
/// with a mid-block balance, so that later prestate must not be taken as the
/// block-initial state.
fn created_mid_block_accounts(diff: &DiffTrace) -> HashSet<B160> {
    let mut seen: HashSet<B160> = HashSet::new();
    let mut created: HashSet<B160> = HashSet::new();
    for item in diff.result.iter() {
        let d = &item.result;
        for addr in d.pre.keys().chain(d.post.keys()) {
            // Only judge each account at its first diff-trace appearance.
            if seen.insert(addr.0) && d.post.contains_key(addr) && !d.pre.contains_key(addr) {
                created.insert(addr.0);
            }
        }
    }
    created
}

pub fn populate_prestate<const RANDOMIZED_TREE: bool>(
    chain: &mut Chain<RANDOMIZED_TREE>,
    ps: PrestateTrace,
    diff: &DiffTrace,
) -> Cache {
    let mut cache = Cache::default();

    let created = created_mid_block_accounts(diff);

    // Pre-seed accounts created mid-block as empty (balance 0). This prevents a
    // later tx's prestate (which reports their post-creation state) from being
    // taken as the block-initial state — `filter_pre_account_state` fills each
    // field only once. Cache-only: chain state stays empty for these accounts.
    for address in &created {
        cache.0.entry(*address).or_insert(AccountState {
            balance: Some(ruint::aliases::U256::ZERO),
            ..Default::default()
        });
    }

    ps.result.into_iter().for_each(|item| {
        item.result.into_iter().for_each(|(address, mut account)| {
            // A mid-block-created account had no storage at block start, so any
            // storage a later prestate reports for it is post-creation. Drop it
            // here so it isn't installed as block-initial state (the balance
            // pre-seed above only blocks balance/nonce/code, not storage).
            // Execution replays the writes, so the slots are rebuilt anyway.
            if created.contains(&address.0) {
                account.storage = None;
            }
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
