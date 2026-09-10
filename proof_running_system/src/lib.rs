#![cfg_attr(all(not(feature = "testing"), not(test)), no_std)]
#![feature(allocator_api)]
#![allow(incomplete_features)]
// Used by `system::bootloader` under the guest build configuration, which the
// host workspace lint run does not exercise.
#![feature(slice_ptr_get)]
#![feature(pointer_is_aligned_to)]
#![feature(const_trait_impl)]
#![feature(unsafe_cell_access)]

extern crate alloc;

pub mod io_oracle;
pub mod system;
pub mod talc;

pub use zk_ee;

#[cfg(test)]
mod tests;
