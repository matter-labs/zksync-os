use alloc::vec::Vec;
use iwasm_interpreter::routines::memory::SystemMemoryManager;
use zk_ee::system_trait::System;

pub struct ZkOSIWasmMemoryManager<S: System> {
    allocator: S::Allocator,
    _marker: core::marker::PhantomData<S>,
}

impl<S: System> Clone for ZkOSIWasmMemoryManager<S> {
    fn clone(&self) -> Self {
        Self {
            allocator: self.allocator.clone(),
            _marker: core::marker::PhantomData,
        }
    }
}

impl<S: System> ZkOSIWasmMemoryManager<S> {
    pub fn new(system: &S) -> Self {
        let allocator = system.get_allocator();
        Self {
            allocator,
            _marker: core::marker::PhantomData,
        }
    }
}

impl<S: System> SystemMemoryManager for ZkOSIWasmMemoryManager<S> {
    type Allocator = S::Allocator;

    type OutputBuffer<T: Sized> = Vec<T, Self::Allocator>;
    type ScratchSpace<T: Sized> = Vec<T, Self::Allocator>;

    fn empty_scratch_space<T: Sized>(&self) -> Self::ScratchSpace<T> {
        Vec::new_in(self.get_allocator())
    }
    #[inline]
    fn get_allocator(&self) -> Self::Allocator {
        self.allocator.clone()
    }
    fn allocate_output_buffer<T: Sized>(
        &mut self,
        capacity: usize,
    ) -> Result<Self::OutputBuffer<T>, ()> {
        Ok(Vec::with_capacity_in(capacity, self.get_allocator()))
    }
    fn allocate_scratch_space<T: Sized>(
        &mut self,
        capacity: usize,
    ) -> Result<Self::ScratchSpace<T>, ()> {
        Ok(Vec::with_capacity_in(capacity, self.get_allocator()))
    }

    fn clone_scratch_space<T: Clone>(
        &mut self,
        existing_scratch: &Self::ScratchSpace<T>,
    ) -> Result<Self::ScratchSpace<T>, ()> {
        Ok(existing_scratch.clone())
    }
}
