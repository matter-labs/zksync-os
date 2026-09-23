#![feature(allocator_api)]
#![allow(clippy::new_without_default)]
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]
#![cfg_attr(
    any(feature = "error_origins", not(target_arch = "riscv32")),
    allow(clippy::result_large_err)
)]

// this environment can have access to databases, internet, p2p, whatever, so
// it's oracle implementation is assumed to do exactly so, and all allocator work can be just degraded
// to system allocator and reallocations instead carefully work with sparse memory, but we can anyway implement
// such sparse memory once and for all

pub mod run;
pub mod system;
