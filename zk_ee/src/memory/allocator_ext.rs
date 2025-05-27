use core::alloc::Allocator;

pub trait ZSTAllocator: 'static + Allocator + Sized + Clone + Copy + Default {}

use alloc::alloc::Global;

impl ZSTAllocator for Global {}

const _: () = {
    assert!(core::mem::size_of::<Global>() == 0);
};
