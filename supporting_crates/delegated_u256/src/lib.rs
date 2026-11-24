#![cfg_attr(not(test), no_std)]

// The integer of the bigint delegation of the RV32 guest, and off RV32 the software model of
// that delegation for hosts. An RV64 guest has no delegation and must not carry its model:
// nothing of it is compiled there, so a use on RV64 does not build.
#[cfg(not(target_arch = "riscv64"))]
mod arithmetic;
pub mod byte_order;
#[cfg(not(target_arch = "riscv64"))]
mod copy;
#[cfg(not(target_arch = "riscv64"))]
mod delegation;
#[cfg(not(target_arch = "riscv64"))]
mod utils;

#[cfg(not(target_arch = "riscv64"))]
#[allow(clippy::derived_hash_with_manual_eq)]
#[derive(Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(align(32))]
pub struct DelegatedU256([u64; 4]);

#[cfg(not(target_arch = "riscv64"))]
pub use arithmetic::*;
#[cfg(not(target_arch = "riscv64"))]
pub use copy::*;
#[cfg(not(target_arch = "riscv64"))]
pub use delegation::*;
