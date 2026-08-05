// NOTE: cc-traits gives almost everything that we need for our collections,
// with few small fixes: use of core in aliases (not mandatory), implementation for notable traits for
// `alloc` structures with non-Global allocator, and addition of WithCapacityIn trait. To reflect explicit allocator
// use a new `allocator-api` feature was added

//! This crate provide traits to describe common operations available on data structures.
//! This is particularly useful when building new types on top of generic data structures without relying on the actual implementation of the underlying data structure.
//!
//! Here is an example of the kind of traits provided by this crate:
//! ```rust
//! # use cc_traits::Collection;
//! /// Mutable collection where new elements can be inserted.
//! pub trait Insert: Collection {
//!     /// The output of the insertion function.
//!     type Output;
//!
//!     /// Insert a new element in the collection.
//!     fn insert(&mut self, element: Self::Item) -> Self::Output;
//! }
//! ```
//!
//! # Usage
//!
//! Such traits can be used to define collections with special properties,
//! independently of the actual internal data structure.
//! For instance the following code defines an `Ordered<S>` stack collection,
//! guarantying the well-sortedness of the elements in the stack.
//!
//! ```rust
//! use cc_traits::{
//!     Collection,
//!     Back,
//!     PushBack
//! };
//!
//! /// Ordered stack.
//! pub struct Ordered<S> {
//!     inner: S
//! }
//!
//! impl<S> Ordered<S> {
//!     pub fn new() -> Self where S: Default {
//!         Ordered {
//!             inner: S::default()
//!         }
//!     }
//! }
//!
//! impl<S> Ordered<S> {
//!     /// Push the given element on the stack iff it is grater or equal
//!     /// to every other element already in the stack.
//!     pub fn try_push<T>(&mut self, element: T) -> Result<(), T>
//!     where
//!         T: PartialOrd,
//!         S: Collection<Item=T> + Back + PushBack, // `S` must be a stack providing `back` and `push_back`.
//!         for<'a> S::ItemRef<'a>: PartialOrd<&'a T> // The reference type must be comparable with other reference types.
//!     {
//!         if self.inner.back().map(|back| back <= &element).unwrap_or(true) {
//!             self.inner.push_back(element);
//!             Ok(())
//!         } else {
//!             Err(element)
//!         }
//!     }
//! }
//!
//! #[cfg(feature = "std")]
//! fn ordered_stack_usage<S>()
//! where
//!     S: Default + Collection<Item = i32> + Back + PushBack,
//!     for<'a> S::ItemRef<'a>: PartialOrd<&'a i32>,
//! {
//!     let mut ordered: Ordered<S> = Ordered::new();
//!     assert!(ordered.try_push(1).is_ok());
//!     assert!(ordered.try_push(2).is_ok());
//!     assert!(ordered.try_push(0).is_err());
//! }
//!
//! #[cfg(feature = "std")]
//! ordered_stack_usage::<Vec<i32>>(); // a `Vec` is a stack so it works.
//!
//! #[cfg(feature = "std")]
//! use std::collections::VecDeque;
//! #[cfg(feature = "std")]
//! ordered_stack_usage::<VecDeque<i32>>(); // a `VecDeque` is also a stack.
//! ```
//!
//! # Trait aliases
//!
//! By enabling the `nightly` you can get access to
//! some trait alias definitions that can be useful to reduce the
//! verbosity of your code.
//! Here is an example of such aliases defining the common interface of stacks:
//! ```ignore
//! pub trait Stack<T> = Collection<Item=T> + Len + Back;
//! pub trait StackMut<T> = Stack<T> + BackMut + PushBack + PopBack;
//! ```
//!
//! As of version 0.8.0, those traits are also available without the `nightly`
//! feature as regular trait definitions.
//!
//! # Standard library
//!
//! By default, all the traits defined in this crate are implemented (when relevant)
//! for the standard library collections.
//! You can disable it by using the `nostd` feature.
//!
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "nightly", feature(trait_alias))]
#![cfg_attr(feature = "allocator_api", feature(allocator_api))]

