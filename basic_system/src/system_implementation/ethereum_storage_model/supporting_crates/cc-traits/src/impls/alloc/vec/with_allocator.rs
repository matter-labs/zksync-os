use crate::{
    Capacity, Clear, Collection, CollectionMut, CollectionRef, Get, GetMut, Iter, IterMut, Len,
    PopBack, PushBack, Remove, Reserve, SimpleCollectionMut, SimpleCollectionRef, WithCapacityIn,
};
use alloc::vec::Vec;
use core::alloc::Allocator;

impl<T, A: Allocator> Collection for Vec<T, A> {
    type Item = T;
}

impl<T, A: Allocator> CollectionRef for Vec<T, A> {
    type ItemRef<'a>
        = &'a T
    where
        Self: 'a;

    crate::covariant_item_ref!();
}

impl<T, A: Allocator> CollectionMut for Vec<T, A> {
    type ItemMut<'a>
        = &'a mut T
    where
        Self: 'a;

    crate::covariant_item_mut!();
}

impl<T, A: Allocator> SimpleCollectionRef for Vec<T, A> {
    crate::simple_collection_ref!();
}

impl<T, A: Allocator> SimpleCollectionMut for Vec<T, A> {
    crate::simple_collection_mut!();
}

impl<T, A: Allocator> WithCapacityIn<A> for Vec<T, A> {
    #[inline(always)]
    fn with_capacity_in(capacity: usize, allocator: A) -> Self {
        Vec::with_capacity_in(capacity, allocator)
    }
}

impl<T, A: Allocator> Len for Vec<T, A> {
    #[inline(always)]
    fn len(&self) -> usize {
        self.len()
    }

    #[inline(always)]
    fn is_empty(&self) -> bool {
        self.is_empty()
    }
}

impl<T, A: Allocator> Get<usize> for Vec<T, A> {
    #[inline(always)]
    fn get(&self, index: usize) -> Option<&T> {
        self.as_slice().get(index)
    }
}

impl<T, A: Allocator> GetMut<usize> for Vec<T, A> {
    #[inline(always)]
    fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.as_mut_slice().get_mut(index)
    }
}

impl<T, A: Allocator> Capacity for Vec<T, A> {
    #[inline(always)]
    fn capacity(&self) -> usize {
        self.capacity()
    }
}

impl<T, A: Allocator> Reserve for Vec<T, A> {
    #[inline(always)]
    fn reserve(&mut self, additional: usize) {
        self.reserve(additional)
    }
}

impl<T, A: Allocator> PushBack for Vec<T, A> {
    type Output = ();

    #[inline(always)]
    fn push_back(&mut self, t: T) {
        self.push(t)
    }
}

impl<T, A: Allocator> PopBack for Vec<T, A> {
    #[inline(always)]
    fn pop_back(&mut self) -> Option<T> {
        self.pop()
    }
}

impl<T, A: Allocator> Remove<usize> for Vec<T, A> {
    #[inline(always)]
    fn remove(&mut self, index: usize) -> Option<T> {
        if index < self.len() {
            Some(self.remove(index))
        } else {
            None
        }
    }
}

impl<T, A: Allocator> Clear for Vec<T, A> {
    #[inline(always)]
    fn clear(&mut self) {
        self.clear()
    }
}

impl<T, A: Allocator> Iter for Vec<T, A> {
    type Iter<'a>
        = core::slice::Iter<'a, T>
    where
        Self: 'a;

    #[inline(always)]
    fn iter(&self) -> Self::Iter<'_> {
        self.as_slice().iter()
    }
}

impl<T, A: Allocator> IterMut for Vec<T, A> {
    type IterMut<'a>
        = core::slice::IterMut<'a, T>
    where
        Self: 'a;

    #[inline(always)]
    fn iter_mut(&mut self) -> Self::IterMut<'_> {
        self.as_mut_slice().iter_mut()
    }
}
