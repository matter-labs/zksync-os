#![no_std]
#![no_main]
#![allow(incomplete_features)]
#![feature(allocator_api)]

use proof_running_system::system::bootloader::{init_allocator, run_proving};

mod csr {
    pub struct CsrWordSource;

    impl proof_running_system::io_oracle::WordSource for CsrWordSource {
        #[inline(always)]
        fn read_word(&mut self) -> u32 {
            airbender::rt::sys::read_word()
        }
    }
}

pub use self::csr::CsrWordSource;

#[cfg(feature = "print_debug_info")]
pub mod quasi_uart;

#[cfg(not(feature = "print_debug_info"))]
type LoggerTy = proof_running_system::zk_ee::system::NullLogger;

#[cfg(feature = "print_debug_info")]
type LoggerTy = crate::quasi_uart::QuasiUartLogger;

use proof_running_system::system::bootloader::OptionalGlobalAllocator;
#[global_allocator]
static GLOBAL_ALLOC: OptionalGlobalAllocator = OptionalGlobalAllocator;

#[airbender::main(allocator_init = init_allocator)]
fn main() -> [u32; 8] {
    run_proving::<CsrWordSource, LoggerTy>()
}
