use super::*;
use crate::io_oracle::NonDeterminismCSRSourceImplementation;
use alloc::alloc::{GlobalAlloc, Layout};
use basic_bootloader::bootloader::config::BasicBootloaderProvingExecutionConfig;
use core::alloc::Allocator;
use core::mem::MaybeUninit;
use zk_ee::logger_log;
use zk_ee::memory::ZSTAllocator;
use zk_ee::oracle::IOOracle;
use zk_ee::system::tracer::NopTracer;
use zk_ee::system::validator::NopTxValidator;
use zk_ee::system::{logger::Logger, NopResultKeeper};

#[derive(Clone, Copy, Debug, Default)]
pub struct ProxyAllocator;

impl ZSTAllocator for ProxyAllocator {}

unsafe impl Allocator for ProxyAllocator {
    fn allocate(
        &self,
        layout: core::alloc::Layout,
    ) -> Result<core::ptr::NonNull<[u8]>, core::alloc::AllocError> {
        #[allow(static_mut_refs)]
        unsafe {
            USED_ALLOCATOR.assume_init_ref().allocate(layout)
        }
    }

    unsafe fn deallocate(&self, ptr: core::ptr::NonNull<u8>, layout: core::alloc::Layout) {
        #[allow(static_mut_refs)]
        unsafe {
            USED_ALLOCATOR.assume_init_ref().deallocate(ptr, layout)
        }
    }

    unsafe fn grow(
        &self,
        _ptr: core::ptr::NonNull<u8>,
        _old_layout: core::alloc::Layout,
        _new_layout: core::alloc::Layout,
    ) -> Result<core::ptr::NonNull<[u8]>, core::alloc::AllocError> {
        panic!("grow is not allowed");
        // Commented out to avoid warning:
        // #[allow(static_mut_refs)]
        // unsafe {
        //     USED_ALLOCATOR
        //         .assume_init_ref()
        //         .grow(ptr, old_layout, new_layout)
        // }
    }

    unsafe fn shrink(
        &self,
        _ptr: core::ptr::NonNull<u8>,
        _old_layout: core::alloc::Layout,
        _new_layout: core::alloc::Layout,
    ) -> Result<core::ptr::NonNull<[u8]>, core::alloc::AllocError> {
        panic!("shrink is not allowed");
        // Commented out to avoid warning:
        // unsafe {
        //     USED_ALLOCATOR
        //         .assume_init_ref()
        //         .shrink(ptr, old_layout, new_layout)
        // }
    }
}

cfg_if::cfg_if! {
    if #[cfg(feature = "scalloc")] {
        static mut USED_ALLOCATOR: MaybeUninit<crate::one_level_allocator::SizeClassesAllocator> =
        MaybeUninit::uninit();
    } else {
        static mut USED_ALLOCATOR: MaybeUninit<crate::talc::TalcWrapper> = MaybeUninit::uninit();
    }
}

#[inline(never)]
/// # Safety
/// `heap_start` must be less than or equal to heap_end
pub unsafe fn init_allocator(heap_start: *mut usize, heap_end: *mut usize) {
    cfg_if::cfg_if! {
        if #[cfg(feature = "scalloc")] {
            unsafe {
                crate::one_level_allocator::SizeClassesAllocator::init(
                    USED_ALLOCATOR.as_mut_ptr(),
                    heap_start,
                    heap_end,
                );
            }
        } else {
          #[allow(static_mut_refs)]
            unsafe {
                crate::talc::create_talc_allocator_wrapper(
                    USED_ALLOCATOR.as_mut_ptr(),
                    heap_start,
                    heap_end,
                );
            }
        }
    }
}

// we can not use generic allocator below due to constraints cycles (even though it's not true),
// so we have to typedef

pub type BootloaderAllocator = ProxyAllocator;

// TODO: disable global alloc once dependencies are fixed
pub struct OptionalGlobalAllocator;

#[cfg(feature = "global-alloc")]
unsafe impl GlobalAlloc for OptionalGlobalAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        BootloaderAllocator::default()
            .allocate(layout)
            .expect("Global allocactor: alloc")
            .as_mut_ptr()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        BootloaderAllocator::default().deallocate(
            core::ptr::NonNull::new(ptr).expect("Global allocator: dealloc"),
            layout,
        );
    }
}

#[cfg(not(feature = "global-alloc"))]
unsafe impl GlobalAlloc for OptionalGlobalAllocator {
    unsafe fn alloc(&self, _layout: Layout) -> *mut u8 {
        panic!("global alloc not allowed")
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        panic!("global alloc not allowed");
    }
}

///
/// main zksync_os program, that is responsible for running the proving flow.
///
/// it fetches all the necessary information from the oracle (via the CRS register).
/// Uses a special allocator, to only use memory between heap_start and heap_end.
///
/// Returns public input.
///
#[inline(never)]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn run_proving<I: NonDeterminismCSRSourceImplementation, L: Logger + Default>(
    heap_start: *mut usize,
    heap_end: *mut usize,
) -> [u32; 8] {
    logger_log!(L::default(), "Enter proving bootloader");

    // init allocator
    // allocator is a global singleton object, that can be later accessed by ProxyAllocator
    unsafe {
        init_allocator(heap_start, heap_end);
    }

    logger_log!(L::default(), "Allocator init is complete");

    // oracle is just a thin proxy
    let oracle = CsrBasedIOOracle::<I>::init();

    logger_log!(L::default(), "Oracle init is complete");

    run_proving_inner::<_, I, L>(oracle)
}

#[cfg(not(feature = "multiblock-batch"))]
pub fn run_proving_inner<
    O: IOOracle,
    I: NonDeterminismCSRSourceImplementation,
    L: Logger + Default,
>(
    oracle: O,
) -> [u32; 8] {
    logger_log!(L::default(), "IO implementer init is complete");

    // Load all transactions from oracle and apply them.
    let (_oracle, public_input) =
        ProvingBootloader::<O, L>::run_prepared::<BasicBootloaderProvingExecutionConfig>(
            oracle,
            &mut (),
            &mut NopResultKeeper::default(),
            &mut NopTracer::default(),
            &mut NopTxValidator,
        )
        .expect("Tried to prove a failing batch");

    unsafe { core::mem::transmute(public_input) }
}

#[cfg(feature = "multiblock-batch")]
pub fn run_proving_inner<
    O: IOOracle,
    I: NonDeterminismCSRSourceImplementation,
    L: Logger + Default,
>(
    mut oracle: O,
) -> [u32; 8] {
    logger_log!(L::default(), "IO implementer init is complete");

    // simulating query, just in case
    I::csr_write_impl(0xdeadbeef);
    I::csr_write_impl(0);
    let count = I::csr_read_impl();
    let mut batch_data = basic_bootloader::bootloader::block_flow::ZKBatchDataKeeper::new();
    for _ in 0..count {
        oracle = ProvingBootloader::<O, L>::run_prepared::<BasicBootloaderProvingExecutionConfig>(
            oracle,
            &mut batch_data,
            &mut NopResultKeeper::default(),
            &mut NopTracer::default(),
            &mut NopTxValidator,
        )
        .expect("Tried to prove a failing batch");
    }

    let public_input = zk_ee::utils::Bytes32::from_array(
        batch_data
            .into_public_input(L::default(), &mut oracle)
            .hash(),
    );

    unsafe { core::mem::transmute(public_input) }
}
