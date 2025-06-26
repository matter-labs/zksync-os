use core::ops::DerefMut;

use zk_ee::memory::slice_vec::SliceVec;

use crate::{native_resource_constants, ExitCode};

pub struct FrameHeap<'a> {
    heap: SliceVec<'a, u8>,
    gas_paid_for_heap_growth: u64
}

impl <'a> FrameHeap<'a> {
    #[inline(always)]
    pub fn new() -> Self {
        Self {
            heap: SliceVec::new(&mut []),
            gas_paid_for_heap_growth: 0
        }
    }

    #[inline(always)]
    pub fn new_from_parts(heap: SliceVec<'a, u8>) -> Self {
        Self {
            heap,
            gas_paid_for_heap_growth: 0
        }
    }

    pub(crate) fn memory_len(&self) -> usize {
        self.heap.len()
    }

    pub(crate) fn heap(&'_ mut self) -> &'_ mut [u8] {
        self.heap.deref_mut()
    }

    pub(crate) fn resize_heap(&mut self, offset: usize, len: usize) -> Result<(), ExitCode> {
        use native_resource_constants::*;
        let max_offset = offset.saturating_add(len);
        let multiple_of_32 = if max_offset > ((u32::MAX - 31) as usize) {
            return Err(ExitCode::MemoryLimitOOG);
        } else {
            max_offset.next_multiple_of(32)
        };
        let current_heap_size = self.memory_len();
        if multiple_of_32 > current_heap_size {
            let net_byte_increase = multiple_of_32 - current_heap_size;
            let new_heap_size_words = multiple_of_32 as u64 / 32;

            let end_cost = crate::gas_constants::MEMORY
                .saturating_mul(new_heap_size_words)
                .saturating_add(new_heap_size_words.saturating_mul(new_heap_size_words) / 512);
            let net_cost_gas = end_cost - self.gas_paid_for_heap_growth;
            let net_cost_native = HEAP_EXPANSION_BASE_NATIVE_COST.saturating_add(
                HEAP_EXPANSION_PER_BYTE_NATIVE_COST.saturating_mul(net_byte_increase as u64),
            );
            self.spend_gas_and_native(net_cost_gas, net_cost_native)?;
            self.gas_paid_for_heap_growth = end_cost;

            self.heap
                .resize(multiple_of_32, 0)
                .map_err(|_| ExitCode::MemoryOOG)?;
        }

        Ok(())
    }
}