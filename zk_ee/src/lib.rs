#![cfg_attr(not(feature = "testing"), no_std)]
#![allow(incomplete_features)]
#![feature(generic_const_exprs)]
#![feature(const_type_id)]
#![feature(allocator_api)]
#![feature(array_chunks)]
#![feature(associated_type_defaults)]
#![feature(get_mut_unchecked)]
#![feature(array_windows)]
#![feature(vec_push_within_capacity)]
#![feature(slice_from_ptr_range)]
#![feature(never_type)]
#![feature(box_into_inner)]
#![feature(btreemap_alloc)]
#![feature(iter_array_chunks)]
#![feature(pointer_is_aligned_to)]
#![feature(const_trait_impl)]
#![feature(btree_cursors)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::result_unit_err)]
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::type_complexity)]
#![allow(clippy::needless_borrow)]
#![allow(clippy::double_must_use)]
#![allow(clippy::bool_comparison)]
#![allow(clippy::borrow_deref_ref)]
#![allow(clippy::len_without_is_empty)]
#![allow(clippy::needless_return)]
#![allow(clippy::wrong_self_convention)]
#![cfg_attr(
    any(feature = "error_origins", not(target_arch = "riscv32")),
    allow(clippy::result_large_err)
)]
#![cfg_attr(
    any(feature = "error_origins", not(target_arch = "riscv32")),
    allow(clippy::large_enum_variant)
)]

extern crate alloc;

pub mod common_structs;
pub mod common_traits;
pub mod execution_environment_type;
pub mod kv_markers;
pub mod memory;
pub mod oracle;
pub mod reference_implementations;
pub mod system;
pub mod system_io_oracle;
pub mod types_config;
pub mod utils;
