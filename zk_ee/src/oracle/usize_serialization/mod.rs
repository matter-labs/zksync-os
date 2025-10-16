//! Type-safe serialization framework for oracle data exchange.
//! It serves as the core serialization layer for the oracle system, enabling
//! data exchange between ZKsync OS and external data providers.
//!
//! The serialization is based on `usize` sequences for cross-architecture compatibility.
//!
//! # Security Considerations
//!
//! This module handles endianness and pointer width differences between 32-bit
//! and 64-bit systems. The serialization format is designed to be deterministic across
//! architectures, but relies on consistent memory layout assumptions.

use core::mem::MaybeUninit;

use ruint::aliases::{B160, U256};

use crate::{
    internal_error,
    system::errors::internal::InternalError,
    utils::exact_size_chain::{ExactSizeChain, ExactSizeChainN},
};

pub mod dyn_usize_iterator;
#[cfg(test)]
mod tests;

/// Trait for types that can be serialized into a sequence of `usize` values with a known (fixed) length.
pub trait UsizeSerializable {
    const USIZE_LEN: usize;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize>;
}

/// Trait for types that can be deserialized from a sequence of `usize` values with a known (fixed) length.
pub trait UsizeDeserializable: Sized {
    const USIZE_LEN: usize;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError>;

    ///
    /// # Safety
    ///
    /// The correct layout of the serialization is enforced by the `from_iter`
    /// implementation, as long as the data in the external storage is correctly populated. It is a
    /// UB to read from any location that wasn't populated by this type before.
    ///
    unsafe fn init_from_iter(
        this: &mut MaybeUninit<Self>,
        src: &mut impl ExactSizeIterator<Item = usize>,
    ) -> Result<(), InternalError> {
        let new = UsizeDeserializable::from_iter(src)?;
        this.write(new);

        Ok(())
    }
}

impl UsizeSerializable for () {
    const USIZE_LEN: usize = 0;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        core::iter::empty()
    }
}

impl UsizeDeserializable for () {
    const USIZE_LEN: usize = 0;

    fn from_iter(_src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        Ok(())
    }
}

impl UsizeSerializable for u8 {
    const USIZE_LEN: usize = <u64 as UsizeSerializable>::USIZE_LEN;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        cfg_if::cfg_if!(
            if #[cfg(target_endian = "big")] {
                compile_error!("unsupported architecture: big endian arch is not supported")
            } else if #[cfg(target_pointer_width = "32")] {
                let low = *self as usize;
                let high = 0;
                return [low, high].into_iter();
            } else if #[cfg(target_pointer_width = "64")] {
                return core::iter::once(*self as usize)
            } else {
                compile_error!("unsupported architecture")
            }
        );
    }
}

impl UsizeDeserializable for u8 {
    const USIZE_LEN: usize = <Self as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let word = <u64 as UsizeDeserializable>::from_iter(src)?;
        if word > u8::MAX as u64 {
            return Err(internal_error!("u8 deserialization failed"));
        }
        Ok(word as u8)
    }
}

impl UsizeSerializable for bool {
    const USIZE_LEN: usize = <u64 as UsizeSerializable>::USIZE_LEN;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        cfg_if::cfg_if!(
            if #[cfg(target_endian = "big")] {
                compile_error!("unsupported architecture: big endian arch is not supported")
            } else if #[cfg(target_pointer_width = "32")] {
                let low = *self as usize;
                let high = 0;
                return [low, high].into_iter();
            } else if #[cfg(target_pointer_width = "64")] {
                return core::iter::once(*self as usize)
            } else {
                compile_error!("unsupported architecture")
            }
        );
    }
}

impl UsizeDeserializable for bool {
    const USIZE_LEN: usize = <Self as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let word = <u64 as UsizeDeserializable>::from_iter(src)?;
        if word == false as u64 {
            Ok(false)
        } else if word == true as u64 {
            Ok(true)
        } else {
            Err(internal_error!("bool deserialization failed"))
        }
    }
}

