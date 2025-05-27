pub const unsafe fn default_bumping_allocator() -> AssumeSingleThreaded<BumpingAllocator> {
    AssumeSingleThreaded::new(BumpingAllocator::new())
}

pub unsafe fn init_global_alloc(
    allocator: *mut AssumeSingleThreaded<BumpingAllocator>,
    mut heap_start: usize,
) {
    if heap_start == 0 {
        heap_start = 32;
    }
    *(*allocator).inner.start.get() = heap_start;
}

pub struct AssumeSingleThreaded<T> {
    pub inner: T,
}

impl<T> AssumeSingleThreaded<T> {
    pub const unsafe fn new(t: T) -> Self {
        AssumeSingleThreaded { inner: t }
    }
}

unsafe impl<T> Sync for AssumeSingleThreaded<T> {}

unsafe impl<T: Allocator> Allocator for AssumeSingleThreaded<T> {
    fn allocate(&self, layout: Layout) -> Result<core::ptr::NonNull<[u8]>, AllocError> {
        self.inner.allocate(layout)
    }
    unsafe fn deallocate(&self, ptr: core::ptr::NonNull<u8>, layout: Layout) {
        self.inner.deallocate(ptr, layout)
    }
}

unsafe impl<T: GlobalAlloc> GlobalAlloc for AssumeSingleThreaded<T> {
    #[inline(never)]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.inner.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.inner.dealloc(ptr, layout);
    }
}

/// A non-thread safe allocator that uses linear memory and doesn't deallocate
pub struct BumpingAllocator<T = DefaultGrower> {
    start: UnsafeCell<usize>,
    grower: T,
}

pub trait MemoryGrower {
    fn memory_grow(&self, delta: usize) -> usize;
}

/// Stateless heap grower.
/// On wasm32, provides a default implementation of [MemoryGrower].
pub struct DefaultGrower;

#[cfg(target_arch = "wasm32")]
impl MemoryGrower for DefaultGrower {
    fn memory_grow(&self, delta: usize) -> usize {
        core::arch::wasm32::memory_grow::<0>(delta)
    }
}

#[cfg(target_arch = "wasm32")]
impl BumpingAllocator<DefaultGrower> {
    pub const fn new() -> Self {
        BumpingAllocator {
            start: UnsafeCell::new(0),
            grower: DefaultGrower,
        }
    }
}

unsafe impl<T> Send for BumpingAllocator<T> {}

const PAGE_SIZE: usize = 1 << 16;

use core::{alloc::*, cell::UnsafeCell};

unsafe impl<T: MemoryGrower> GlobalAlloc for BumpingAllocator<T> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let alignment = layout.align();

        let start = *self.start.get();
        let current_max_page_idx = start / PAGE_SIZE;
        let allocation_start = align_up(start, alignment);
        let new_start = allocation_start + size;
        let new_max_page_idx = (new_start - 1) / PAGE_SIZE;
        let diff = new_max_page_idx - current_max_page_idx;
        if diff > 0 {
            self.grower.memory_grow(diff);
        }

        *self.start.get() = new_start;

        allocation_start as *mut u8
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}

unsafe impl<T: MemoryGrower> Allocator for BumpingAllocator<T> {
    fn allocate(&self, layout: Layout) -> Result<core::ptr::NonNull<[u8]>, AllocError> {
        if layout.size() == 0 {
            return Ok(core::ptr::NonNull::slice_from_raw_parts(
                layout.dangling(),
                0,
            ));
        } else {
            let start = unsafe { *self.start.get() };
            let current_max_page_idx = start / PAGE_SIZE;
            let allocation_start = start + (start as *const u8).align_offset(layout.align());
            let new_start = allocation_start + layout.size();
            let new_max_page_idx = (new_start - 1) / PAGE_SIZE;
            let diff = new_max_page_idx - current_max_page_idx;
            if diff > 0 {
                self.grower.memory_grow(diff);
            }

            unsafe { *self.start.get() = new_start };
            Ok(unsafe {
                core::ptr::NonNull::slice_from_raw_parts(
                    core::ptr::NonNull::new_unchecked(allocation_start as *mut u8),
                    layout.size(),
                )
            })
        }
    }

    unsafe fn deallocate(&self, _ptr: core::ptr::NonNull<u8>, _layout: Layout) {}
}

#[inline(always)]
fn align_up(offset: usize, alignment: usize) -> usize {
    if alignment.is_power_of_two() == false {
        unsafe {
            core::hint::unreachable_unchecked();
        }
    }
    (!(alignment - 1)) & (offset + (alignment - 1))
}
