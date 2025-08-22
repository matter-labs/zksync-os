#![cfg_attr(not(test), no_std)]

mod arithmetic;
mod copy;
mod delegation;
mod utils;

#[allow(clippy::derived_hash_with_manual_eq)]
#[derive(Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(align(32))]
pub struct DelegatedU256([u64; 4]);

pub use arithmetic::*;
pub use copy::*;
pub use delegation::*;

pub fn init() {
    arithmetic::init();
}
