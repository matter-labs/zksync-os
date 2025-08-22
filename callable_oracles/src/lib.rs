#![cfg_attr(all(not(feature = "evaluate"), not(test)), no_std)]
#![allow(incomplete_features)]
#![feature(generic_const_exprs)]
#![feature(array_chunks)]
#![allow(clippy::new_without_default)]
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::needless_borrow)]
#![allow(clippy::needless_borrows_for_generic_args)]
#![allow(clippy::bool_comparison)]
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::result_unit_err)]
#![allow(clippy::double_must_use)]
#![allow(clippy::explicit_auto_deref)]
#![allow(clippy::assertions_on_constants)]
#![allow(clippy::borrow_deref_ref)]
#![allow(clippy::op_ref)]
#![allow(clippy::precedence)]

pub mod arithmetic;
pub mod utils;

use zk_ee::{
    kv_markers::{ExactSizeChain, UsizeDeserializable, UsizeSerializable},
    system::errors::internal::InternalError,
};

pub mod hash_to_prime;

#[derive(Clone, Copy, Debug)]
pub struct MemoryRegionDescriptionParams {
    pub offset: u32,
    pub len: u32,
}

impl UsizeSerializable for MemoryRegionDescriptionParams {
    const USIZE_LEN: usize = <u32 as UsizeSerializable>::USIZE_LEN * 2;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        ExactSizeChain::new(
            UsizeSerializable::iter(&self.offset),
            UsizeSerializable::iter(&self.len),
        )
    }
}

impl UsizeDeserializable for MemoryRegionDescriptionParams {
    const USIZE_LEN: usize = <Self as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let offset = <u32 as UsizeDeserializable>::from_iter(src)?;
        let len = <u32 as UsizeDeserializable>::from_iter(src)?;

        let new = Self { offset, len };

        Ok(new)
    }
}
