//! Interners of the keys the Ethereum storage model is addressed by: every
//! distinct address and every distinct slot key seen in the block gets a small
//! dense index (`u32`, in order of first appearance), and the caches are keyed
//! by those indices. A cache lookup is then a hash of 20 or 32 bytes plus a
//! hash map probe instead of an ordered search over wide keys, and the account
//! cache becomes a direct table.
//!
//! The indices carry no ordering: the state commitment sorts accounts and slots
//! by their trie keys anyway.
//!
//! Capacities are fixed: the proving-mode allocator forbids reallocation, so
//! every table is allocated once at full size. They are sized from the block
//! gas limit: at 60M gas a block can touch at most ~32k distinct slots (an
//! access-list slot costs 1900 gas) and ~25k distinct accounts, so 2^16
//! entries leave a 2x margin. Running out of room is an internal error, which
//! fails the block instead of corrupting the caches.

use alloc::vec::Vec;
use core::alloc::Allocator;
use core::hash::{Hash, Hasher};
use hashbrown::hash_map::Entry;
use hashbrown::HashMap;
use ruint::aliases::B160;
use zk_ee::internal_error;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::utils::word_hasher::BuildWordHasher;
use zk_ee::utils::Bytes32;

/// Maximum number of distinct addresses a block can touch
pub const MAX_UNIQUE_ADDRESSES: usize = 1 << 16;
/// Maximum number of distinct slot keys a block can touch, across all accounts
pub const MAX_UNIQUE_SLOT_KEYS: usize = 1 << 16;
/// Maximum number of distinct (address, slot key) pairs a block can touch: the
/// cold access of every such pair costs at least 1900 gas
pub const MAX_UNIQUE_SLOTS: usize = 1 << 16;

const _: () = assert!(MAX_UNIQUE_ADDRESSES <= 1 << 16 && MAX_UNIQUE_SLOT_KEYS <= 1 << 16);

/// The key of a cached storage slot: the address index in the high half, the
/// slot key index in the low half. Both are below 2^16.
#[inline(always)]
pub fn slot_cache_key(address_index: u32, key_index: u32) -> u32 {
    debug_assert!((address_index as usize) < MAX_UNIQUE_ADDRESSES);
    debug_assert!((key_index as usize) < MAX_UNIQUE_SLOT_KEYS);
    (address_index << 16) | key_index
}

/// The two indices a slot cache key was made of
#[inline(always)]
pub fn split_slot_cache_key(key: u32) -> (u32, u32) {
    (key >> 16, key & 0xffff)
}

/// A key that can be interned. The hash and the equality are word-wise, so
/// no generic slice compare (a `memcmp` call on the proving target) runs on
/// the lookup path.
pub trait InternKey: Copy + Eq + Hash {}

/// An address, hashed limb by limb
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct AddressKey(pub B160);

impl Hash for AddressKey {
    #[inline(always)]
    fn hash<H: Hasher>(&self, state: &mut H) {
        for limb in self.0.as_limbs() {
            state.write_u64(*limb);
        }
    }
}

impl InternKey for AddressKey {}

/// A slot key, hashed word by word
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct SlotKey(pub Bytes32);

impl Hash for SlotKey {
    #[inline(always)]
    fn hash<H: Hasher>(&self, state: &mut H) {
        for word in self.0.as_usize_array_ref() {
            state.write_usize(*word);
        }
    }
}

impl InternKey for SlotKey {}

/// Assigns dense indices to distinct keys, in order of first appearance: the
/// index of a new key is the number of keys seen before it.
pub struct Interner<K: InternKey, A: Allocator> {
    /// Key -> index, allocated once for `max_keys` keys so it never grows
    indices: HashMap<K, u32, BuildWordHasher, A>,
    /// The keys by index, for the reports and the state commitment. Pushed as
    /// keys arrive; its capacity is the maximum number of keys, so it never
    /// reallocates.
    keys: Vec<K, A>,
    /// The last key interned and its index: consecutive lookups repeat the key
    /// often (the storage ops of one contract, an SLOAD followed by the SSTORE
    /// of the same slot), and a compare is much cheaper than a hash probe.
    last: Option<(K, u32)>,
}

