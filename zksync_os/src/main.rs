#![no_std]
#![no_main]
#![allow(incomplete_features)]
#![feature(allocator_api)]

use proof_running_system::system::bootloader::{init_allocator, run_proving};

mod word_source {
    use airbender::guest::transport::CsrTransport;

    pub struct CsrWordSource(CsrTransport);

    impl CsrWordSource {
        pub fn new() -> Self {
            Self(CsrTransport)
        }
    }

    impl proof_running_system::zk_ee::oracle::word_layout::WordSource for CsrWordSource {
        #[inline(always)]
        fn read_word(&mut self) -> u32 {
            airbender::guest::transport::Transport::read_word(&mut self.0)
        }
    }
}

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
    run_proving::<word_source::CsrWordSource, LoggerTy>(word_source::CsrWordSource::new())
}