#[cfg(feature = "alloc")]
extern crate alloc;

extern crate core;

mod impls;
mod macros;

#[cfg(feature = "nightly")]
mod alias;
#[cfg(feature = "nightly")]
pub use alias::*;

#[cfg(not(feature = "nightly"))]
mod non_alias;
#[cfg(not(feature = "nightly"))]
pub use non_alias::*;

use core::ops::{Deref, DerefMut};

/// Abstract collection.
pub trait Collection {
    /// Type of the items of the collection.
    type Item;
}

/// Abstract collection that can be immutably referenced.
pub trait CollectionRef: Collection {
    /// Type of references to items of the collection.
    type ItemRef<'a>: Clone + Deref<Target = Self::Item>
    where
        Self: 'a;

    /// Changes an item reference into a shorter lived reference.
    ///
    /// Item references are [covariant](https://doc.rust-lang.org/nomicon/subtyping.html#variance) with
    /// regard to the defined lifetime parameter `'a`.
    /// Since this cannot be directly expressed by the type system, this associated function
    /// allows one to explicitly shorten the reference's lifetime.
    ///
    /// You can use the [`covariant_item_ref!`] macro to automatically
    /// implement this function.
    fn upcast_item_ref<'short, 'long: 'short>(r: Self::ItemRef<'long>) -> Self::ItemRef<'short>
    where
        Self: 'long;
}

/// Collection where each item reference can be converted into a standard
/// "simple" rust reference.
///
/// This trait is particularly useful to avoid having to include where bounds
/// of the form `for<'r> T::ItemRef<'r>: Into<&'r T::Item>`, which can
/// currently lead the compiler to try to prove `T: 'static`
/// (see https://github.com/rust-lang/rust/pull/96709#issuecomment-1182403490)
/// for more details.
pub trait SimpleCollectionRef: CollectionRef {
    fn into_ref<'r>(r: Self::ItemRef<'r>) -> &'r Self::Item
    where
        Self: 'r;
}

/// Abstract collection that can be mutably referenced.
pub trait CollectionMut: Collection {
    /// Type of mutable references to items of the collection.
    type ItemMut<'a>: DerefMut<Target = Self::Item>
    where
        Self: 'a;

    /// Changes an item mutable reference into a shorter lived mutable reference.
    ///
    /// See the [`CollectionRef::upcast_item_ref`] function for more information.
    /// You can use the [`covariant_item_mut!`] macro to automatically
    /// implement this function.
    fn upcast_item_mut<'short, 'long: 'short>(r: Self::ItemMut<'long>) -> Self::ItemMut<'short>
    where
        Self: 'long;
}

/// Collection where each item reference can be converted into a standard
/// "simple" rust reference.
///
/// This trait is particularly useful to avoid having to include where bounds
/// of the form `for<'r> T::ItemMut<'r>: Into<&'r mut T::Item>`, which can
/// currently lead the compiler to try to prove `T: 'static`
/// (see https://github.com/rust-lang/rust/pull/96709#issuecomment-1182403490)
/// for more details.
pub trait SimpleCollectionMut: CollectionMut {
    fn into_mut<'r>(r: Self::ItemMut<'r>) -> &'r mut Self::Item
    where
        Self: 'r;
}

/// Abstract keyed collection.
pub trait Keyed: Collection {
    /// Type of the keys indexing each item of the collection.
    type Key;
}

/// Abstract keyed collection whose key can be referenced.
pub trait KeyedRef: Keyed {
    /// Type of references to keys of the collection.
    type KeyRef<'a>: Clone + Deref<Target = Self::Key>
    where
        Self: 'a;

    /// Changes a key reference into a shorter lived reference.
    ///
    /// See the [`CollectionRef::upcast_item_ref`] function for more information.
    /// You can use the [`covariant_key_ref!`] macro to automatically
    /// implement this function.
    fn upcast_key_ref<'short, 'long: 'short>(r: Self::KeyRef<'long>) -> Self::KeyRef<'short>
    where
        Self: 'long;
}

