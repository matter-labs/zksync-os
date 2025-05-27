//! Memory subsystem interface.
//! Memory is split into a user-facing minimal interface (exposed to EEs)
//! and an extended one with more functionality for use within the
//! rest of the system and bootloader.
//!
use core::ops::{Deref, DerefMut};

use super::errors::InternalError;

// Hierarchy here is simple:
// - OS owns everything and has 'static lifetime relative to any EE that will be launched
// - `OSManagedRegion` reflects any slice that OS allocated and can eventually deallocate, but it'll happen beyond
//    the scope of EE lifetime
// - `OSManagedRegion` can be given back to OS and new one will be returned with the same content at relative offsets.
//   So, it's almost like `reallocate + grow` if we would consider Memory as Allocator
// - `OSManagedImmutableSlice` is a region that can not be resized and not intended to be mutated

pub trait OSManagedRegion: 'static + Deref<Target = [u8]> + DerefMut<Target = [u8]> {
    type OSManagedImmutableSlice: 'static + Deref<Target = [u8]>;

    fn take_slice(&self, range: core::ops::Range<usize>) -> Self::OSManagedImmutableSlice;
}

///
/// User facing memory trait.
///
pub trait MemorySubsystem {
    type Allocator: core::alloc::Allocator + Clone;
    type ManagedRegion: OSManagedRegion;

    fn get_allocator(&self) -> Self::Allocator;

    /// Creates an empty region managed by the OS.
    fn empty_managed_region(&mut self) -> Self::ManagedRegion;

    /// Creates an empty slice managed by the OS.
    fn empty_immutable_slice(
        &mut self,
    ) -> <Self::ManagedRegion as OSManagedRegion>::OSManagedImmutableSlice;

    /// Grows the heap, returns None on OOM.
    fn grow_heap(
        &mut self,
        existing_region: Self::ManagedRegion,
        new_size: usize,
    ) -> Result<Option<Self::ManagedRegion>, InternalError>;
}

///
/// Extended memory trait for use in the system and bootloader.
///
pub trait MemorySubsystemExt: MemorySubsystem {
    type Snapshot: Clone;

    fn new(allocator: Self::Allocator) -> Self;

    /// Indicate the a new transaction is being processed.
    fn begin_next_tx(&mut self);

    /// Control method for OS only.
    fn start_memory_frame(&mut self) -> Self::Snapshot;

    /// Control method for OS only.
    fn finish_memory_frame(&mut self, snapshot: Option<Self::Snapshot>);

    /// Caller must ensure that all regions referencing internals are destroyed. Namely - that EEs callstack is empty,
    /// and we no longer post-process anything
    /// # Safety
    /// TODO
    unsafe fn clear_returndata_region(&mut self);

    fn copy_into_return_memory(
        &mut self,
        source: &[u8],
    ) -> Result<Self::ManagedRegion, InternalError>;

    /// If slice is `'static` relative to the OS, then we can allow it to pretend as to be OS managed.
    /// In any case immutable slice is not something that is "deallocated"
    /// # Safety
    /// TODO
    unsafe fn construct_immutable_slice_from_static_slice(
        &self,
        slice: &'static [u8],
    ) -> <Self::ManagedRegion as OSManagedRegion>::OSManagedImmutableSlice;

    fn assert_no_frames_opened(&self);
}
