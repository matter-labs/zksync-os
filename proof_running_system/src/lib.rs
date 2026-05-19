#![cfg_attr(all(not(feature = "testing"), not(test)), no_std)]
#![feature(allocator_api)]
#![allow(incomplete_features)]
#![feature(pointer_is_aligned_to)]
#![feature(slice_ptr_get)]
#![feature(const_trait_impl)]
#![feature(unsafe_cell_access)]
#![feature(min_specialization)]

extern crate alloc;

pub mod proving_oracle;
pub mod system;
pub mod talc;

pub use zk_ee;

#[cfg(test)]
mod tests;