impl<K: InternKey, A: Allocator> Interner<K, A> {
    /// An interner of at most `max_keys` keys
    pub fn new_in(max_keys: usize, alloc: A) -> Self
    where
        A: Clone,
    {
        assert!(max_keys < u32::MAX as usize);
        Self {
            indices: HashMap::with_capacity_and_hasher_in(max_keys, BuildWordHasher, alloc.clone()),
            keys: Vec::with_capacity_in(max_keys, alloc),
            last: None,
        }
    }

    /// Number of distinct keys seen so far
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.indices.len()
    }

    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    /// The key with this index. The index must have been produced by this
    /// interner.
    #[inline(always)]
    pub fn key(&self, index: u32) -> &K {
        &self.keys[index as usize]
    }

    /// All the keys, by index
    pub fn keys(&self) -> &[K] {
        &self.keys
    }

    /// The index of a key that was interned before
    #[inline(always)]
    pub fn get(&self, key: &K) -> Option<u32> {
        self.indices.get(key).copied()
    }

    /// The index of a key, assigning the next one if the key is new. Fails
    /// when the interner is full.
    #[inline(always)]
    pub fn intern(&mut self, key: &K) -> Result<u32, InternalError> {
        if let Some((last_key, index)) = &self.last {
            if last_key == key {
                return Ok(*index);
            }
        }
        let index = match self.indices.entry(*key) {
            Entry::Occupied(e) => *e.get(),
            Entry::Vacant(e) => {
                // within the reserved capacity: neither container grows
                if self.keys.len() == self.keys.capacity() {
                    return Err(internal_error!("interner: too many distinct keys"));
                }
                let index = self.keys.len() as u32;
                self.keys.push(*key);
                e.insert(index);
                index
            }
        };
        self.last = Some((*key, index));
        Ok(index)
    }
}

/// A key together with its interned index. Handed down to the caches, which
/// look elements up by the index and still have the key for the oracle
/// queries and the reports.
#[derive(Clone, Copy, Debug)]
pub struct Interned<'a, K> {
    pub index: u32,
    pub key: &'a K,
}

/// The interners of the Ethereum storage model, shared by its caches
pub struct EthereumKeyInterners<A: Allocator> {
    pub addresses: Interner<AddressKey, A>,
    pub slot_keys: Interner<SlotKey, A>,
}

impl<A: Allocator + Clone> EthereumKeyInterners<A> {
    pub fn new_in(alloc: A) -> Self {
        Self {
            addresses: Interner::new_in(MAX_UNIQUE_ADDRESSES, alloc.clone()),
            slot_keys: Interner::new_in(MAX_UNIQUE_SLOT_KEYS, alloc),
        }
    }

    /// The interned addresses, by index
    #[inline(always)]
    pub fn addresses(&self) -> &[B160] {
        // Safety: `AddressKey` is a transparent wrapper of `B160`
        unsafe {
            core::slice::from_raw_parts(self.addresses.keys().as_ptr().cast(), self.addresses.len())
        }
    }

    /// The interned slot keys, by index
    #[inline(always)]
    pub fn slot_keys(&self) -> &[Bytes32] {
        // Safety: `SlotKey` is a transparent wrapper of `Bytes32`
        unsafe {
            core::slice::from_raw_parts(self.slot_keys.keys().as_ptr().cast(), self.slot_keys.len())
        }
    }

    #[inline(always)]
    pub fn address_index(&self, address: &B160) -> Option<u32> {
        self.addresses.get(&AddressKey(*address))
    }

    #[inline(always)]
    pub fn slot_key_index(&self, key: &Bytes32) -> Option<u32> {
        self.slot_keys.get(&SlotKey(*key))
    }

