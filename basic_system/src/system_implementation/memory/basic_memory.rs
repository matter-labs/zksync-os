//! Implementation of the memory subsystem.
use alloc::vec::Vec;
use core::alloc::Allocator;
use core::alloc::Layout;
use core::ops::Deref;
use core::ops::DerefMut;
use core::ptr::NonNull;
use zk_ee::system::errors::InternalError;
use zk_ee::system::*;
use zk_ee::utils::allocate_vec_usize_aligned;
use zk_ee::utils::USIZE_ALIGNMENT;
use zk_ee::utils::USIZE_SIZE;

pub const MAX_HEAP_BUFFER_SIZE: usize = 1 << 27; // 128 MB
pub const MAX_RETURNDATA_BUFFER_SIZE: usize = 1 << 27; // 128 MB

// OS managed slice is non-clone and non-copy,
// but can be given out to Memory to grow, and dereferenced
pub struct OSManagedResizableSlice {
    ptr: NonNull<u8>,
    len: usize,
}

unsafe impl Send for OSManagedResizableSlice {}
unsafe impl Sync for OSManagedResizableSlice {}

impl Deref for OSManagedResizableSlice {
    type Target = [u8];

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe { core::slice::from_raw_parts(self.ptr.as_ptr().cast_const(), self.len) }
    }
}

impl DerefMut for OSManagedResizableSlice {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { core::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) }
    }
}

pub struct OSManagedSlice {
    ptr: NonNull<u8>,
    len: usize,
}

unsafe impl Send for OSManagedSlice {}
unsafe impl Sync for OSManagedSlice {}

impl Deref for OSManagedSlice {
    type Target = [u8];

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe { core::slice::from_raw_parts(self.ptr.as_ptr().cast_const(), self.len) }
    }
}

impl OSManagedRegion for OSManagedResizableSlice {
    type OSManagedImmutableSlice = OSManagedSlice;

    fn take_slice(&self, range: core::ops::Range<usize>) -> Self::OSManagedImmutableSlice {
        assert!(range.end <= self.len);
        unsafe {
            let ptr = self.ptr.add(range.start);
            OSManagedSlice {
                ptr,
                len: range.len(),
            }
        }
    }
}

// NOTE: aliasing rules are preserved here: as long as we use our memory subsystems,
// safe code can not make produce overlapping pointers to memory regions belonging to
// different EEs. Effectively, we are very dump dump allocator with an option to deallocate
// latest element and reuse space to grow "new latest" one

const HEAP_BUFFER_LAYOUT: Layout =
    match Layout::from_size_align(MAX_HEAP_BUFFER_SIZE, USIZE_ALIGNMENT) {
        Ok(layout) => layout,
        Err(_) => unreachable!(),
    };

impl<A: Allocator> Drop for MemoryImpl<A> {
    fn drop(&mut self) {
        unsafe {
            self.allocator
                .deallocate(self.heap_buffer.cast(), HEAP_BUFFER_LAYOUT);
        }
    }
}

pub struct MemoryImpl<A: Allocator> {
    heap_buffer: NonNull<[u8]>,
    next_free_byte: NonNull<u8>,
    current_heap_start: NonNull<u8>,
    allocator: A,

    heap_buffers_stack: Vec<NonNull<u8>, A>,
    returndata_buffer: Vec<u8, A>,
}

impl<A: Allocator + Clone> MemorySubsystem for MemoryImpl<A> {
    type Allocator = A;
    type ManagedRegion = OSManagedResizableSlice;

    fn get_allocator(&self) -> Self::Allocator {
        self.allocator.clone()
    }

    fn empty_managed_region(&mut self) -> Self::ManagedRegion {
        // same behavior as for empty Rust slice: any properly aligned pointer works
        OSManagedResizableSlice {
            ptr: NonNull::dangling(),
            len: 0,
        }
    }

    fn empty_immutable_slice(
        &mut self,
    ) -> <Self::ManagedRegion as OSManagedRegion>::OSManagedImmutableSlice {
        // same behavior as for empty Rust slice: any properly aligned pointer works
        OSManagedSlice {
            ptr: NonNull::dangling(),
            len: 0,
        }
    }

