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

use alloc::vec::Vec;
use core::mem::MaybeUninit;

use ruint::aliases::{B160, U256};
pub use word_serialization_derive::{WordDeserializable, WordSerializable};

use crate::{
    internal_error,
    system::errors::internal::InternalError,
};

pub mod dyn_word_iterator;
#[cfg(test)]
mod tests;

/// A sink for oracle transport words.
pub trait WordSink {
    fn write_word(&mut self, word: usize);
}

impl WordSink for Vec<usize> {
    fn write_word(&mut self, word: usize) {
        self.push(word);
    }
}

impl<const N: usize> WordSink for arrayvec::ArrayVec<usize, N> {
    fn write_word(&mut self, word: usize) {
        self.push(word);
    }
}

/// Serialization into oracle transport words.
pub trait WordSerializable {
    /// Returns the number of transport words that `write_words()` will emit.
    ///
    /// This is not just a convenience helper: the proving-side oracle transport writes
    /// the payload length before the payload itself, so it must know the word count
    /// up front without first materializing a temporary buffer.
    fn word_len(&self) -> usize;

    fn write_words(&self, out: &mut impl WordSink);

    fn to_word_vec(&self) -> Vec<usize> {
        let mut out = Vec::with_capacity(self.word_len());
        self.write_words(&mut out);
        debug_assert_eq!(out.len(), self.word_len());
        out
    }
}

/// Deserialization from oracle transport words.
pub trait WordDeserializable: Sized {
    fn read_words(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError>;

    ///
    /// # Safety
    ///
    /// The correct layout of the serialization is enforced by the `read_words`
    /// implementation, as long as the data in the external storage is correctly populated. It is a
    /// UB to read from any location that wasn't populated by this type before.
    ///
    unsafe fn init_from_words(
        this: &mut MaybeUninit<Self>,
        src: &mut impl ExactSizeIterator<Item = usize>,
    ) -> Result<(), InternalError> {
        let new = Self::read_words(src)?;
        this.write(new);

        Ok(())
    }
}

impl WordSerializable for () {
    fn word_len(&self) -> usize {
        0
    }

    fn write_words(&self, _out: &mut impl WordSink) {}
}

impl WordDeserializable for () {
    fn read_words(_src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        Ok(())
    }
}

impl WordSerializable for u8 {
    fn word_len(&self) -> usize {
        <u64 as WordSerializable>::word_len(&(*self as u64))
    }

    fn write_words(&self, out: &mut impl WordSink) {
        <u64 as WordSerializable>::write_words(&(*self as u64), out);
    }
}

impl WordDeserializable for u8 {
    fn read_words(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let word = <u64 as WordDeserializable>::read_words(src)?;
        if word > u8::MAX as u64 {
            return Err(internal_error!("u8 deserialization failed"));
        }
        Ok(word as u8)
    }
}

impl WordSerializable for bool {
    fn word_len(&self) -> usize {
        <u64 as WordSerializable>::word_len(&(*self as u64))
    }

    fn write_words(&self, out: &mut impl WordSink) {
        <u64 as WordSerializable>::write_words(&(*self as u64), out);
    }
}

impl WordDeserializable for bool {
    fn read_words(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let word = <u64 as WordDeserializable>::read_words(src)?;
        if word == false as u64 {
            Ok(false)
        } else if word == true as u64 {
            Ok(true)
        } else {
            Err(internal_error!("bool deserialization failed"))
        }
    }
}

impl WordSerializable for u32 {
    fn word_len(&self) -> usize {
        <u64 as WordSerializable>::word_len(&(*self as u64))
    }

    fn write_words(&self, out: &mut impl WordSink) {
        <u64 as WordSerializable>::write_words(&(*self as u64), out);
    }
}

impl WordDeserializable for u32 {
    fn read_words(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let word = <u64 as WordDeserializable>::read_words(src)?;
        if word > u32::MAX as u64 {
            return Err(internal_error!("u32 deserialization failed"));
        }
        Ok(word as u32)
    }
}

impl WordSerializable for u64 {
    fn word_len(&self) -> usize {
        cfg_if::cfg_if!(
            if #[cfg(target_endian = "big")] {
                compile_error!("unsupported architecture: big endian arch is not supported")
            } else if #[cfg(target_pointer_width = "32")] {
                return 2;
            } else if #[cfg(target_pointer_width = "64")] {
                return 1;
            } else {
                compile_error!("unsupported architecture")
            }
        );
    }

    fn write_words(&self, out: &mut impl WordSink) {
        cfg_if::cfg_if!(
            if #[cfg(target_endian = "big")] {
                compile_error!("unsupported architecture: big endian arch is not supported")
            } else if #[cfg(target_pointer_width = "32")] {
                out.write_word(*self as usize);
                out.write_word((*self >> 32) as usize);
            } else if #[cfg(target_pointer_width = "64")] {
                out.write_word(*self as usize);
            } else {
                compile_error!("unsupported architecture")
            }
        );
    }
}

