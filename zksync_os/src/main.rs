#![no_std]
#![no_main]
#![allow(incomplete_features)]
#![feature(allocator_api)]

use proof_running_system::system::bootloader::{init_allocator, run_proving};

#[cfg(all(feature = "external-rv64-runtime", target_arch = "riscv64"))]
#[path = "../../../rv64-port/runtime.rs"]
mod runtime;

/// Uses CSR (control & status register) to communicate with outside oracle.
mod csr {
    #[cfg(all(feature = "external-rv64-runtime", target_arch = "riscv64"))]
    use crate::runtime::{read_word, write_word};
    #[cfg(not(all(feature = "external-rv64-runtime", target_arch = "riscv64")))]
    use airbender::rt::sys::{read_word, write_word};

    #[derive(Clone, Copy, Debug)]
    pub struct CSRBasedNonDeterminismSource;

    impl proof_running_system::io_oracle::NonDeterminismCSRSourceImplementation
        for CSRBasedNonDeterminismSource
    {
        #[inline(always)]
        fn csr_read_impl() -> usize {
            #[cfg(not(all(feature = "external-rv64-runtime", target_arch = "riscv64")))]
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

pub use self::csr::CSRBasedNonDeterminismSource;

#[cfg(feature = "print_debug_info")]
pub mod quasi_uart;

#[cfg(not(feature = "print_debug_info"))]
type LoggerTy = proof_running_system::zk_ee::system::NullLogger;

#[cfg(feature = "print_debug_info")]
type LoggerTy = crate::quasi_uart::QuasiUartLogger;

use proof_running_system::system::bootloader::OptionalGlobalAllocator;
#[global_allocator]
static GLOBAL_ALLOC: OptionalGlobalAllocator = OptionalGlobalAllocator;

#[cfg_attr(
    not(all(feature = "external-rv64-runtime", target_arch = "riscv64")),
    airbender::main(allocator_init = init_allocator)
)]
fn main() -> [u32; 8] {
    run_proving::<CSRBasedNonDeterminismSource, LoggerTy>()
}
