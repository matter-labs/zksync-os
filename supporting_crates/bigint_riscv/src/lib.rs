#![cfg_attr(not(test), no_std)]

mod arithmetic;
mod copy;
mod delegation;
mod utils;

#[derive(Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(align(32))]
pub struct DelegatedU256([u64; 4]);

pub fn init() {
    arithmetic::init();
}

pub use arithmetic::*;
pub use copy::*;
