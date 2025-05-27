#![cfg_attr(all(not(feature = "testing"), not(test)), no_std)]
#![feature(allocator_api)]
#![allow(incomplete_features)]
#![feature(generic_const_exprs)]
#![feature(pointer_is_aligned_to)]
#![feature(slice_ptr_get)]
#![feature(const_trait_impl)]

extern crate alloc;

pub mod io_oracle;
pub mod memory_aux;
pub mod skip_list_quasi_vec;
pub mod system;
pub mod talc;

pub use zk_ee;

#[cfg(test)]
mod tests;