    #[inline(always)]
    pub fn intern_address<'a>(
        &mut self,
        address: &'a B160,
    ) -> Result<Interned<'a, B160>, InternalError> {
        Ok(Interned {
            index: self.addresses.intern(&AddressKey(*address))?,
            key: address,
        })
    }

    #[inline(always)]
    pub fn intern_slot_key<'a>(
        &mut self,
        key: &'a Bytes32,
    ) -> Result<Interned<'a, Bytes32>, InternalError> {
        Ok(Interned {
            index: self.slot_keys.intern(&SlotKey(*key))?,
            key,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::alloc::Global;

    #[test]
    fn interns_in_order_of_first_appearance() {
        let mut interner = Interner::<AddressKey, Global>::new_in(8, Global);
        let a = AddressKey(B160::from_limbs([1, 0, 0]));
        let b = AddressKey(B160::from_limbs([2, 0, 0]));
        assert_eq!(interner.get(&a), None);
        assert_eq!(interner.intern(&a).unwrap(), 0);
        assert_eq!(interner.intern(&a).unwrap(), 0, "repeated key");
        assert_eq!(interner.intern(&b).unwrap(), 1);
        assert_eq!(interner.intern(&b).unwrap(), 1);
        assert_eq!(interner.intern(&a).unwrap(), 0);
        assert_eq!(interner.get(&b), Some(1));
        assert_eq!(interner.len(), 2);
        assert_eq!(*interner.key(1), b);
        assert_eq!(interner.keys(), &[a, b]);
    }

    #[test]
    fn refuses_more_keys_than_its_capacity_and_never_grows() {
        let mut interner = Interner::<SlotKey, Global>::new_in(4, Global);
        let allocated = interner.indices.capacity();
        for i in 0..4u64 {
            let key = SlotKey(Bytes32::from_u256_le(&ruint::aliases::U256::from(i + 1)));
            assert_eq!(interner.intern(&key).unwrap(), i as u32);
        }
        let extra = SlotKey(Bytes32::from_u256_le(&ruint::aliases::U256::from(100u64)));
        assert!(interner.intern(&extra).is_err());
        assert_eq!(interner.get(&extra), None);
        // the ones that fit are still there
        let key = SlotKey(Bytes32::from_u256_le(&ruint::aliases::U256::from(3u64)));
        assert_eq!(interner.get(&key), Some(2));
        assert_eq!(interner.indices.capacity(), allocated);
    }

    #[test]
    fn slot_keys_that_differ_only_in_a_high_word_are_distinct() {
        let mut interner = Interner::<SlotKey, Global>::new_in(32, Global);
        let mut seen = std::collections::HashSet::new();
        for word in 0..4 {
            for v in 1..=8u64 {
                let mut bytes = [0u8; 32];
                bytes[word * 8..][..8].copy_from_slice(&v.to_le_bytes());
                let key = SlotKey(Bytes32::from_array(bytes));
                let index = interner.intern(&key).unwrap();
                assert!(seen.insert(index), "distinct keys get distinct indices");
                assert_eq!(interner.get(&key), Some(index));
            }
        }
    }

    #[test]
    fn model_interners_expose_the_keys_by_index() {
        let mut interners = EthereumKeyInterners::new_in(Global);
        let address = B160::from_limbs([7, 0, 0]);
        let key = Bytes32::from_byte_fill(9);
        let a = interners.intern_address(&address).unwrap();
        let k = interners.intern_slot_key(&key).unwrap();
        assert_eq!((a.index, k.index), (0, 0));
        assert_eq!(interners.addresses(), &[address]);
        assert_eq!(interners.slot_keys(), &[key]);
        assert_eq!(interners.address_index(&address), Some(0));
        assert_eq!(interners.slot_key_index(&Bytes32::ZERO), None);
    }

    #[test]
    fn slot_cache_key_packs_both_indices() {
        let key = slot_cache_key(0xabcd, 0x1234);
        assert_eq!(split_slot_cache_key(key), (0xabcd, 0x1234));
        assert_eq!(split_slot_cache_key(slot_cache_key(0, 0)), (0, 0));
    }
}
