#![allow(incomplete_features)]
#![cfg_attr(all(not(test)), no_std)]
#![feature(alloc_layout_extra)]
#![feature(allocator_api)]
#![feature(clone_to_uninit)]
#![feature(generic_const_exprs)]
#![feature(maybe_uninit_write_slice)]

extern crate alloc;

// use self::interface::SystemInterface;

pub mod abi;
// pub mod interface;
pub mod sys;
pub mod types;

#[cfg(test)]
mod tests;

pub(crate) mod qol;
pub mod storage;

use alloc::vec::Vec;

#[cfg(not(target_arch = "wasm32"))]
mod impl_native;

#[cfg(not(target_arch = "wasm32"))]
mod impl_arch {
    pub use super::impl_native::*;
}

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
mod impl_wasm;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
mod impl_arch {
    pub use super::impl_wasm::*;
}

pub use impl_arch::*;
pub use syslib_derive::*;

#[macro_export]
macro_rules! dev {
    () => {
        cfg!(feature = "dev")
    };
}
