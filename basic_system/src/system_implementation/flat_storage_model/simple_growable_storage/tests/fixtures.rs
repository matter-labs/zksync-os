use super::*;

pub(super) fn hex_bytes(s: &str) -> Bytes32 {
    let s = s.strip_prefix("0x").unwrap_or(s);
    assert_eq!(s.len(), 64);

    let mut bytes = [0_u8; 32];
    for (i, &byte) in s.as_bytes().iter().enumerate() {
        let hex_digit = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            _ => panic!("Invalid byte in hex string"),
        };
        let bit_shift = if i % 2 == 0 { 4 } else { 0 };
        bytes[i / 2] += hex_digit << bit_shift;
    }
    Bytes32::from_array(bytes)
}

pub(super) fn to_be_bytes(value: u64) -> Bytes32 {
    Bytes32::from_u256_be(&U256::try_from(value).unwrap())
}

pub(super) const EXACT: u32 = ExactIndexQuery::QUERY_ID;

pub(super) const PREV: u32 = PreviousIndexQuery::QUERY_ID;

pub(super) const PROOF: u32 = PROOF_FOR_INDEX_QUERY_ID;

pub(super) fn flat_key_of(key: &WarmStorageKey) -> Bytes32 {
    derive_flat_storage_key(&key.address, &key.key)
}

/// A storage slot for tests.
pub(super) fn storage_key(i: u64) -> WarmStorageKey {
    let mut address = [0u8; 20];
    address[12..].copy_from_slice(&i.to_be_bytes());
    WarmStorageKey {
        address: B160::from_be_bytes(address),
        key: to_be_bytes(i),
    }
}

/// The key `delta` away from `key`, as a big-endian integer.
pub(super) fn offset(key: &Bytes32, delta: i64) -> Bytes32 {
    let value = key.into_u256_be();
    let value = if delta >= 0 {
        value + U256::from(delta as u64)
    } else {
        value - U256::from(delta.unsigned_abs())
    };
    Bytes32::from_u256_be(&value)
}

/// A key with an 8-byte prefix and a 24-byte suffix.
pub(super) fn key_with_prefix(prefix: u64, suffix: &[u8; 24]) -> Bytes32 {
    let mut bytes = [0u8; 32];
    bytes[..8].copy_from_slice(&prefix.to_be_bytes());
    bytes[8..].copy_from_slice(suffix);
    Bytes32::from_array(bytes)
}

pub(super) fn suffix_of(i: u64) -> [u8; 24] {
    let mut suffix = [0u8; 24];
    suffix[16..].copy_from_slice(&i.to_be_bytes());
    suffix
}

/// How an entry of a batch accesses its slot.
#[derive(Clone, Copy, Debug)]
pub(super) enum Access {
    Read,
    Write(Bytes32),
    /// Writes the initial value back: a read for the batch
    Restore,
    /// Writes zero: a read for the batch if the slot is new
    Zero,
}

/// The batch entry of an access to a slot, with the initial value from the tree.
pub(super) fn entry<const R: bool>(
    tree: &TestingTree<R>,
    key: WarmStorageKey,
    access: Access,
) -> (WarmStorageKey, WarmStorageValue) {
    let initial_value = tree.get_value(&flat_key_of(&key));
    let is_new_storage_slot = initial_value.is_none();
    let initial_value = initial_value.unwrap_or_default();
    let current_value = match access {
        Access::Read | Access::Restore => initial_value,
        Access::Write(value) => value,
        Access::Zero => Bytes32::ZERO,
    };
    let value = WarmStorageValue {
        initial_value,
        current_value,
        initial_value_used: true,
        is_new_storage_slot,
        // The fields below are not used during verification
        value_at_the_start_of_tx: initial_value,
        changes_stack_depth: 0,
        last_accessed_at_tx_number: None,
        pubdata_diff_bytes: 0,
    };
    (key, value)
}

/// A tree with leaves of these keys, at positions 2, 3, ... in key order.
pub(super) fn tree_of(keys: &[Bytes32]) -> TestingTree<false> {
    let mut leaves = Vec::new_in(Global);
    for (i, key) in keys.iter().enumerate() {
        leaves.push((*key, to_be_bytes(i as u64 + 1)));
    }
    TestingTree::new_in_with_leaves(Global, leaves)
}

