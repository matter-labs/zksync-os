use crate::{internal_error, system::errors::InternalError, utils::*};
use ruint::aliases::B160;

use super::*;

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
                return self.as_limbs().iter().map(|el| *el as usize);
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
                return self.as_limbs().iter().map(|el| *el as usize);
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

impl UsizeSerializable for Bytes32 {
    const USIZE_LEN: usize = const {
        cfg_if::cfg_if!(
            if #[cfg(target_endian = "big")] {
                compile_error!("unsupported architecture: big endian arch is not supported")
            } else if #[cfg(target_pointer_width = "32")] {
                let size = 8;
            } else if #[cfg(target_pointer_width = "64")] {
                let size = 4;
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
                return self.as_u32_array_ref().into_iter().map(|el| *el as usize);
            } else if #[cfg(target_pointer_width = "64")] {
                return self.as_u64_array_ref().iter().map(|el| *el as usize);
            } else {
                compile_error!("unsupported architecture")
            }
        );
    }
}

impl UsizeDeserializable for Bytes32 {
    const USIZE_LEN: usize = <Bytes32 as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        if src.len() < <Self as UsizeDeserializable>::USIZE_LEN {
            return Err(internal_error!("Bytes32 deserialization failed: too short"));
        }
        let mut new = Bytes32::ZERO;
        for dst in new.as_array_mut().iter_mut() {
            *dst = unsafe { src.next().unwrap_unchecked() };
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
        for dst in this.assume_init_mut().as_array_mut().iter_mut() {
            *dst = src.next().unwrap_unchecked()
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
