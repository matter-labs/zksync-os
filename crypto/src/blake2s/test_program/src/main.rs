#![no_std]
#![no_main]

use riscv_common::zksync_os_finish_success;

#[link_section = ".init.rust"]
#[export_name = "_start_rust"]
unsafe extern "C" fn start_rust() -> ! {
    main()
}

unsafe fn workload() -> ! {
    crypto::init_lib();

    crypto::blake2s::blake2s_tests::run_tests();
    zksync_os_finish_success(&[1, 0, 0, 0, 0, 0, 0, 0]);
}

#[inline(never)]
fn main() -> ! {
    unsafe { workload() }
}
