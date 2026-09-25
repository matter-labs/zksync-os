//! Values exchanged with the oracle through memory: passed by address, received memcpy-like.

use crate::system::errors::internal::InternalError;
use core::mem::{align_of, size_of};

/// A value that is passed to the oracle by address: the oracle reads the `size_of::<Self>()` bytes of the
/// value straight from the memory of the querier.
///
/// # Safety
///
/// The implementor guarantees that:
/// - the representation of `Self` is fully defined and the same on every target running ZKsync OS (the
///   host and the proving target): the size and the meaning of every byte, so the oracle can interpret
///   them; only the alignment may differ. This holds for fixed-width integers, arrays, and `#[repr(C)]`
///   or `#[repr(transparent)]` structs of those, but not for `usize`, `isize`, pointers or references;
/// - `Self` has no padding bytes, so the oracle may read any value of it as plain words;
/// - `size_of::<Self>()` is a non-zero multiple of 4, and `align_of::<Self>()` is at least 4. This part is
///   also checked at compile time wherever the type is sent to the oracle.
pub unsafe trait ContinuousSerializable: Sized {}

/// A value that the oracle writes memcpy-like: the oracle overwrites all `size_of::<Self>()` bytes of the
/// destination with the untrusted response, padding bytes included, and [`validate`](Self::validate) then
/// checks the fields whose representation is restricted.
///
/// # Safety
///
/// The implementor guarantees that:
/// - the representation of `Self` is fully defined and the same on every target running ZKsync OS, as
///   for [`ContinuousSerializable`], except that padding bytes are allowed;
/// - `size_of::<Self>()` is a non-zero multiple of 4, and `align_of::<Self>()` is at least 4. This part is
///   also checked at compile time wherever the type is received from the oracle;
/// - [`validate`](Self::validate) only returns `Ok` if the bytes, after the normalization it may apply,
///   form a valid value of `Self`.
pub unsafe trait ContinuousDeserializable: Sized {
    /// Validates the value that the oracle has just written at `this`, normalizing some of its fields in
    /// place if needed, and returns it as a reference.
    ///
    /// Implementations must only read the bytes through types for which any bit pattern is valid
    /// (integers), never through a reference to `Self`, or to a restricted field before it is checked.
    ///
    /// # Safety
    ///
    /// `this` must be aligned and valid for reads and writes of `Self` for `'a`, and must not be accessed
    /// through any other pointer during `'a`. All its bytes must be initialized, as they are after
    /// [`MemoryOracle::write`](super::MemoryOracle::write), but they may have any values.
    unsafe fn validate<'a>(this: *mut Self) -> Result<&'a mut Self, InternalError>;
}

/// Checks that the oracle can exchange a `T` as whole aligned `u32` words. To be evaluated in a `const`
/// block, so that a violation fails compilation.
pub(crate) const fn assert_continuous_layout<T>() {
    assert!(size_of::<T>() > 0, "oracle values must not be zero-sized");
    assert!(
        size_of::<T>().is_multiple_of(size_of::<u32>()),
        "oracle values must consist of whole u32 words"
    );
    assert!(
        align_of::<T>() >= align_of::<u32>(),
        "oracle values must be aligned for u32"
    );
}

/// Number of `u32` words in a `T`. To be evaluated in a `const` block.
pub(crate) const fn continuous_words<T>() -> usize {
    const { assert_continuous_layout::<T>() };
    size_of::<T>() / size_of::<u32>()
}

/// The address to give to the oracle for `value`. The provenance is exposed, as the oracle reads the value
/// through this address.
#[inline(always)]
pub(crate) fn exposed_address<T: ContinuousSerializable>(value: &T) -> usize {
    const { assert_continuous_layout::<T>() }
    core::ptr::from_ref(value).expose_provenance()
}

/// Normalizes a `bool` field of a value written by the oracle, the same way a short `bool` is decoded: any
/// non-zero byte is `true`. For use in [`ContinuousDeserializable::validate`].
///
/// # Safety
///
/// `field` must be valid for reads and writes of one initialized byte.
#[inline(always)]
pub unsafe fn normalize_bool(field: *mut bool) {
    let field = field.cast::<u8>();
    // SAFETY: guaranteed by the caller; the byte is read as `u8`, for which any value is valid
    unsafe { field.write(u8::from(field.read() != 0)) };
}
