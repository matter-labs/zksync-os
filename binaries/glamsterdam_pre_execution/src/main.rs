//! The Glamsterdam pre-execution check as a guest program: witnesses the
//! header, its parent and the block access list, carries the parent state
//! root to the header's through the access list, and returns the hash of its
//! linkage output (`ethereum_glamsterdam::linkage::ProgramOutput`).

#![no_std]
#![no_main]
#![allow(incomplete_features)]
#![feature(allocator_api)]

use proof_running_system::system::bootloader::{
    init_allocator, run_glamsterdam_state_consistency, OptionalGlobalAllocator,
};

/// Uses CSR (control & status register) to communicate with the outside oracle.
mod csr {
    use airbender::rt::sys::{read_word, write_word};

    #[derive(Clone, Copy, Debug)]
    pub struct CSRBasedNonDeterminismSource;

    impl proof_running_system::io_oracle::NonDeterminismCSRSourceImplementation
        for CSRBasedNonDeterminismSource
    {
        #[inline(always)]
        fn csr_read_impl() -> usize {
            const {
                assert!(core::mem::size_of::<usize>() == core::mem::size_of::<u32>());
            }
            read_word() as usize
        }
        #[inline(always)]
        #[allow(clippy::unit_arg)]
        fn csr_write_impl(value: usize) {
            core::hint::black_box(write_word(value as u32))
        }
    }
}

#[global_allocator]
static GLOBAL_ALLOC: OptionalGlobalAllocator = OptionalGlobalAllocator;

#[airbender::main(allocator_init = init_allocator)]
fn main() -> [u32; 8] {
    run_glamsterdam_state_consistency::<csr::CSRBasedNonDeterminismSource>()
}
