use core::marker::PhantomData;

use crate::{qol::{PipeOp, UnsafeCellEx}, types::ints::U256BE};

use super::{entries::Entry, Storeable, StoreableIndirection};


#[derive(Default)]
pub struct Value<T> {
    phantom: PhantomData<T>,
}

impl<T: Storeable> Stored<Value<T>, false> {

    /// Reads the value from the storage.
    pub fn read(&self) -> T {
        crate::impl_arch::system::storage::read_s(&U256BE::from_usize(self.ix))
            .to(|v| T::from_u256_be(&v, 0))
    }

    /// Writes the value to the storage.
    pub fn write(&mut self, value: &T) {
        crate::impl_arch::system::storage::write_s(&U256BE::from_usize(self.ix), &value.to_u256_be(0));
    }
}

pub struct Mapping<'contract, K, V> {
    key: U256BE,
    phantom: PhantomData<&'contract (K, V)>,
}

impl<K: Storeable, V> Mapping<'_, K, V> {
    pub fn new(key: U256BE) -> Self {
        Self { key, phantom: PhantomData }
    }

    pub fn get_ix_hash(&self, key: &K) -> U256BE {
        let key = crate::system::slice::hash_keccak256(self.key.as_bytes());

        key
    }
}

// Solidity storage layout:
// For:
//  - `p`: mapping storage slot
//  - `k`: element key
//  - `h`: `pad(k)` | when `k` is value type
//         `k`      | when `k` is dynamic array
//  - `.`: concatenation
// `keccak256(h(k) . p)`
impl<K: Storeable, V: Storeable> Mapping<'_, K, V> {
    pub fn instantiate(key: &U256BE) -> Self {
        Self {
            key: key.clone(),
            phantom: PhantomData,
        }
    }

    fn read_raw(key: &U256BE) -> V {
        crate::impl_arch::system::storage::read_s(key).to(|v| V::from_u256_be(&v, 0))
    }

    fn write_raw(key: &U256BE, value: &U256BE) {
        crate::impl_arch::system::storage::write_s(key, value)
    }

    pub fn entry(&self, key: &K) -> Entry<V> {
        let key_hash = self.get_ix_hash(key);
        Entry {
            value: crate::impl_arch::system::storage::read_s(&key_hash).to(|v| V::from_u256_be(&v, 0)),
            key_hash,
        }
    }

    pub fn read(&self, key: &K) -> V {
        let key_hash = self.get_ix_hash(key);
        crate::impl_arch::system::storage::read_s(&key_hash).to(|v| V::from_u256_be(&v, 0))
    }

    pub fn write(&mut self, key: &K, value: &V) {
        let key_hash = self.get_ix_hash(key);
        crate::impl_arch::system::storage::write_s(&key_hash, &value.to_u256_be(0));
    }
}

impl<K: Storeable, V: StoreableIndirection> Mapping<'_, K, V> {
    fn get(&self, key: &K) -> V {
        V::from_key(&self.get_ix_hash(key))
    }
}

pub struct Stored<T, const TRANSIENT: bool = false> {
    ix: usize,
    store_kind: T,
}

impl<T, const TRANSIENT: bool> Stored<T, TRANSIENT> {
    pub fn new(ix: usize, t: T) -> Self {
        Self { ix, store_kind: t }
    }
}

impl<K: Storeable, V: Storeable> Stored<Mapping<'_, K, V>> {
    pub fn read(&self, key: &K) -> V {
        self.store_kind.read(key)
    }

    pub fn write(&mut self, key: &K, value: &V) {
        self.store_kind.write(key, value);
    }

    pub fn entry(&self, key: &K) -> Entry<V> {
        self.store_kind.entry(key)
    }
}

impl<K: Storeable, V: StoreableIndirection> Stored<Mapping<'_, K, V>> {
    pub fn get(&self, key: &K) -> V {
        self.store_kind.get(key)
    }
}
