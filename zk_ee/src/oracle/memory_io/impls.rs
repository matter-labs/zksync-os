//! Implementations for existing types.

use super::continuous::{normalize_bool, ContinuousDeserializable, ContinuousSerializable};
use super::short::{ShortDeserializable, ShortSerializable};
use crate::execution_environment_type::ExecutionEnvironmentType;
use crate::internal_error;
use crate::storage_types::InitialStorageSlotData;
use crate::system::errors::internal::InternalError;
use crate::types_config::SystemIOTypesConfig;
use crate::utils::Bytes32;
use core::mem::{align_of, size_of};
use ruint::aliases::{B160, U256};

impl ShortSerializable for ExecutionEnvironmentType {
    #[inline(always)]
    fn to_short_word(self) -> u32 {
        u32::from(self as u8)
    }
}

impl ShortDeserializable for ExecutionEnvironmentType {
    #[inline(always)]
    fn from_short_word(word: u32) -> Result<Self, InternalError> {
        Self::parse_ee_version_byte(u8::from_short_word(word)?)
    }
}

/// For types without padding bytes, for which any bytes form a valid value.
macro_rules! impl_continuous_for_plain_data {
    ($($t:ty),+) => {$(
        // SAFETY: see the layout assertions of the type
        unsafe impl ContinuousSerializable for $t {}

        // SAFETY: see the layout assertions of the type; any bytes form a valid value
        unsafe impl ContinuousDeserializable for $t {
            #[inline(always)]
            unsafe fn validate<'a>(this: *mut Self) -> Result<&'a mut Self, InternalError> {
                // SAFETY: guaranteed by the caller, and any initialized bytes form a valid value
                Ok(unsafe { &mut *this })
            }
        }
    )+};
}

// Aligned to 8 on the proving target as well.
const _: () = assert!(size_of::<u64>() == 8 && align_of::<u64>() == 8);

// `Uint<256, 4>` is `#[repr(transparent)]` over its little-endian limbs, `[u64; 4]`.
const _: () = assert!(size_of::<U256>() == 32 && align_of::<U256>() == 8);

// The same limbs, in a `#[repr(transparent)]` wrapper, or with delegation in a single-field wrapper of
// `#[repr(align(32))] [u64; 4]`: as the size of each wrapper is the size of its only field, the limbs are
// at offset 0.
const _: () = assert!(size_of::<u256::U256>() == 32 && align_of::<u256::U256>() >= 8);

// A single-field struct over its 32 bytes, stored as words (size asserted next to its definition).
const _: () = assert!(align_of::<Bytes32>() >= align_of::<u64>());

impl_continuous_for_plain_data!(u64, U256, u256::U256, Bytes32);

// `B160` is `Bits<160, 3>`, a single-field wrapper of `Uint<160, 3>`, which is `#[repr(transparent)]` over
// its little-endian limbs, `[u64; 3]`: as the wrapper has the size of its field, the limbs are at offset
// 0. A value fits in 160 bits: the top limb is at most `u32::MAX`.
const _: () = assert!(size_of::<B160>() == 24 && align_of::<B160>() == 8);

// SAFETY: see above; there is no padding
unsafe impl ContinuousSerializable for B160 {}

// SAFETY: see above; the only restriction on the limbs is checked
unsafe impl ContinuousDeserializable for B160 {
    #[inline(always)]
    unsafe fn validate<'a>(this: *mut Self) -> Result<&'a mut Self, InternalError> {
        // SAFETY: the top limb is the last 8 bytes of the value, initialized as per the caller contract
        let top_limb = unsafe { this.cast::<u64>().add(2).read() };
        if top_limb > u64::from(u32::MAX) {
            return Err(internal_error!("B160 from oracle does not fit in 160 bits"));
        }
        // SAFETY: guaranteed by the caller, and the value is valid
        Ok(unsafe { &mut *this })
    }
}

// SAFETY: the elements of an array are laid out back to back, without padding; the element type
// carries the rest
unsafe impl<T: ContinuousSerializable, const N: usize> ContinuousSerializable for [T; N] {}

// SAFETY: the elements of an array are laid out back to back, and every element is validated
unsafe impl<T: ContinuousDeserializable, const N: usize> ContinuousDeserializable for [T; N] {
    #[inline(always)]
    unsafe fn validate<'a>(this: *mut Self) -> Result<&'a mut Self, InternalError> {
        let first = this.cast::<T>();
        for i in 0..N {
            // SAFETY: element `i` is inside the array, aligned, and initialized as per the caller contract
            unsafe { T::validate(first.add(i))? };
        }
        // SAFETY: guaranteed by the caller, and every element is valid
        Ok(unsafe { &mut *this })
    }
}

// `#[repr(C)]`: the `bool` flag, padding up to the alignment of the value, and the value. The flag is
// normalized the way a short `bool` is decoded (any non-zero byte is `true`), the padding is ignored, and
// the value is validated by its type. With padding bytes, the type can only be received.
// SAFETY: see above
unsafe impl<IOTypes: SystemIOTypesConfig> ContinuousDeserializable
    for InitialStorageSlotData<IOTypes>
where
    IOTypes::StorageValue: ContinuousDeserializable,
{
    #[inline(always)]
    unsafe fn validate<'a>(this: *mut Self) -> Result<&'a mut Self, InternalError> {
        // SAFETY: both fields are inside the value, and initialized as per the caller contract
        unsafe {
            normalize_bool(&raw mut (*this).is_new_storage_slot);
            IOTypes::StorageValue::validate(&raw mut (*this).initial_value)?;
            Ok(&mut *this)
        }
    }
}
