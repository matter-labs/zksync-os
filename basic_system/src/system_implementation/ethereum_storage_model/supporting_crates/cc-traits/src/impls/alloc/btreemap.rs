use crate::{
    Clear, Collection, CollectionMut, CollectionRef, Get, GetKeyValue, GetMut, Iter, Keyed,
    KeyedRef, Len, MapInsert, MapIter, MapIterMut, Remove, SimpleCollectionMut,
    SimpleCollectionRef, SimpleKeyedRef,
};
use alloc::collections::BTreeMap;
use core::borrow::Borrow;

impl<K, V> Collection for BTreeMap<K, V> {
    type Item = V;
}

impl<K, V> CollectionRef for BTreeMap<K, V> {
    type ItemRef<'a>
        = &'a V
    where
        Self: 'a;

    crate::covariant_item_ref!();
}

impl<K, V> CollectionMut for BTreeMap<K, V> {
    type ItemMut<'a>
        = &'a mut V
    where
        Self: 'a;

    crate::covariant_item_mut!();
}

impl<K, V> SimpleCollectionRef for BTreeMap<K, V> {
    crate::simple_collection_ref!();
}

impl<K, V> SimpleCollectionMut for BTreeMap<K, V> {
    crate::simple_collection_mut!();
}

impl<K, V> Keyed for BTreeMap<K, V> {
    type Key = K;
}

impl<K, V> KeyedRef for BTreeMap<K, V> {
    type KeyRef<'a>
        = &'a K
    where
        Self: 'a;

    crate::covariant_key_ref!();
}

impl<K, V> SimpleKeyedRef for BTreeMap<K, V> {
    crate::simple_keyed_ref!();
}

impl<K, V> Len for BTreeMap<K, V> {
    #[inline(always)]
    fn len(&self) -> usize {
        self.len()
    }

    #[inline(always)]
    fn is_empty(&self) -> bool {
        self.is_empty()
    }
}

impl<'a, Q, K: Ord, V> Get<&'a Q> for BTreeMap<K, V>
where
    K: Borrow<Q>,
    Q: Ord + ?Sized,
{
    #[inline(always)]
    fn get(&self, key: &'a Q) -> Option<&V> {
        self.get(key)
    }
}

impl<'a, Q, K: Ord, V> GetKeyValue<&'a Q> for BTreeMap<K, V>
where
    K: Borrow<Q>,
    Q: Ord + ?Sized,
{
    #[inline(always)]
    fn get_key_value(&self, key: &'a Q) -> Option<(&K, &V)> {
        self.get_key_value(key)
    }
}

impl<'a, Q, K: Ord, V> GetMut<&'a Q> for BTreeMap<K, V>
where
    K: Borrow<Q>,
    Q: Ord + ?Sized,
{
    #[inline(always)]
    fn get_mut(&mut self, key: &'a Q) -> Option<&mut V> {
        self.get_mut(key)
    }
}

impl<K: Ord, V> MapInsert<K> for BTreeMap<K, V> {
    type Output = Option<V>;

    #[inline(always)]
    fn insert(&mut self, key: K, value: V) -> Option<V> {
        self.insert(key, value)
    }
}

impl<'a, Q, K: Ord, V> Remove<&'a Q> for BTreeMap<K, V>
where
    K: Borrow<Q>,
    Q: Ord + ?Sized,
{
    #[inline(always)]
    fn remove(&mut self, key: &'a Q) -> Option<V> {
        self.remove(key)
    }
}

impl<K: Ord, V> Clear for BTreeMap<K, V> {
    #[inline(always)]
    fn clear(&mut self) {
        self.clear()
    }
}

impl<K, V> Iter for BTreeMap<K, V> {
    type Iter<'a>
        = alloc::collections::btree_map::Values<'a, K, V>
    where
        Self: 'a;

    #[inline(always)]
    fn iter(&self) -> Self::Iter<'_> {
        self.values()
    }
}

impl<K, V> MapIter for BTreeMap<K, V> {
    type Iter<'a>
        = alloc::collections::btree_map::Iter<'a, K, V>
    where
        Self: 'a;

    #[inline(always)]
    fn iter(&self) -> Self::Iter<'_> {
        self.iter()
    }
}

impl<K, V> MapIterMut for BTreeMap<K, V> {
    type IterMut<'a>
        = alloc::collections::btree_map::IterMut<'a, K, V>
    where
        Self: 'a;

    #[inline(always)]
    fn iter_mut(&mut self) -> Self::IterMut<'_> {
        self.iter_mut()
    }
}
