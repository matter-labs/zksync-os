#![cfg_attr(not(test), no_std)]
#![feature(never_type)]
#![feature(exhaustive_patterns)]
#![feature(associated_type_defaults)]
#![feature(allocator_api)]
#![feature(vec_push_within_capacity)]
#![feature(btreemap_alloc)]

pub mod constants;
pub mod leb128;
pub mod parsers;
pub mod routines;
pub mod types;
pub mod utils;

extern crate alloc;

#[cfg(test)]
pub mod tester;
