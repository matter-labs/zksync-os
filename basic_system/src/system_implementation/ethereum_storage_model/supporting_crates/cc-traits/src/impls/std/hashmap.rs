use crate::{
    Clear, Collection, CollectionMut, CollectionRef, Get, GetKeyValue, GetMut, Iter, Keyed,
    KeyedRef, Len, MapInsert, MapIter, MapIterMut, Remove, SimpleCollectionMut,
    SimpleCollectionRef, SimpleKeyedRef,
};
use std::{borrow::Borrow, collections::HashMap, hash::Hash};

impl<K, V> Collection for HashMap<K, V> {
    type Item = V;
}

impl<K, V> CollectionRef for HashMap<K, V> {
    type ItemRef<'a>
        = &'a V
    where
        Self: 'a;

    crate::covariant_item_ref!();
}

impl<K, V> CollectionMut for HashMap<K, V> {
    type ItemMut<'a>
        = &'a mut V
    where
        Self: 'a;

    crate::covariant_item_mut!();
}

impl<K, V> SimpleCollectionRef for HashMap<K, V> {
    crate::simple_collection_ref!();
}

impl<K, V> SimpleCollectionMut for HashMap<K, V> {
    crate::simple_collection_mut!();
}

impl<K, V> Keyed for HashMap<K, V> {
    type Key = K;
}

impl<K, V> KeyedRef for HashMap<K, V> {
    type KeyRef<'a>
        = &'a K
    where
        Self: 'a;

    crate::covariant_key_ref!();
}

impl<K, V> SimpleKeyedRef for HashMap<K, V> {
    crate::simple_keyed_ref!();
}

impl<K, V> Len for HashMap<K, V> {
    #[inline(always)]
    fn len(&self) -> usize {
        self.len()
    }

    #[inline(always)]
    fn is_empty(&self) -> bool {
        self.is_empty()
    }
}

impl<'a, Q, K: Hash + Eq, V> Get<&'a Q> for HashMap<K, V>
where
    K: Borrow<Q>,
    Q: Hash + Eq + ?Sized,
{
    #[inline(always)]
    fn get(&self, key: &'a Q) -> Option<&V> {
        self.get(key)
    }
}

impl<'a, Q, K: Hash + Eq, V> GetMut<&'a Q> for HashMap<K, V>
where
    K: Borrow<Q>,
    Q: Hash + Eq + ?Sized,
{
    #[inline(always)]
    fn get_mut(&mut self, key: &'a Q) -> Option<&mut V> {
        self.get_mut(key)
    }
}

impl<'a, Q, K: Hash + Eq, V> GetKeyValue<&'a Q> for HashMap<K, V>
where
    K: Borrow<Q>,
    Q: Hash + Eq + ?Sized,
{
    #[inline(always)]
    fn get_key_value(&self, key: &'a Q) -> Option<(&K, &V)> {
        self.get_key_value(key)
    }
}

impl<K: Hash + Eq, V> MapInsert<K> for HashMap<K, V> {
    type Output = Option<V>;

    #[inline(always)]
    fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.insert(key, value)
    }
}

impl<'a, Q, K: Hash + Eq, V> Remove<&'a Q> for HashMap<K, V>
where
    K: Borrow<Q>,
    Q: Hash + Eq + ?Sized,
{
    #[inline(always)]
    fn remove(&mut self, key: &'a Q) -> Option<V> {
        self.remove(key)
    }
}

impl<K, V> Clear for HashMap<K, V> {
    #[inline(always)]
    fn clear(&mut self) {
        self.clear()
    }
}

impl<K, V> Iter for HashMap<K, V> {
    type Iter<'a>
        = std::collections::hash_map::Values<'a, K, V>
    where
        Self: 'a;

    #[inline(always)]
    fn iter(&self) -> Self::Iter<'_> {
        self.values()
    }
}

impl<K, V> MapIter for HashMap<K, V> {
    type Iter<'a>
        = std::collections::hash_map::Iter<'a, K, V>
    where
        Self: 'a;

    #[inline(always)]
    fn iter(&self) -> Self::Iter<'_> {
        self.iter()
    }
}

impl<K, V> MapIterMut for HashMap<K, V> {
    type IterMut<'a>
        = std::collections::hash_map::IterMut<'a, K, V>
    where
        Self: 'a;

    #[inline(always)]
    fn iter_mut(&mut self) -> Self::IterMut<'_> {
        self.iter_mut()
    }
}
