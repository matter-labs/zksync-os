pub mod allocator;
pub mod intx;
pub mod system;

use iwasm_specification::host_ops::*;
use iwasm_specification::sys::HostOpResult;
use system::terminate_execution;

use crate::allocator::*;
use crate::*;
use types::uintx::*;

// Memory manamgement related linker variables
extern "C" {
    pub static __heap_base: usize;
}

#[cfg(feature = "default_panic_handler")]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    if let Some(s) = info.payload().downcast_ref::<&str>() {
        system::terminate_execution(s);
    }

    if let Some(s) = info.payload().downcast_ref::<alloc::string::String>() {
        system::terminate_execution(s.as_str());
    }

    if let Some(l) = info.location() {
        system::terminate_execution(
            alloc::format!("Panic at {}:{}: {}", l.file(), l.line(), info.message()).as_str(),
        );
    }

    system::terminate_execution(
        alloc::format!("Panic at unknown location: {}", info.message()).as_str(),
    );
}

// All mutating operations below require:
// - alignment (otherwise will bail out immediately)
// - in-bounds, so if some pointer + implied length are beyond range of the heap, then it bails or panics
// and guarantee that:
// - if the same pointer is passed as in and out, then in is read before out
// - if same pointer is set for two outs, then first out is written before second out

#[allow(improper_ctypes)]
extern "C" {
    // this will be compiled as WASM multivalue, even though we use extern "C" above
    pub fn short_host_op(op: ShortHostOp, op_param: u64, op1: u32, op2: u32) -> HostOpResult;
    // we can also hide all IO ops (sstore/sload, logs) under another interface. Also, calldataload/copy/returndataload/copy.
    // op1/2 and dst1/2 could be pointers to something that is u32 aligned, but not u32 sized
    pub fn long_host_op(
        op: LongHostOp,
        op_param: u64,
        op1: *const (),
        op2: *const (),
        dst1: *mut (),
        dst2: *mut (),
    ) -> HostOpResult;
}

#[inline(always)]
pub fn handle_host_call<F: FnOnce() -> (bool, u64)>(f: F) -> u64 {
    let (success, result) = f();

    if success == false {
        terminate_execution("Host call resulted with failure.")
    }

    result
}

pub fn init(allocator: *mut AssumeSingleThreaded<BumpingAllocator>) {
    unsafe {
        let heap_base = core::ptr::addr_of!(__heap_base) as usize;
        init_global_alloc(allocator, heap_base);
    }
}
