use crate::*;
use core::ops::{Index, IndexMut};

/// Collection with mutable capacity.
pub trait CapacityMut: Capacity + Reserve {}

impl<C: Capacity + Reserve> CapacityMut for C {}

/// Immutable stack data structure.
///
/// A stack provides two main operations:
///   - [`PushBack::push_back`], which adds an element to the collection, and
///   - [`PopBack::pop_back`], which removes the most recently added element that was not yet removed.
///
/// This trait alias describes the immutables operations derived from the two main operation above:
///   - [`Len::len`], returning the number of elements in the stack, and
///   - [`Back::back`], returning a reference to the most recently added element that was not yet removed.
pub trait Stack<T>: Collection<Item = T> + Len + Back {}

impl<T, C: Collection<Item = T> + Len + Back> Stack<T> for C {}

/// Mutable stack data structure.
///
/// This trait alias describes the mutables operations on a stack.
/// See [`Stack`] for more details.
pub trait StackMut<T>: Stack<T> + BackMut + PushBack + PopBack {}

impl<T, C: Stack<T> + BackMut + PushBack + PopBack> StackMut<T> for C {}

/// Immutable array data structure (conventionally named "Vec").
///
/// A Vec is essentially a [`Stack`] indexable by a `usize`.
pub trait Vec<T>: Stack<T> + Index<usize, Output = T> {}

impl<T, C: Stack<T> + Index<usize, Output = T>> Vec<T> for C {}

/// Mutable Vec data structure.
///
/// This trait alias describes the mutables operations on a Vec.
/// See [`Vec`] for more details.
pub trait VecMut<T>: Vec<T> + StackMut<T> + IndexMut<usize> {}

impl<T, C: Vec<T> + StackMut<T> + IndexMut<usize>> VecMut<T> for C {}

/// Immutable double-ended queue.
///
/// A double-ended queue (abbreviated to deque) is a generalization of a stack in which
/// elements can be added and removed from both ends.
/// Such a data structure provides two additional operations compared to regular stacks:
///   - [`PushFront::push_front`], which adds an element to the front of collection, and
///   - [`PopFront::pop_front`], which removes the front element of the collection.
///
/// This trait alias describes the immutables operations available on deques.
pub trait Deque<T>: Stack<T> + Front {}

impl<T, C: Stack<T> + Front> Deque<T> for C {}

/// Mutable double-ended queue.
///
/// This trait alias describes the mutables operations on a deque.
/// See [`Deque`] for more details.
pub trait DequeMut<T>: StackMut<T> + FrontMut + PushFront + PopFront {}

impl<T, C: StackMut<T> + FrontMut + PushFront + PopFront> DequeMut<T> for C {}

/// Immutable indexable deque.
///
/// See [`Deque`] and [`Vec`] for more details.
pub trait VecDeque<T>: Deque<T> + Vec<T> {}

impl<T, C: Deque<T> + Vec<T>> VecDeque<T> for C {}

/// Mutable indexable deque.
///
/// See [`VecDeque`], [`DequeMut`] and [`VecMut`] for more details.
pub trait VecDequeMut<T>: VecDeque<T> + DequeMut<T> + VecMut<T> {}

impl<T, C: VecDeque<T> + DequeMut<T> + VecMut<T>> VecDequeMut<T> for C {}

/// Imutable set data structure.
///
/// A set is an unordered collection storing at most one single copy of each element.
pub trait Set<T>: Collection<Item = T> + Len + for<'a> Get<&'a T> {}

impl<T, C: Collection<Item = T> + Len + for<'a> Get<&'a T>> Set<T> for C {}

/// Mutable set data structure.
pub trait SetMut<T>: Set<T> + Insert<Output = bool> + for<'a> Remove<&'a T> {}

impl<T, C: Set<T> + Insert<Output = bool> + for<'a> Remove<&'a T>> SetMut<T> for C {}

/// Imutable map data structure.
///
/// A map is an unordered collection storing key-value pairs, indexed by the key.
pub trait Map<K, V>:
    Keyed<Key = K, Item = V> + Len + for<'a> Get<&'a K> + for<'a> GetKeyValue<&'a K>
{
}

impl<K, V, C: Keyed<Key = K, Item = V> + Len + for<'a> Get<&'a K> + for<'a> GetKeyValue<&'a K>>
    Map<K, V> for C
{
}

/// Mutable map data structure.
pub trait MapMut<K, V>:
    Map<K, V> + for<'a> GetMut<&'a K> + MapInsert<K, Output = Option<V>> + for<'a> Remove<&'a K>
{
}

impl<
        K,
        V,
        C: Map<K, V>
            + for<'a> GetMut<&'a K>
            + MapInsert<K, Output = Option<V>>
            + for<'a> Remove<&'a K>,
    > MapMut<K, V> for C
{
}

/// Imutable slab data structure.
///
/// A slab is a linear collection storing each element at a given index.
/// The index of the element is allocated and returned upon insertion.
pub trait Slab<T>: Collection<Item = T> + Len + Get<usize> {}

impl<T, C: Collection<Item = T> + Len + Get<usize>> Slab<T> for C {}

/// Mutable slab data structure.
pub trait SlabMut<T>: Slab<T> + GetMut<usize> + Insert<Output = usize> + Remove<usize> {}

impl<T, C: Slab<T> + GetMut<usize> + Insert<Output = usize> + Remove<usize>> SlabMut<T> for C {}