/// Keyed collection where each key reference can be converted into a standard
/// "simple" rust reference.
///
/// This trait is particularly useful to avoid having to include where bounds
/// of the form `for<'r> T::KeyRef<'r>: Into<&'r T::Key>`, which can
/// currently lead the compiler to try to prove `T: 'static`
/// (see https://github.com/rust-lang/rust/pull/96709#issuecomment-1182403490)
/// for more details.
pub trait SimpleKeyedRef: KeyedRef {
    fn into_ref<'r>(r: Self::KeyRef<'r>) -> &'r Self::Key
    where
        Self: 'r;
}

/// Collection that can be created with a minimum given capacity.
pub trait WithCapacity {
    /// Creates a new instance of `Self` with the given minimum capacity.
    fn with_capacity(capacity: usize) -> Self;
}

#[cfg(feature = "allocator_api")]
/// Collection that can be created with a minimum given capacity in some allocator.
pub trait WithCapacityIn<A: core::alloc::Allocator> {
    /// Creates a new instance of `Self` with the given minimum capacity.
    fn with_capacity_in(capacity: usize, allocator: A) -> Self;
}

/// Sized collection.
pub trait Len {
    /// Returns the number of elements in the collection.
    fn len(&self) -> usize;

    /// Checks if the collection is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Collection with known capacity.
pub trait Capacity {
    /// Returns the current capacity of the collection.
    ///
    /// This corresponds to the number of elements the collection can hold without reallocation.
    fn capacity(&self) -> usize;
}

/// Collection that can extend their capacity.
pub trait Reserve {
    /// Reserve enough memory for `edditional` more elements.
    fn reserve(&mut self, additional: usize);
}

/// Queryable collection.
pub trait Get<T>: CollectionRef {
    /// Returns a reference to the item stored behind the given key (if any).
    fn get(&self, key: T) -> Option<Self::ItemRef<'_>>;

    /// Checks if the collection contains an item behind the given key.
    fn contains(&self, key: T) -> bool {
        self.get(key).is_some()
    }
}

/// Mutably queryable collection.
pub trait GetMut<T>: Get<T> + CollectionMut {
    /// Returns a mutable reference to the item stored behind the given key (if any).
    fn get_mut(&mut self, key: T) -> Option<Self::ItemMut<'_>>;
}

/// Queryable map.
pub trait GetKeyValue<T>: CollectionRef + KeyedRef {
    /// Returns the key-value pair matching the given `key`.
    fn get_key_value(&self, key: T) -> Option<(Self::KeyRef<'_>, Self::ItemRef<'_>)>;
}

/// Mutably queryable map.
pub trait GetKeyValueMut<T>: CollectionMut + KeyedRef {
    /// Returns the key-value pair matching the given `key`, with a mutable reference to the value.
    fn get_key_value_mut(&mut self, key: T) -> Option<(Self::KeyRef<'_>, Self::ItemMut<'_>)>;
}

/// Collection exposing a reference to its front element.
pub trait Front: CollectionRef {
    /// Get a reference to the front element of the collection.
    fn front(&self) -> Option<Self::ItemRef<'_>>;
}

impl<T: Get<usize> + Len> Front for T {
    fn front(&self) -> Option<Self::ItemRef<'_>> {
        match self.len() {
            0 => None,
            _ => self.get(0),
        }
    }
}

/// Collection exposing a reference to its back element.
pub trait Back: CollectionRef {
    /// Get a reference to the back element of the collection.
    fn back(&self) -> Option<Self::ItemRef<'_>>;
}

impl<T: Get<usize> + Len> Back for T {
    fn back(&self) -> Option<Self::ItemRef<'_>> {
        match self.len() {
            0 => None,
            l => self.get(l - 1),
        }
    }
}

