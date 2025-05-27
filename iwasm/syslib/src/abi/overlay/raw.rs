use core::mem::{align_of, size_of};

use iwasm_specification::intx::U256Repr;

use super::overlaid_calldata::OverlaidCalldata;

pub type CalldataIndex = u32;

#[repr(transparent)]
pub(crate) struct AbiSlot([u8; crate::abi::ABI_SLOT_SIZE]);

impl AbiSlot {
    /// Safety: Pointed bytes must be init, ptr must be aligned.
    pub(crate) unsafe fn instantiate<'a>(ptr: *mut AbiSlot) -> &'a mut Self {
        debug_assert_eq!(
            0,
            ptr as usize % align_of::<u64>(),
            "Improperly aligned abi slot. Required alignment of {}, have {}",
            align_of::<u64>(),
            ptr as usize % align_of::<u64>()
        );

        // Safety: ptr is aligned, bytes are init as required.
        &mut *ptr
    }

    pub(crate) fn read_u32_be(&self) -> u32 {
        // TODO: check asm efficiency
        u32::from_be_bytes(self.0[28..].try_into().unwrap())
    }

    pub(crate) fn write_u32_ne(&mut self, value: u32) {
        // TODO: check asm efficiency
        self.0[0..4].copy_from_slice(&u32::to_ne_bytes(value));
    }

    /// Safety: `&self` must be properly aligned.
    pub(crate) unsafe fn as_u32<'a>(&self) -> &'a u32 {
        // Safety: Dereferensing as shared from shared ref.
        unsafe { &*(self as *const _ as *const _) }
    }

    #[allow(dead_code)]
    pub(crate) unsafe fn as_u256_repr(&self) -> &U256Repr {
        const { assert!(size_of::<U256Repr>() == size_of::<AbiSlot>()) }
        unsafe { &*(self as *const _ as *const _) }
    }

    pub(crate) unsafe fn as_u256_repr_mut(&mut self) -> &mut U256Repr {
        const { assert!(size_of::<U256Repr>() == size_of::<AbiSlot>()) }
        unsafe { &mut *(self as *mut _ as *mut _) }
    }

    /// # Safety:
    /// - `size_of::<T>()` must be equal or smaller than `self`.
    /// - T should have alignment as `u64` or smaller.
    pub(crate) unsafe fn write_raw<T>(&mut self, value: T) {
        (self as *mut _ as *mut T).write(value);
    }

    /// # Safety:
    /// - T must already be written to self.
    pub(crate) unsafe fn as_raw<'a, T>(&self) -> &'a T {
        // Safety: Dereferensing as shared from shared ref.
        unsafe { &*(self as *const _ as *const _) }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct OverlaidData {
    pub(crate) base_ptr: *mut (),
    pub(crate) ix: CalldataIndex,
}

impl OverlaidData {
    pub unsafe fn new(base_ptr: *mut (), ix: CalldataIndex) -> Self {
        Self { base_ptr, ix }
    }

    pub(crate) fn as_mut_pair(&self) -> (*mut AbiSlot, BitRefMut) {
        let slot_ptr = unsafe { (self.base_ptr as *mut AbiSlot).add(self.ix as usize) };

        let key_ix = self.ix / 32;
        let key_mask = 1 << (self.ix % 32);

        let key_ptr =
            unsafe { OverlaidCalldata::base_to_key_ptr(self.base_ptr).sub(key_ix as usize) };

        let key_ref = BitRefMut {
            ptr: key_ptr,
            mask: key_mask,
        };

        (slot_ptr, key_ref)
    }
}

pub(crate) struct BitRefMut {
    ptr: *mut u32,
    mask: u32,
}

impl BitRefMut {
    pub(crate) fn is_set(&self) -> bool {
        unsafe { self.ptr.read() & self.mask == self.mask }
    }

    pub(crate) fn set(&mut self) {
        unsafe { self.ptr.write(self.ptr.read() | self.mask) };
    }
}

#[cfg(test)]
mod tests {
    use super::OverlaidData;

    #[test]
    fn overlaid_data_bit_ref() {
        let mut x = 0u32;

        let ptr_u32 = &mut x as *mut u32;
        let ptr = ptr_u32 as *mut ();

        let a = |ix: u32, ptr_offset: usize, bit_offset: usize| {
            let r = unsafe { OverlaidData::new(ptr, ix).as_mut_pair().1 };

            unsafe {
                assert_eq!(
                    ptr_u32.sub(ptr_offset),
                    r.ptr,
                    "Wrong ptr offset for ix {}.",
                    ix
                )
            };
            assert_eq!(1 << bit_offset, r.mask, "Wrong bit offset for ix {}.", ix);
        };

        a(0, 3, 0);
        a(1, 3, 1);
        a(31, 3, 31);
        a(32, 4, 0);
    }
}
