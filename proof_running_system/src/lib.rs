#![cfg_attr(all(not(feature = "testing"), not(test)), no_std)]
#![feature(allocator_api)]
#![allow(incomplete_features)]
// Only `global-alloc` builds use `NonNull::<[u8]>::as_mut_ptr` (in
// `system::bootloader`), so gate the feature the same way. Declaring it
// unconditionally trips `unused_features` on host builds that leave the
// allocator out.
#![cfg_attr(feature = "global-alloc", feature(slice_ptr_get))]
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