impl UsizeSerializable for u32 {
    const USIZE_LEN: usize = <u64 as UsizeSerializable>::USIZE_LEN;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        cfg_if::cfg_if!(
            if #[cfg(target_endian = "big")] {
                compile_error!("unsupported architecture: big endian arch is not supported")
            } else if #[cfg(target_pointer_width = "32")] {
                let low = *self as usize;
                let high = 0;
                return [low, high].into_iter();
            } else if #[cfg(target_pointer_width = "64")] {
                return core::iter::once(*self as usize)
            } else {
                compile_error!("unsupported architecture")
            }
        );
    }
}

impl UsizeDeserializable for u32 {
    const USIZE_LEN: usize = <Self as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let word = <u64 as UsizeDeserializable>::from_iter(src)?;
        if word > u32::MAX as u64 {
            return Err(internal_error!("u32 deserialization failed"));
        }
        Ok(word as u32)
    }
}

impl UsizeSerializable for u64 {
    const USIZE_LEN: usize = {
        cfg_if::cfg_if!(
            if #[cfg(target_endian = "big")] {
                compile_error!("unsupported architecture: big endian arch is not supported")
            } else if #[cfg(target_pointer_width = "32")] {
                let size = 2;
            } else if #[cfg(target_pointer_width = "64")] {
                let size = 1;
            } else {
                compile_error!("unsupported architecture")
            }
        );
        #[allow(clippy::let_and_return)]
        size
    };

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        cfg_if::cfg_if!(
            if #[cfg(target_endian = "big")] {
                compile_error!("unsupported architecture: big endian arch is not supported")
            } else if #[cfg(target_pointer_width = "32")] {
                let low = *self as usize;
                let high = (*self >> 32) as usize;
                return [low, high].into_iter();
            } else if #[cfg(target_pointer_width = "64")] {
                return core::iter::once(*self as usize)
            } else {
                compile_error!("unsupported architecture")
            }
        );
    }
}

impl UsizeDeserializable for u64 {
    const USIZE_LEN: usize = <u64 as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        cfg_if::cfg_if!(
            if #[cfg(target_endian = "big")] {
                compile_error!("unsupported architecture: big endian arch is not supported")
            } else if #[cfg(target_pointer_width = "32")] {
                let low = src.next().ok_or(internal_error!("u64 low deserialization failed"))?;
                let high = src.next().ok_or(internal_error!("u64 high deserialization failed"))?;
                return Ok(((high as u64) << 32) | (low as u64));
            } else if #[cfg(target_pointer_width = "64")] {
                let value = src.next().ok_or(internal_error!("u64 deserialization failed"))?;
                return Ok(value as u64);
            } else {
                compile_error!("unsupported architecture")
            }
        );
    }
}

impl UsizeSerializable for U256 {
    const USIZE_LEN: usize = <u64 as UsizeSerializable>::USIZE_LEN * 4;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        cfg_if::cfg_if!(
            if #[cfg(target_endian = "big")] {
                compile_error!("unsupported architecture: big endian arch is not supported")
            } else if #[cfg(target_pointer_width = "32")] {
                unsafe {
                    return core::mem::transmute::<Self, [u32; 8]>(*self).into_iter().map(|el| el as usize);
                }
            } else if #[cfg(target_pointer_width = "64")] {
                return self.as_limbs().map(|el| el as usize).into_iter();
            } else {
                compile_error!("unsupported architecture")
            }
        );
    }
}

impl UsizeDeserializable for U256 {
    const USIZE_LEN: usize = <U256 as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let mut new = MaybeUninit::uninit();
        unsafe {
            Self::init_from_iter(&mut new, src)?;

            Ok(new.assume_init())
        }
    }

    unsafe fn init_from_iter(
        this: &mut MaybeUninit<Self>,
        src: &mut impl ExactSizeIterator<Item = usize>,
    ) -> Result<(), InternalError> {
        cfg_if::cfg_if!(
            if #[cfg(target_endian = "big")] {
                compile_error!("unsupported architecture: big endian arch is not supported")
            } else if #[cfg(target_pointer_width = "32")] {
                for dst in this.assume_init_mut().as_limbs_mut() {
                    let low = src.next().ok_or(internal_error!("u256 limb low deserialization failed"))?;
                    let high = src.next().ok_or(internal_error!("u256 limb high deserialization failed"))?;
                    *dst = ((high as u64) << 32) | (low as u64);
                }
                return Ok(())
            } else if #[cfg(target_pointer_width = "64")] {
                for dst in this.assume_init_mut().as_limbs_mut() {
                    *dst = src.next().ok_or(internal_error!("u256 limb deserialization failed"))? as u64;
                }
                return Ok(())
            } else {
                compile_error!("unsupported architecture")
            }
        );
    }
}

