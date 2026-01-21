use crate::{
    Clear, Collection, CollectionMut, CollectionRef, Get, Insert, Iter, Len, Remove,
    SimpleCollectionMut, SimpleCollectionRef,
};
use alloc::collections::BTreeSet;
use core::borrow::Borrow;

impl<T> Collection for BTreeSet<T> {
    type Item = T;
}

impl<T> CollectionRef for BTreeSet<T> {
    type ItemRef<'a>
        = &'a T
    where
        Self: 'a;

    crate::covariant_item_ref!();
}

impl<T> CollectionMut for BTreeSet<T> {
    type ItemMut<'a>
        = &'a mut T
    where
        Self: 'a;

    crate::covariant_item_mut!();
}

impl<T> SimpleCollectionRef for BTreeSet<T> {
    crate::simple_collection_ref!();
}

impl<T> SimpleCollectionMut for BTreeSet<T> {
    crate::simple_collection_mut!();
}

impl<T> Len for BTreeSet<T> {
    #[inline(always)]
    fn len(&self) -> usize {
        self.len()
    }

    #[inline(always)]
    fn is_empty(&self) -> bool {
        self.is_empty()
    }
}

impl<'a, Q, T: Ord> Get<&'a Q> for BTreeSet<T>
where
    T: Borrow<Q>,
    Q: Ord + ?Sized,
{
    #[inline(always)]
    fn get(&self, t: &'a Q) -> Option<&T> {
        self.get(t)
    }
}

impl<T: Ord> Insert for BTreeSet<T> {
    type Output = bool;

    #[inline(always)]
    fn insert(&mut self, t: T) -> bool {
        self.insert(t)
    }
}

impl<'a, Q, T: Ord> Remove<&'a Q> for BTreeSet<T>
where
    T: Borrow<Q>,
    Q: Ord + ?Sized,
{
    #[inline(always)]
    fn remove(&mut self, t: &'a Q) -> Option<T> {
        self.take(t)
    }
}

impl<T: Ord> Clear for BTreeSet<T> {
    #[inline(always)]
    fn clear(&mut self) {
        self.clear()
    }
}

impl<T> Iter for BTreeSet<T> {
    type Iter<'a>
        = alloc::collections::btree_set::Iter<'a, T>
    where
        Self: 'a;

    #[inline(always)]
    fn iter(&self) -> Self::Iter<'_> {
        self.iter()
    }
}
