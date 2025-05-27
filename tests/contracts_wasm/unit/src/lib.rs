#![feature(generic_const_exprs)]
#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

use core::hint::black_box;

use syslib::abi::encoder;
use syslib::contract;
use syslib::selector;
use syslib::types::Address;

mod system;

// SAFETY: This application is single threaded, so using AssumeSingleThreaded is allowed.
#[global_allocator]
#[cfg(all(target_arch = "wasm32", target_os="unknown"))]
static mut WASM_ALLOCATOR: syslib::allocator::AssumeSingleThreaded<
syslib::allocator::BumpingAllocator> =
    unsafe { syslib::allocator::default_bumping_allocator() };


#[no_mangle]
#[allow(unused_must_use)]
pub extern "C" fn runtime() -> &'static syslib::sys::SliceRef<usize> {
    use syslib::abi::Encodable;

    #[cfg(all(target_arch = "wasm32", target_os="unknown"))]
    {
        syslib::init(unsafe { core::ptr::addr_of_mut!(WASM_ALLOCATOR) });
    }

    let selector = syslib::system::calldata::selector();

    let mut encoder = syslib::abi::Encoder::new(32);

    match selector {
        0x1 => {
            black_box(system::msg_from(&mut encoder));
        },
        0x2 => {
            black_box(system::hash_keccak256(&mut encoder));
        }
        x => panic!("Unknown selector 0x{:08x?}.", x),
    };

    let r = syslib::sys::SliceRef::<usize>::from_ref(encoder.finalize());
    let r = alloc::boxed::Box::new(r);
    alloc::boxed::Box::leak(r)
}

