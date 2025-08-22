#![no_std]
#![no_main]

use riscv_common::zksync_os_finish_success;

extern "C" {
    // Boundaries of the .rodata section
    static mut _sirodata: usize;
    static mut _srodata: usize;
    static mut _erodata: usize;
}

#[link_section = ".init.rust"]
#[export_name = "_start_rust"]
unsafe extern "C" fn start_rust() -> ! {
    main()
}

core::arch::global_asm!(include_str!(
    "../../../../../zksync_os/src/asm/asm_reduced.S"
));

unsafe fn load_to_ram(src: *const u8, dst_start: *mut u8, dst_end: *mut u8) {
    #[cfg(debug_assertions)]
    {
        const ROM_BOUND: usize = 1 << 21;
    
        debug_assert!(src.addr() < ROM_BOUND);
        debug_assert!(dst_start.addr() >= ROM_BOUND);
        debug_assert!(dst_end.addr() >= dst_start.addr());
    }

    let offset = dst_end.addr() - dst_start.addr();

    core::ptr::copy_nonoverlapping(
        src,
        dst_start,
        offset
    );
}

unsafe fn workload() -> ! {
    use core::ptr::addr_of_mut;

    let load_address = addr_of_mut!(_sirodata);
    let rodata_start = addr_of_mut!(_srodata);
    let rodata_end = addr_of_mut!(_erodata);
    load_to_ram(load_address as *const u8, rodata_start as *mut u8, rodata_end as *mut u8);

    crypto::init_lib();

    crypto::blake2s::blake2s_tests::run_tests();
    zksync_os_finish_success(&[1, 0, 0, 0, 0, 0, 0, 0]);
}

#[inline(never)]
fn main() -> ! {
    unsafe { workload() }
}
