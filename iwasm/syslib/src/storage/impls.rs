use crate::types::{
    ints::U256BE,
    uintx::{size_bound, Assert, IntX, IsTrue},
};

use super::{ kinds::Mapping, Storeable, StoreableIndirection};

impl<const N: usize> Storeable for IntX<N, crate::types::uintx::LE>
where
    Assert<{ size_bound(N) }>: IsTrue,
{
    const SIZED: bool = true;

    fn to_u256_be(&self, _offset: u8) -> crate::types::ints::U256BE {
        self.to_be().into_u256()
    }

    fn from_key<F: Fn(&crate::types::ints::U256BE) -> Self>(
        f: F,
        key: &crate::types::ints::U256BE,
    ) -> Self {
        f(key)
    }

    fn from_u256_be(value: &U256BE, _offset: u8) -> Self {
        value.to_le().into_size()
    }
}

impl<const N: usize> Storeable for IntX<N, crate::types::uintx::BE>
where
    Assert<{ size_bound(N) }>: IsTrue,
{
    const SIZED: bool = true;

    fn to_u256_be(&self, _offset: u8) -> U256BE {
        self.clone().into_u256()
    }

    fn from_key<F: Fn(&U256BE) -> Self>(f: F, key: &U256BE) -> Self {
        f(key)
    }

    fn from_u256_be(value: &U256BE, _offset: u8) -> Self {
        value.clone().into_size()
    }
}

impl<K: Storeable, V: Storeable> StoreableIndirection for Mapping<'_, K, V> {
    fn from_key(key: &U256BE) -> Self {
        Self::instantiate(key)
    }
}

// impl<K: Storeable, V: Storeable> StoreableIndirection for Mapping<'_, K, Mapping<'_, K, V>> {
// }

// impl<K: Storeable, V: Storeable> Storeable for Mapping<'_, K, V> {
//     const SIZED: bool = false;
//
//     fn to_u256_be(&self, _offset: u8) -> crate::types::ints::U256BE {
//         // TODO: mapping can't serve as key, create new trait for keys
//         todo!()
//     }
//
//     fn from_key<F: Fn(&crate::types::ints::U256BE) -> Self>(
//         _f: F,
//         key: &crate::types::ints::U256BE,
//     ) -> Self {
//         Self::instantiate(key)
//     }
//
//     fn from_u256_be(_value: &U256BE, _offset: u8) -> Self {
//         todo!()
//     }
// }
