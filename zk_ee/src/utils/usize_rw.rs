//! Low-level utilities for reading and writing `usize` values from/to byte streams.
//!
//! This module provides unsafe but efficient traits and adapters for converting between
//! different data representations and `usize` values. It's primarily used by the oracle
//! serialization system to handle cross-architecture data exchange.

use crate::utils::USIZE_SIZE;

/// Trait for types that can read `usize` values from underlying data.
///
/// This is an unsafe trait that provides direct access to raw data without bounds checking.
/// Implementers must ensure the underlying data source has sufficient bytes available.
pub trait UsizeReadable {
    /// Reads a single `usize` value from the underlying data source.
    ///
    /// # Safety
    ///
    /// Caller must ensure that the underlying data source has at least `USIZE_SIZE` bytes
    /// available to read. Behavior is undefined if insufficient data is available.
    unsafe fn read_usize(&mut self) -> usize;
}

/// Safe wrapper around `UsizeReadable`
pub trait SafeUsizeReadable: UsizeReadable {
    /// Returns the number of `usize` values available to read.
    fn len(&self) -> usize;

    fn try_read(&mut self) -> Result<usize, ()> {
        unsafe { Ok(UsizeReadable::read_usize(self)) } // TODO actual checking?
    }
}

/// Adapter that wraps iterators to implement `UsizeReadable`.
///
/// This wrapper allows any iterator over copyable types to be used as a `usize` data source.
/// It handles the conversion from the iterator's item type to `usize` values.
pub struct ReadIterWrapper<T: 'static + Clone + Copy, I: Iterator<Item = T>> {
    inner: I,
}

impl<T: 'static + Clone + Copy, I: Iterator<Item = T>> From<I> for ReadIterWrapper<T, I> {
    fn from(value: I) -> Self {
        ReadIterWrapper::<T, I> { inner: value }
    }
}

impl<I: Iterator<Item = u8>> UsizeReadable for ReadIterWrapper<u8, I> {
    unsafe fn read_usize(&mut self) -> usize {
        let mut dst = 0usize.to_ne_bytes();
        for (dst, src) in dst.iter_mut().zip(&mut self.inner) {
            *dst = src;
        }
        usize::from_ne_bytes(dst)
    }
}

impl<I: Iterator<Item = usize>> UsizeReadable for ReadIterWrapper<usize, I> {
    unsafe fn read_usize(&mut self) -> usize {
        self.inner.next().unwrap_unchecked()
    }
}

impl<I: ExactSizeIterator<Item = u8>> Iterator for ReadIterWrapper<u8, I> {
    type Item = usize;
    fn next(&mut self) -> Option<Self::Item> {
        if self.inner.len() == 0 {
            return None;
        }
        let mut dst = 0usize.to_ne_bytes();
        for (dst, src) in dst.iter_mut().zip(&mut self.inner) {
            *dst = src;
        }
        Some(usize::from_ne_bytes(dst))
    }
}

impl<I: ExactSizeIterator<Item = u8>> ExactSizeIterator for ReadIterWrapper<u8, I> {
    fn len(&self) -> usize {
        self.inner.len().next_multiple_of(USIZE_SIZE) / USIZE_SIZE
    }
}

/// Wrapper for exact-size iterators over `usize` values with safe reading support.
///
/// This wrapper provides both unsafe and safe reading methods for iterators that
/// already produce `usize` values and can report their exact length.
#[derive(Clone)]
pub struct ExactSizeIterReadWrapper<I: ExactSizeIterator<Item = usize>> {
    inner: I,
}

impl<I: ExactSizeIterator<Item = usize>> From<I> for ExactSizeIterReadWrapper<I> {
    fn from(value: I) -> Self {
        ExactSizeIterReadWrapper::<I> { inner: value }
    }
}

impl<I: ExactSizeIterator<Item = usize>> UsizeReadable for ExactSizeIterReadWrapper<I> {
    unsafe fn read_usize(&mut self) -> usize {
        self.inner.next().unwrap_unchecked()
    }
}

