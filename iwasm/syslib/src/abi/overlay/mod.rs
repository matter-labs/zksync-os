//! Overlays the calldata.

pub mod impls;
pub mod overlaid_calldata;
mod raw;

use core::marker::PhantomData;

pub use impls::*;
pub use raw::*;

use super::Overlaid;
use crate::{abi::ABI_SLOT_SIZE, dev};
use overlaid_calldata::OverlaidCalldata;

/// Calldata reference. Points to a slot in calldata that contains `T`.
#[derive(Debug, Copy)]
pub struct Cdr<T>
where
    T: Overlaid,
{
    // TODO: once the derive macro is done, make the fields private.
    pub(crate) data: OverlaidData,
    pub(crate) reflection: T::Reflection,
    phantom: PhantomData<T>,
}

impl<T> Clone for Cdr<T>
where
    T: Overlaid,
{
    fn clone(&self) -> Self {
        Self {
            data: self.data,
            reflection: self.reflection.clone(),
            phantom: self.phantom,
        }
    }
}

impl<T: Overlaid> Cdr<T> {
    /// Creates a new calldata reference for `T`, at `ix` slot, in a calldata instance at
    /// `base_ptr`.
    ///
    /// # Safety
    ///  - `base_ptr` must point to the first calldata slot in the `OverlaidCalldata` (returned
    ///     with `as_mut_ptr`).
    pub unsafe fn new(base_ptr: *mut (), ix: CalldataIndex) -> Self {
        if ix >= OverlaidCalldata::get_metadata(base_ptr).calldata_slots as u32 {
            if dev!() {
                panic!(
                    "Calldata index out of bounds: index {ix} for calldata length {}",
                    OverlaidCalldata::get_metadata(base_ptr).calldata_slots
                );
            } else {
                panic!("Calldata index out of bounds.");
            }
        }

        Self {
            data: OverlaidData::new(base_ptr, ix),
            reflection: T::reflection_uninit(),
            phantom: PhantomData,
        }
    }

    /// Creates a new calldata reference for `T`. If `T` is encoded directly, then the reference
    /// points to the `ix` slot, at which the value resides. If `T` is decoded through an
    /// indirection, then the reference will point to offset value at `ix` + `offset_base`.
    /// # Safety
    ///
    /// TODO: add docs
    pub unsafe fn new_with_offset(
        base_ptr: *mut (),
        ix: CalldataIndex,
        offset_base: CalldataIndex,
    ) -> Self {
        let ix = match T::IS_INDIRECT {
            true => {
                // `ix` is the offset.
                let slot = AbiSlot::instantiate((base_ptr as *mut AbiSlot).add(ix as usize));
                let offset = slot.read_u32_be();

                assert_eq!(
                    0,
                    offset % ABI_SLOT_SIZE as u32,
                    "Wrong overlay calldata alignment: offset: {}, alignment: {}.",
                    offset,
                    offset % ABI_SLOT_SIZE as u32
                );

                let offset = offset / ABI_SLOT_SIZE as u32;

                offset + offset_base
            }
            false => ix, // `ix` is the value.
        };

        Self::new(base_ptr, ix)
    }

    pub fn decode(&self) -> T {
        T::decode(self)
    }
}

impl<T: Overlaid> core::ops::Deref for Cdr<T> {
    type Target = T::Deref;

    fn deref(&self) -> &Self::Target {
        T::to_deref(self)
    }
}