/// Collection exposing a mutable reference to its front element.
pub trait FrontMut: CollectionMut {
    /// Get a mutable reference to the front element of the collection.
    fn front_mut(&mut self) -> Option<Self::ItemMut<'_>>;
}

impl<T: GetMut<usize> + Len> FrontMut for T {
    fn front_mut(&mut self) -> Option<Self::ItemMut<'_>> {
        match self.len() {
            0 => None,
            _ => self.get_mut(0),
        }
    }
}

/// Collection exposing a mutable reference to its back element.
pub trait BackMut: CollectionMut {
    /// Get a mutable reference to the back element of the collection.
    fn back_mut(&mut self) -> Option<Self::ItemMut<'_>>;
}

impl<T: GetMut<usize> + Len> BackMut for T {
    fn back_mut(&mut self) -> Option<Self::ItemMut<'_>> {
        match self.len() {
            0 => None,
            l => self.get_mut(l - 1),
        }
    }
}

/// Mutable collection where new elements can be inserted.
pub trait Insert: Collection {
    /// The output of the insertion function.
    type Output;

    /// Insert a new element in the collection.
    fn insert(&mut self, element: Self::Item) -> Self::Output;
}

/// Mutable map where new new key-value pairs can be inserted.
pub trait MapInsert<K>: Collection {
    /// The output of the insertion function.
    type Output;

    /// Insert a new key-value pair in the collection.
    fn insert(&mut self, key: K, value: Self::Item) -> Self::Output;
}

/// Mutable collection where new elements can be pushed on the front.
pub trait PushFront: Collection {
    /// The output of the push function.
    type Output;

    /// Push a new element on the front of the collection.
    fn push_front(&mut self, element: Self::Item) -> Self::Output;
}

/// Mutable collection where new elements can be pushed on the back.
pub trait PushBack: Collection {
    /// The output of the push function.
    type Output;

    /// Push a new element on the back of the collection.
    fn push_back(&mut self, element: Self::Item) -> Self::Output;
}

/// Mutable collection where elements can be removed from.
pub trait Remove<T>: Collection {
    /// Remove the element identified by the given `key`.
    fn remove(&mut self, key: T) -> Option<Self::Item>;
}

/// Mutable collection where elements can be popped from the front.
pub trait PopFront: Collection {
    /// Remove the front element of the collection and return it (if any).
    fn pop_front(&mut self) -> Option<Self::Item>;
}

/// Mutable collection where elements can be popped from the back.
pub trait PopBack: Collection {
    /// Remove the back element of the collection and return it (if any).
    fn pop_back(&mut self) -> Option<Self::Item>;
}

/// Clearable collection.
pub trait Clear {
    /// Remove all the elements of the collection.
    fn clear(&mut self);
}

/// Iterable collection.
pub trait Iter: CollectionRef {
    /// Iterator type.
    type Iter<'a>: Iterator<Item = Self::ItemRef<'a>>
    where
        Self: 'a;

    /// Create an iterator over the items of the collection.
    fn iter(&self) -> Self::Iter<'_>;
}

/// Mutably iterable collection.
pub trait IterMut: CollectionMut {
    /// Iterator type.
    type IterMut<'a>: Iterator<Item = Self::ItemMut<'a>>
    where
        Self: 'a;

    /// Create an iterator over the mutable items of the collection.
    fn iter_mut(&mut self) -> Self::IterMut<'_>;
}

pub trait MapIter: KeyedRef + CollectionRef {
    type Iter<'a>: Iterator<Item = (Self::KeyRef<'a>, Self::ItemRef<'a>)>
    where
        Self: 'a;

    fn iter(&self) -> Self::Iter<'_>;
}

pub trait MapIterMut: KeyedRef + CollectionMut {
    type IterMut<'a>: Iterator<Item = (Self::KeyRef<'a>, Self::ItemMut<'a>)>
    where
        Self: 'a;

    fn iter_mut(&mut self) -> Self::IterMut<'_>;
}
