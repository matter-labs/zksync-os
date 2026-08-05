#![cfg_attr(all(not(feature = "testing"), not(test)), no_std)]
#![feature(allocator_api)]
#![allow(incomplete_features)]
#![feature(get_mut_unchecked)]
#![feature(vec_push_within_capacity)]
#![feature(ptr_alignment_type)]
#![feature(btreemap_alloc)]
#![feature(unsafe_cell_access)]
#![feature(maybe_uninit_array_assume_init)]
#![feature(ptr_metadata)]
#![feature(slice_from_ptr_range)]
#![feature(split_array)]
#![feature(int_roundings)]
#![allow(clippy::type_complexity)]
#![allow(clippy::new_without_default)]
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::needless_borrow)]
#![allow(clippy::needless_borrows_for_generic_args)]
#![allow(clippy::bool_comparison)]
#![allow(clippy::upper_case_acronyms)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::len_zero)]
#![allow(clippy::result_unit_err)]
#![cfg_attr(
    any(feature = "error_origins", not(target_arch = "riscv32")),
    allow(clippy::result_large_err)
)]
#![cfg_attr(
    any(feature = "error_origins", not(target_arch = "riscv32")),
    allow(clippy::large_enum_variant)
)]

extern crate alloc;

pub mod bootloader;