    fn grow_heap(
        &mut self,
        existing_region: Self::ManagedRegion,
        new_size: usize,
    ) -> Result<Option<Self::ManagedRegion>, InternalError> {
        // first we verify that we indeed manage the heap, and can only resize such slices,
        // and it must be "top" of the managed heap

        if !existing_region.is_empty()
            && (self.current_heap_start != existing_region.ptr
                || self.next_free_byte.as_ptr()
                    != existing_region
                        .ptr
                        .as_ptr()
                        .wrapping_add(existing_region.len))
        {
            return Err(InternalError("unmanaged region or not latest region"));
        }

        let new_size = new_size.next_multiple_of(USIZE_SIZE);

        let current_heap_capacity = unsafe {
            self.heap_buffer.len()
                - self
                    .current_heap_start
                    .offset_from_unsigned(self.heap_buffer.cast())
        };
        if new_size > current_heap_capacity {
            return Ok(None);
        }
        let new_end = unsafe { self.current_heap_start.add(new_size) };

        // zero-initialize the new memory
        unsafe {
            let growth = new_end.offset_from(self.next_free_byte);
            if growth > 0 {
                self.next_free_byte.write_bytes(0, growth as usize);
            }
        }
        self.next_free_byte = new_end;

        Ok(Some(OSManagedResizableSlice {
            ptr: self.current_heap_start,
            len: new_size,
        }))
    }
}

impl<A: Allocator + Clone> MemorySubsystemExt for MemoryImpl<A> {
    type Snapshot = u16;

    fn new(allocator: Self::Allocator) -> Self {
        let heap_buffer = allocator.allocate(HEAP_BUFFER_LAYOUT).unwrap();
        let heap_buffers_stack =
            Vec::with_capacity_in(MAX_GLOBAL_CALLS_STACK_DEPTH, allocator.clone());
        let returndata_buffer =
            allocate_vec_usize_aligned(MAX_RETURNDATA_BUFFER_SIZE, allocator.clone());

        Self {
            heap_buffer,
            next_free_byte: heap_buffer.cast(),
            current_heap_start: heap_buffer.cast(),
            allocator,

            heap_buffers_stack,
            returndata_buffer,
        }
    }

    fn begin_next_tx(&mut self) {
        assert!(self.heap_buffers_stack.is_empty());
        self.next_free_byte = self.heap_buffer.cast();
        self.current_heap_start = self.heap_buffer.cast();
        unsafe { self.clear_returndata_region() };
    }

    unsafe fn clear_returndata_region(&mut self) {
        self.returndata_buffer.clear();
    }

    fn start_memory_frame(&mut self) -> Self::Snapshot {
        let r = self.heap_buffers_stack.len();
        // we need to put a mark
        self.heap_buffers_stack.push(self.current_heap_start);
        self.current_heap_start = self.next_free_byte;

        r as u16 // 65k is much larger than vm limit.
    }

    fn finish_memory_frame(&mut self, snapshot: Option<Self::Snapshot>) {
        let heap_start = match snapshot {
            Some(s) => {
                let heap_start = self.heap_buffers_stack[s as usize];
                self.heap_buffers_stack.truncate(s as usize);
                heap_start
            }
            None => self
                .heap_buffers_stack
                .pop()
                .expect("Finishing a frame implies having one opened."),
        };

        self.next_free_byte = self.current_heap_start;
        self.current_heap_start = heap_start;
    }

    fn copy_into_return_memory(
        &mut self,
        source: &[u8],
    ) -> Result<OSManagedResizableSlice, InternalError> {
        debug_assert!(self.returndata_buffer.len() % USIZE_SIZE == 0);
        debug_assert!(self.returndata_buffer.len() % USIZE_ALIGNMENT == 0);

        let new_returndata_start = self.returndata_buffer.len();

        let Some(new_returndata_len) = new_returndata_start.checked_add(source.len()) else {
            return Err(InternalError("OOM"));
        };
        let new_returndata_len = new_returndata_len.next_multiple_of(USIZE_SIZE);
        self.returndata_buffer.extend_from_slice(source);
        self.returndata_buffer.resize(new_returndata_len, 0);

        unsafe {
            let start = self
                .returndata_buffer
                .as_mut_ptr()
                .add(new_returndata_start);
            let slice = OSManagedResizableSlice {
                ptr: NonNull::new_unchecked(start),
                len: source.len(),
            };

            Ok(slice)
        }
    }

    fn assert_no_frames_opened(&self) {
        assert!(self.heap_buffers_stack.is_empty());
    }

    unsafe fn construct_immutable_slice_from_static_slice(
        &self,
        slice: &'static [u8],
    ) -> <Self::ManagedRegion as OSManagedRegion>::OSManagedImmutableSlice {
        // `OSManagedSlice` is immutable internally, so we only need cast for construction
        let ptr = NonNull::new_unchecked(slice.as_ptr() as *mut u8);
        let len = slice.len();

        OSManagedSlice { ptr, len }
    }
}
