#![no_std]
#![no_main]

core::arch::global_asm!(include_str!(
    "../../../../../zksync_os/src/asm/asm_reduced.S"
));

#[link_section = ".init.rust"]
#[export_name = "_start_rust"]
unsafe extern "C" fn start_rust() -> ! {
    main()
}

#[inline(never)]
fn main() -> ! {
    unsafe { workload() }
}

// copied from repo/zksync_os/src/main.rs
// (this is necessary to ensure ROM pointers keep working with new linker script)
unsafe fn init_rom() {
    extern "C" {
        // Boundaries of the heap
        static mut _sheap: usize;
        static mut _eheap: usize;
    
        // Boundaries of the stack
        static mut _sstack: usize;
        static mut _estack: usize;
    
        // Boundaries of the .data section (and it's part in ROM)
        static mut _sidata: usize;
        static mut _sdata: usize;
        static mut _edata: usize;
    
        // Boundaries of the .rodata section
        static mut _sirodata: usize;
        static mut _srodata: usize;
        static mut _erodata: usize;
    }
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
    let heap_start = core::ptr::addr_of_mut!(_sheap);
    let heap_end = core::ptr::addr_of_mut!(_eheap);
    let load_address = core::ptr::addr_of_mut!(_sirodata);
    let rodata_start = core::ptr::addr_of_mut!(_srodata);
    let rodata_end = core::ptr::addr_of_mut!(_erodata);
    load_to_ram(load_address as *const u8, rodata_start as *mut u8, rodata_end as *mut u8);
}

unsafe fn workload() -> ! {
    init_rom();
    crypto::init_lib();

    #[cfg(not(any(feature="keccak_f1600_test",feature="bad_keccak_f1600_test",feature="mini_digest_test",feature="hash_chain_test")))] 
    compile_error!("at least one testing feature must be enabled");

    #[cfg(feature = "keccak_f1600_test")]
    crypto::sha3::delegated::tests::keccak_f1600_test();

    #[cfg(feature = "bad_keccak_f1600_test")]
    crypto::sha3::delegated::tests::bad_keccak_f1600_test();

    #[cfg(feature = "mini_digest_test")]
    crypto::sha3::delegated::tests::mini_digest_test();

    #[cfg(feature = "hash_chain_test")]
    crypto::sha3::delegated::tests::hash_chain_test();

    riscv_common::zksync_os_finish_success(&[1, 0, 0, 0, 0, 0, 0, 0]);
}