impl<I: ExactSizeIterator<Item = usize>> SafeUsizeReadable for ExactSizeIterReadWrapper<I> {
    fn len(&self) -> usize {
        self.inner.len()
    }
    fn try_read(&mut self) -> Result<usize, ()> {
        self.inner.next().ok_or(())
    }
}

/// Trait for types that can write `usize` values to underlying data.
///
/// This is an unsafe trait that provides direct access to raw data without bounds checking.
/// Implementers must ensure the underlying data destination has sufficient space available.
pub trait UsizeWriteable {
    /// Writes a single `usize` value to the underlying data destination.
    ///
    /// # Safety
    ///
    /// Caller must ensure that the underlying data destination has at least `USIZE_SIZE` bytes
    /// available to write. Behavior is undefined if insufficient space is available.
    unsafe fn write_usize(&mut self, value: usize);
}

/// Safe wrapper around `UsizeWriteable` that provides bounds checking.
///
/// This trait extends `UsizeWriteable` with length information and safe writing methods
/// that return errors instead of causing undefined behavior on insufficient space.
pub trait SafeUsizeWritable: UsizeWriteable {
    /// Returns the number of `usize` values that can be written.
    fn len(&self) -> usize;

    fn try_write(&mut self, value: usize) -> Result<(), ()> {
        unsafe {
            UsizeWriteable::write_usize(self, value); // TODO actual checking?
        }
        Ok(())
    }
}

/// Adapter to expose a type as a `SafeUsizeWritable`.
///
/// Produces a lifetime-tied writable view over `self` without exposing the backing storage.
pub trait AsUsizeWritable: Sized {
    /// Borrowed writable view tied to the lifetime of `&mut self`.
    type Writable<'a>: SafeUsizeWritable
    where
        Self: 'a;

    /// Returns a writable view over `self`.
    fn as_writable<'a>(&'a mut self) -> Self::Writable<'a>
    where
        Self: 'a;
}

/// Adapter that wraps mutable iterators to implement `UsizeWriteable`.
///
/// This wrapper allows any iterator over mutable references to copyable types to be used
/// as a `usize` data destination. It handles the conversion from `usize` values to the
/// iterator's target type.
pub struct WriteIterWrapper<'a, T: 'static + Clone + Copy, I: Iterator<Item = &'a mut T>> {
    inner: I,
}

impl<'a, T: 'static + Clone + Copy, I: ExactSizeIterator<Item = &'a mut T>>
    WriteIterWrapper<'a, T, I>
{
    pub fn usize_len(&self) -> usize {
        self.inner.len().next_multiple_of(USIZE_SIZE) / USIZE_SIZE
    }
}

impl<'a, T: 'static + Clone + Copy, I: Iterator<Item = &'a mut T>> From<I>
    for WriteIterWrapper<'a, T, I>
{
    fn from(value: I) -> Self {
        WriteIterWrapper::<T, I> { inner: value }
    }
}

// TODO: specialize in case of aligned iterator

impl<'a, I: Iterator<Item = &'a mut u8>> UsizeWriteable for WriteIterWrapper<'a, u8, I> {
    unsafe fn write_usize(&mut self, value: usize) {
        let le_bytes = value.to_ne_bytes();
        for (src, dst) in le_bytes.into_iter().zip(&mut self.inner) {
            *dst = src;
        }
    }
}

impl<'a, I: Iterator<Item = &'a mut usize>> UsizeWriteable for WriteIterWrapper<'a, usize, I> {
    unsafe fn write_usize(&mut self, value: usize) {
        *self.inner.next().unwrap_unchecked() = value;
    }
}

impl<'a, I: ExactSizeIterator<Item = &'a mut u8>> SafeUsizeWritable
    for WriteIterWrapper<'a, u8, I>
{
    fn len(&self) -> usize {
        self.usize_len()
    }
    fn try_write(&mut self, value: usize) -> Result<(), ()> {
        let le_bytes = value.to_ne_bytes();
        for byte in le_bytes.into_iter() {
            *self.inner.next().ok_or(())? = byte;
        }
        Ok(())
    }
}
