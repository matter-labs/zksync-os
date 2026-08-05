use crate::{
    Clear, Collection, CollectionMut, CollectionRef, Get, Insert, Iter, Len, Remove,
    SimpleCollectionMut, SimpleCollectionRef,
};
use std::{borrow::Borrow, collections::HashSet, hash::Hash};

impl<T> Collection for HashSet<T> {
    type Item = T;
}

impl<T> CollectionRef for HashSet<T> {
    type ItemRef<'a>
        = &'a T
    where
        Self: 'a;

    crate::covariant_item_ref!();
}

impl<T> CollectionMut for HashSet<T> {
    type ItemMut<'a>
        = &'a mut T
    where
        Self: 'a;

    crate::covariant_item_mut!();
}

impl<T> SimpleCollectionRef for HashSet<T> {
    crate::simple_collection_ref!();
}

impl<T> SimpleCollectionMut for HashSet<T> {
    crate::simple_collection_mut!();
}

impl<T> Len for HashSet<T> {
    #[inline(always)]
    fn len(&self) -> usize {
        self.len()
    }

    #[inline(always)]
    fn is_empty(&self) -> bool {
        self.is_empty()
    }
}

impl<'a, Q, T: Hash + Eq> Get<&'a Q> for HashSet<T>
where
    T: Borrow<Q>,
    Q: Hash + Eq + ?Sized,
{
    fn get(&self, value: &'a Q) -> Option<&T> {
        self.get(value)
    }
}

impl<T: Hash + Eq> Insert for HashSet<T> {
    type Output = bool;

    #[inline(always)]
    fn insert(&mut self, t: T) -> bool {
        self.insert(t)
    }
}

impl<'a, Q, T: Hash + Eq> Remove<&'a Q> for HashSet<T>
where
    T: Borrow<Q>,
    Q: Hash + Eq + ?Sized,
{
    #[inline(always)]
    fn remove(&mut self, t: &'a Q) -> Option<T> {
        self.take(t)
    }
}

impl<T: Hash + Eq> Clear for HashSet<T> {
    #[inline(always)]
    fn clear(&mut self) {
        self.clear()
    }
}

impl<T> Iter for HashSet<T> {
    type Iter<'a>
        = std::collections::hash_set::Iter<'a, T>
    where
        Self: 'a;

    #[inline(always)]
    fn iter(&self) -> Self::Iter<'_> {
        self.iter()
    }
}
