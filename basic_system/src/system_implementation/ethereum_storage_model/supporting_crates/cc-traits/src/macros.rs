/// Automatically defines the `CollectionRef::upcast_item_ref` function using the
/// covariance of the `ItemRef<'a>` type with regards to `'a`.
///
/// ## Example
///
/// ```
/// use cc_traits::{Collection, CollectionRef, covariant_item_ref};
///
/// pub struct MyVec<T>(Vec<T>);
///
/// impl<T> Collection for MyVec<T> {
///   type Item = T;
/// }
///
/// impl<T> CollectionRef for MyVec<T> {
///   type ItemRef<'a>
///   = &'a T where Self: 'a;
///
///   covariant_item_ref!();
/// }
/// ```
#[macro_export]
macro_rules! covariant_item_ref {
    () => {
        fn upcast_item_ref<'short, 'long: 'short>(r: Self::ItemRef<'long>) -> Self::ItemRef<'short>
        where
            Self: 'long,
        {
            r
        }
    };
}

/// Automatically defines the `CollectionMut::upcast_item_mut` function using the
/// covariance of the `ItemMut<'a>` type with regards to `'a`.
///
/// ## Example
///
/// ```
/// use cc_traits::{Collection, CollectionMut, covariant_item_mut};
///
/// pub struct MyVec<T>(Vec<T>);
///
/// impl<T> Collection for MyVec<T> {
///   type Item = T;
/// }
///
/// impl<T> CollectionMut for MyVec<T> {
///   type ItemMut<'a>
///   = &'a mut T where Self: 'a;
///
///   covariant_item_mut!();
/// }
/// ```
#[macro_export]
macro_rules! covariant_item_mut {
    () => {
        fn upcast_item_mut<'short, 'long: 'short>(r: Self::ItemMut<'long>) -> Self::ItemMut<'short>
        where
            Self: 'long,
        {
            r
        }
    };
}

/// Automatically defines the `KeyedRef::upcast_item_ref` function using the
/// covariance of the `KeyRef<'a>` type with regards to `'a`.
///
/// ## Example
///
/// ```
/// use cc_traits::{Collection, Keyed, KeyedRef, covariant_key_ref};
///
/// pub struct MyMap<K, V>(std::collections::HashMap<K, V>);
///
/// impl<K, V> Collection for MyMap<K, V> {
///   type Item = V;
/// }
///
/// impl<K, V> Keyed for MyMap<K, V> {
///   type Key = K;
/// }
///
/// impl<K, V> KeyedRef for MyMap<K, V> {
///   type KeyRef<'a>
///   = &'a K where Self: 'a;
///
///   covariant_key_ref!();
/// }
/// ```
#[macro_export]
macro_rules! covariant_key_ref {
    () => {
        fn upcast_key_ref<'short, 'long: 'short>(r: Self::KeyRef<'long>) -> Self::KeyRef<'short>
        where
            Self: 'long,
        {
            r
        }
    };
}

/// Automatically defines the `CollectionMut::upcast_item_mut` function using the
/// covariance of the `ItemMut<'a>` type with regards to `'a`.
///
/// ## Example
///
/// ```
/// use cc_traits::{Collection, CollectionMut, SimpleCollectionMut, covariant_item_mut, simple_collection_mut};
///
/// pub struct MyVec<T>(Vec<T>);
///
/// impl<T> Collection for MyVec<T> {
///   type Item = T;
/// }
///
/// impl<T> CollectionMut for MyVec<T> {
///   type ItemMut<'a>
///   = &'a mut T where Self: 'a;
///
///   covariant_item_mut!();
/// }
///
/// impl<T> SimpleCollectionMut for MyVec<T> {
///   simple_collection_mut!();
/// }
/// ```
#[macro_export]
macro_rules! simple_collection_mut {
    () => {
        fn into_mut<'r>(r: Self::ItemMut<'r>) -> &'r mut Self::Item
        where
            Self: 'r,
        {
            r
        }
    };
}

/// Automatically defines the `SimpleCollectionRef::into_ref` function.
///
/// ## Example
///
/// ```
/// use cc_traits::{Collection, CollectionRef, SimpleCollectionRef, covariant_item_ref, simple_collection_ref};
///
/// pub struct MyVec<T>(Vec<T>);
///
/// impl<T> Collection for MyVec<T> {
///   type Item = T;
/// }
///
/// impl<T> CollectionRef for MyVec<T> {
///   type ItemRef<'a>
///   = &'a T where Self: 'a;
///
///   covariant_item_ref!();
/// }
///
/// impl<T> SimpleCollectionRef for MyVec<T> {
///   simple_collection_ref!();
/// }
/// ```
#[macro_export]
macro_rules! simple_collection_ref {
    () => {
        fn into_ref<'r>(r: Self::ItemRef<'r>) -> &'r Self::Item
        where
            Self: 'r,
        {
            r
        }
    };
}

/// Automatically defines the `SimpleKeyedRef::into_ref` function.
///
/// ## Example
///
/// ```
/// use cc_traits::{Collection, Keyed, KeyedRef, SimpleKeyedRef, covariant_key_ref, simple_keyed_ref};
///
/// pub struct MyMap<K, V>(std::collections::HashMap<K, V>);
///
/// impl<K, V> Collection for MyMap<K, V> {
///   type Item = V;
/// }
///
/// impl<K, V> Keyed for MyMap<K, V> {
///   type Key = K;
/// }
///
/// impl<K, V> KeyedRef for MyMap<K, V> {
///   type KeyRef<'a>
///   = &'a K where Self: 'a;
///
///   covariant_key_ref!();
/// }
///
/// impl<K, V> SimpleKeyedRef for MyMap<K, V> {
///   simple_keyed_ref!();
/// }
/// ```
#[macro_export]
macro_rules! simple_keyed_ref {
    () => {
        fn into_ref<'r>(r: Self::KeyRef<'r>) -> &'r Self::Key
        where
            Self: 'r,
        {
            r
        }
    };
}