impl WordDeserializable for u64 {
    fn read_words(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
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

impl WordSerializable for U256 {
    fn word_len(&self) -> usize {
        <u64 as WordSerializable>::word_len(&0) * 4
    }

    fn write_words(&self, out: &mut impl WordSink) {
        cfg_if::cfg_if!(
            if #[cfg(target_endian = "big")] {
                compile_error!("unsupported architecture: big endian arch is not supported")
            } else if #[cfg(target_pointer_width = "32")] {
                unsafe {
                    for limb in core::mem::transmute::<Self, [u32; 8]>(*self) {
                        out.write_word(limb as usize);
                    }
                }
            } else if #[cfg(target_pointer_width = "64")] {
                for limb in self.as_limbs() {
                    out.write_word(*limb as usize);
                }
            } else {
                compile_error!("unsupported architecture")
            }
        );
    }
}

impl WordDeserializable for U256 {
    fn read_words(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let mut new = MaybeUninit::uninit();
        unsafe {
            Self::init_from_words(&mut new, src)?;

            Ok(new.assume_init())
        }
    }

    unsafe fn init_from_words(
        this: &mut MaybeUninit<Self>,
        src: &mut impl ExactSizeIterator<Item = usize>,
    ) -> Result<(), InternalError> {
        let value: &mut U256 = this.write(U256::ZERO);

        cfg_if::cfg_if! {
            if #[cfg(target_endian = "big")] {
                compile_error!("unsupported architecture: big endian arch is not supported")
            } else if #[cfg(target_pointer_width = "32")] {
                for dst in value.as_limbs_mut() {
                    let low = src
                        .next()
                        .ok_or(internal_error!("u256 limb low deserialization failed"))?;
                    let high = src
                        .next()
                        .ok_or(internal_error!("u256 limb high deserialization failed"))?;
                    *dst = ((high as u64) << 32) | (low as u64);
                }
                Ok(())
            } else if #[cfg(target_pointer_width = "64")] {
                for dst in value.as_limbs_mut() {
                    *dst = src
                        .next()
                        .ok_or(internal_error!("u256 limb deserialization failed"))? as u64;
                }
                Ok(())
            } else {
                compile_error!("unsupported architecture")
            }
        }
    }
}

impl WordSerializable for B160 {
    fn word_len(&self) -> usize {
        cfg_if::cfg_if!(
            if #[cfg(target_endian = "big")] {
                compile_error!("unsupported architecture: big endian arch is not supported")
            } else if #[cfg(target_pointer_width = "32")] {
                return 6;
            } else if #[cfg(target_pointer_width = "64")] {
                return 3;
            } else {
                compile_error!("unsupported architecture")
            }
        );
    }

    fn write_words(&self, out: &mut impl WordSink) {
        cfg_if::cfg_if!(
            if #[cfg(target_endian = "big")] {
                compile_error!("unsupported architecture: big endian arch is not supported")
            } else if #[cfg(target_pointer_width = "32")] {
                unsafe {
                    for limb in core::mem::transmute::<Self, [u32; 6]>(*self) {
                        out.write_word(limb as usize);
                    }
                }
            } else if #[cfg(target_pointer_width = "64")] {
                for limb in self.as_limbs() {
                    out.write_word(*limb as usize);
                }
            } else {
                compile_error!("unsupported architecture")
            }
        );
    }
}

impl WordDeserializable for B160 {
    fn read_words(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let mut new = MaybeUninit::uninit();
        unsafe {
            Self::init_from_words(&mut new, src)?;
            Ok(new.assume_init())
        }
    }

    unsafe fn init_from_words(
        this: &mut MaybeUninit<Self>,
        src: &mut impl ExactSizeIterator<Item = usize>,
    ) -> Result<(), InternalError> {
        if src.len() < Self::word_len(&B160::ZERO) {
            return Err(internal_error!("b160 deserialization failed: too short"));
        }
        let mut limbs = [0u64; B160::LIMBS];
        for limb in &mut limbs {
            *limb = unsafe { <u64 as WordDeserializable>::read_words(src).unwrap_unchecked() };
        }
        this.write(B160::from_limbs(limbs));

        Ok(())
    }
}

impl<T: WordSerializable, U: WordSerializable> WordSerializable for (T, U) {
    fn word_len(&self) -> usize {
        let (t, u) = self;
        t.word_len() + u.word_len()
    }

    fn write_words(&self, out: &mut impl WordSink) {
        let (t, u) = self;
        t.write_words(out);
        u.write_words(out);
    }
}

impl<T: WordDeserializable, U: WordDeserializable> WordDeserializable for (T, U) {
    fn read_words(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let t = T::read_words(src)?;
        let u = U::read_words(src)?;
        Ok((t, u))
    }
}

impl<T: WordSerializable, const N: usize> WordSerializable for [T; N] {
    fn word_len(&self) -> usize {
        self.iter().map(WordSerializable::word_len).sum()
    }

    fn write_words(&self, out: &mut impl WordSink) {
        for element in self {
            element.write_words(out);
        }
    }
}

impl<T: WordDeserializable, const N: usize> WordDeserializable for [T; N] {
    fn read_words(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let mut out: [MaybeUninit<T>; N] = unsafe { MaybeUninit::uninit().assume_init() };
        let mut initialized = 0;

        while initialized < N {
            match T::read_words(src) {
                Ok(value) => {
                    out[initialized].write(value);
                    initialized += 1;
                }
                Err(err) => {
                    for value in &mut out[..initialized] {
                        unsafe { value.assume_init_drop() };
                    }
                    return Err(err);
                }
            }
        }

        let out = core::mem::ManuallyDrop::new(out);
        let ptr = (&*out as *const [MaybeUninit<T>; N]).cast::<[T; N]>();
        Ok(unsafe { ptr.read() })
    }
}