impl UsizeSerializable for B160 {
    const USIZE_LEN: usize = const {
        cfg_if::cfg_if!(
            if #[cfg(target_endian = "big")] {
                compile_error!("unsupported architecture: big endian arch is not supported")
            } else if #[cfg(target_pointer_width = "32")] {
                let size = 6;
            } else if #[cfg(target_pointer_width = "64")] {
                let size = 3;
            } else {
                compile_error!("unsupported architecture")
            }
        );
        #[allow(clippy::let_and_return)]
        size
    };

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        cfg_if::cfg_if!(
            if #[cfg(target_endian = "big")] {
                compile_error!("unsupported architecture: big endian arch is not supported")
            } else if #[cfg(target_pointer_width = "32")] {
                unsafe {
                    return core::mem::transmute::<Self, [u32; 6]>(*self).into_iter().map(|el| el as usize);
                }
            } else if #[cfg(target_pointer_width = "64")] {
                return self.as_limbs().map(|el| el as usize).into_iter();
            } else {
                compile_error!("unsupported architecture")
            }
        );
    }
}

impl UsizeDeserializable for B160 {
    const USIZE_LEN: usize = <B160 as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        if src.len() < <Self as UsizeDeserializable>::USIZE_LEN {
            return Err(internal_error!("b160 deserialization failed: too short"));
        }
        let mut new = B160::ZERO;
        unsafe {
            for dst in new.as_limbs_mut().iter_mut() {
                *dst = <u64 as UsizeDeserializable>::from_iter(src).unwrap_unchecked();
            }
        }

        Ok(new)
    }

    unsafe fn init_from_iter(
        this: &mut MaybeUninit<Self>,
        src: &mut impl ExactSizeIterator<Item = usize>,
    ) -> Result<(), InternalError> {
        if src.len() < <Self as UsizeDeserializable>::USIZE_LEN {
            return Err(internal_error!("b160 deserialization failed: too short"));
        }
        for dst in this.assume_init_mut().as_limbs_mut().iter_mut() {
            *dst = <u64 as UsizeDeserializable>::from_iter(src).unwrap_unchecked();
        }

        Ok(())
    }
}

// for convenience - provide a simple case of tuple

impl<T: UsizeSerializable, U: UsizeSerializable> UsizeSerializable for (T, U) {
    const USIZE_LEN: usize =
        <T as UsizeSerializable>::USIZE_LEN + <U as UsizeSerializable>::USIZE_LEN;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        let (t, u) = self;
        ExactSizeChain::new(UsizeSerializable::iter(t), UsizeSerializable::iter(u))
    }
}

impl<T: UsizeDeserializable, U: UsizeDeserializable> UsizeDeserializable for (T, U) {
    const USIZE_LEN: usize =
        <T as UsizeDeserializable>::USIZE_LEN + <U as UsizeDeserializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let t = <T as UsizeDeserializable>::from_iter(src)?;
        let u = <U as UsizeDeserializable>::from_iter(src)?;
        Ok((t, u))
    }
}

// Only UsizeSerializable has a default impl
impl<T: UsizeSerializable, const N: usize> UsizeSerializable for [T; N] {
    const USIZE_LEN: usize = <T as UsizeSerializable>::USIZE_LEN * N;
    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        ExactSizeChainN::<_, _, N>::new(
            core::iter::empty::<usize>(),
            core::array::from_fn(|i| Some(UsizeSerializable::iter(&self[i]))),
        )
    }
}
