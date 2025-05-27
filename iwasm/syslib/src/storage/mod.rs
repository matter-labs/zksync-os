use core::{cell::UnsafeCell, marker::PhantomData};

use crate::{
    qol::{PipeOp, UnsafeCellEx},
    types::ints::U256BE,
};

pub mod impls;
pub mod kinds;
pub mod entries;

pub struct Storage {}

pub trait Storeable {
    const SIZED: bool;
    /// Offset defines the offset from the right (lower order) at which to write the value when it
    /// is smaller than U256.
    fn to_u256_be(&self, offset: u8) -> U256BE;

    fn from_u256_be(value: &U256BE, offset: u8) -> Self;

    // fn hash_into<S: sha3::Digest>(&self, state: &mut S) {
    //     match Self::SIZED {
    //         true => {
    //             self.to_u256_be(0).as_bytes().to(|x| state.update(x));
    //         }
    //         false => todo!(),
    //     }
    // }

    fn from_key<F: Fn(&U256BE) -> Self>(f: F, key: &U256BE) -> Self;
}

pub trait StoreableIndirection {
    fn from_key(key: &U256BE) -> Self;

}
