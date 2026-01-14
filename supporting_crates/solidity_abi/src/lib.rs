#![cfg_attr(not(test), no_std)]
#![allow(incomplete_features)]
#![feature(generic_const_exprs)]
#![feature(allocator_api)]
#![allow(clippy::result_unit_err)]
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::bool_comparison)]
#![allow(clippy::len_without_is_empty)]
#![allow(clippy::needless_borrow)]

extern crate alloc;

use crate::codable_trait::SolidityCodableReflectionRef;
use codable_trait::SolidityDecodable;

pub mod codable_trait;
pub mod impls;

pub use solidity_abi_derive;

pub fn abi_decode<'a, T: SolidityDecodable>(src: &'a [u8]) -> Result<T::ReflectionRef<'a>, ()> {
    let mut head_offset = 0;
    let el = T::ReflectionRef::parse(src, &mut head_offset)?;

    Ok(el)
}

// #[cfg(test)]
// mod tests;