/// Checks a batch against the reference and against the tree: applies the writes to the tree,
/// and compares the commitments.
pub(super) fn check_batch_against_tree(
    tree: &mut TestingTree<false>,
    entries: &[(WarmStorageKey, WarmStorageValue)],
) -> DiffRuns {
    let runs = diff_run(tree, entries, no_tamper);
    let (root, next_free_slot) = runs.accepted();
    for (key, value) in entries {
        if value.current_value != value.initial_value {
            tree.insert(&flat_key_of(key), &value.current_value);
        }
    }
    assert_eq!(root, *tree.root());
    assert_eq!(next_free_slot, tree.next_free_slot);
    runs
}

pub(super) fn test_verifying_batch_proof(
    tree: &mut TestingTree<false>,
    entries: &[(WarmStorageKey, Option<Bytes32>)],
) {
    let entries: Vec<_> = entries
        .iter()
        .map(|(key, new_value)| {
            let access = new_value.map_or(Access::Read, Access::Write);
            entry(tree, *key, access)
        })
        .collect();
    check_batch_against_tree(tree, &entries);
}

pub(super) fn uniform_bytes() -> impl Strategy<Value = Bytes32> {
    proptest::array::uniform32(proptest::num::u8::ANY).prop_map(Bytes32::from_array)
}

pub(super) fn non_zero_bytes() -> impl Strategy<Value = Bytes32> {
    uniform_bytes().prop_filter("zero", |bytes| !bytes.is_zero())
}

pub(super) fn uniform_full_key() -> impl Strategy<Value = WarmStorageKey> {
    let uniform_address =
        proptest::array::uniform20(proptest::num::u8::ANY).prop_map(B160::from_be_bytes);
    (uniform_address, uniform_bytes()).prop_map(|(address, key)| WarmStorageKey { address, key })
}

pub(super) fn gen_entries() -> impl Strategy<Value = Vec<(WarmStorageKey, Option<Bytes32>)>> {
    let value = proptest::option::of(non_zero_bytes());
    proptest::collection::vec((uniform_full_key(), value), 0..=100)
}

pub(super) fn gen_previous_entries(
    size: ops::Range<usize>,
) -> impl Strategy<Value = Vec<(WarmStorageKey, Bytes32)>> {
    proptest::collection::vec((uniform_full_key(), uniform_bytes()), size)
}

pub(super) fn gen_reads_and_updates() -> impl Strategy<Value = Vec<(Index, Option<Bytes32>)>> {
    let value = proptest::option::of(non_zero_bytes());
    proptest::collection::vec((any::<Index>(), value), 0..=100)
}

pub(super) fn gen_access() -> impl Strategy<Value = Access> {
    prop_oneof![
        3 => Just(Access::Read),
        4 => non_zero_bytes().prop_map(Access::Write),
        1 => Just(Access::Restore),
        1 => Just(Access::Zero),
    ]
}

/// The slot of a batch entry: a slot of the tree, or a fresh one.
#[derive(Clone, Debug)]
pub(super) enum Slot {
    Existing(Index),
    Fresh(WarmStorageKey),
}

pub(super) fn gen_batch(
    len: ops::RangeInclusive<usize>,
) -> impl Strategy<Value = Vec<(Slot, Access)>> {
    let slot = prop_oneof![
        any::<Index>().prop_map(Slot::Existing),
        uniform_full_key().prop_map(Slot::Fresh),
    ];
    proptest::collection::vec((slot, gen_access()), len)
}

/// The entries of a batch, for distinct slots, in the given order.
pub(super) fn batch_entries<const R: bool>(
    tree: &TestingTree<R>,
    existing: &[WarmStorageKey],
    batch: &[(Slot, Access)],
) -> Vec<(WarmStorageKey, WarmStorageValue)> {
    let mut seen = HashSet::new();
    batch
        .iter()
        .filter_map(|(slot, access)| {
            let key = match slot {
                Slot::Existing(i) if !existing.is_empty() => *i.get(existing),
                Slot::Existing(_) => return None,
                Slot::Fresh(key) => *key,
            };
            seen.insert(key).then(|| entry(tree, key, *access))
        })
        .collect()
}

/// A tree with the slots inserted one by one, so that positions are in insertion order.
pub(super) fn tree_with_slots(slots: &[(WarmStorageKey, Bytes32)]) -> TestingTree<false> {
    let mut tree = TestingTree::new_in(Global);
    for (key, value) in slots {
        tree.insert(&flat_key_of(key), value);
    }
    tree
}
