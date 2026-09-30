//! The code of a frame in the form the v2 interpreter runs it: followed by a zero byte. In a
//! module of its own, so that its fields are private to it and not to the whole crate.

use core::alloc::Allocator;
use core::marker::PhantomData;

use alloc::vec::Vec;

use crate::CODE_PADDING_BYTES;

/// Empty code with a zero byte after it: the code of an interpreter that was never given a
/// frame, whose run fetches that byte as its `STOP`
static EMPTY_CODE: [u8; 1] = [0];

/// The code of a frame, followed in the same allocation by a zero byte that is not written
/// while the code is in use. The proving fetch of the v2 interpreter has no end check on
/// unpadded code: at the end of the code it reads that byte, i.e. a `STOP` (see
/// `v2::hot::Hot::fetch_and_advance`).
///
/// Only the constructors make one, and each provides the byte: a padded copy owned by the
/// value ([`Self::padded_copy`]), a borrowed buffer checked to have it
/// ([`Self::borrow_followed_by_zero`]), a buffer of padded code ([`Self::borrow_padded`],
/// unsafe), or the static empty code ([`Self::empty`]). The pointer of the code is taken from
/// a span that covers the byte too, so that reading it through the pointer stays in bounds of
/// what the pointer was derived from.
pub(crate) struct CodeWithSentinel<'a, A: Allocator> {
    /// Start of the code: the `len + 1` bytes from it are readable as long as `self` lives,
    /// and the last of them is zero
    start: *const u8,
    /// Length of the code
    len: usize,
    /// The padded copy that `start` points into, if the code is a copy. Not touched after
    /// construction (only dropped), so its heap buffer is neither moved nor written while
    /// `self` lives
    #[allow(dead_code)]
    owned: Option<Vec<u8, A>>,
    _borrow: PhantomData<&'a [u8]>,
}

impl<'a, A: Allocator> CodeWithSentinel<'a, A> {
    /// Empty code (the static zero byte)
    #[inline]
    pub(crate) fn empty() -> Self {
        Self {
            start: EMPTY_CODE.as_ptr(),
            len: 0,
            owned: None,
            _borrow: PhantomData,
        }
    }

    /// A copy of `code` with `CODE_PADDING_BYTES` zero bytes after it (what padded code has)
    pub(crate) fn padded_copy(allocator: A, code: &[u8]) -> Self {
        let mut buffer = Vec::with_capacity_in(code.len() + CODE_PADDING_BYTES, allocator);
        buffer.extend_from_slice(code);
        buffer.resize(code.len() + CODE_PADDING_BYTES, 0);
        Self {
            // the heap buffer of the copy, which moving the `Vec` does not move
            start: buffer.as_ptr(),
            len: code.len(),
            owned: Some(buffer),
            _borrow: PhantomData,
        }
    }

    /// [`Self::padded_copy`], out of line: the preimage layout makes code without a zero byte
    /// after it rare
    #[cold]
    #[inline(never)]
    pub(crate) fn padded_copy_cold(allocator: A, code: &[u8]) -> Self {
        Self::padded_copy(allocator, code)
    }

    /// The first `len` bytes of `bytecode`, if `bytecode` has a zero byte after them
    #[inline(always)]
    pub(crate) fn borrow_followed_by_zero(bytecode: &'a [u8], len: usize) -> Option<Self> {
        let span = bytecode.get(..=len)?;
        if span[len] == 0 {
            Some(Self {
                start: span.as_ptr(),
                len,
                owned: None,
                _borrow: PhantomData,
            })
        } else {
            None
        }
    }

    /// The first `len` bytes of `bytecode`, which is code of a padded type
    /// (`EthereumLikeTypes::CODE_IS_PADDED`) followed by its padding.
    ///
    /// # Safety
    /// `bytecode` holds the `len` bytes of the code and the `CODE_PADDING_BYTES` zero bytes
    /// after them (the pointer of the code is derived from `bytecode`), and they are not
    /// written for `'a`.
    #[inline(always)]
    pub(crate) unsafe fn borrow_padded(bytecode: &'a [u8], len: usize) -> Self {
        debug_assert!(len.saturating_add(CODE_PADDING_BYTES) <= bytecode.len());
        Self {
            start: bytecode.as_ptr(),
            len,
            owned: None,
            _borrow: PhantomData,
        }
    }

    /// Start of the code, from which the `len() + 1` bytes are readable (the last is zero)
    #[inline(always)]
    pub(crate) fn as_ptr(&self) -> *const u8 {
        self.start
    }

    /// Length of the code
    #[inline(always)]
    pub(crate) fn len(&self) -> usize {
        self.len
    }

    /// The code
    #[inline(always)]
    pub(crate) fn as_slice(&self) -> &[u8] {
        // SAFETY: the first `len` of the readable bytes (see `start`)
        unsafe { core::slice::from_raw_parts(self.start, self.len) }
    }

    /// The byte after the code, which is zero
    #[inline(always)]
    pub(crate) fn sentinel(&self) -> u8 {
        // SAFETY: the last of the readable bytes (see `start`)
        unsafe { self.start.add(self.len).read() }
    }

    /// Whether the code is a copy owned by `self`
    #[cfg(test)]
    pub(crate) fn is_copy(&self) -> bool {
        self.owned.is_some()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use alloc::alloc::Global;

    /// The code, and the byte after it read through the pointer (Miri checks that the pointer
    /// was derived from a span that covers the byte)
    pub(crate) fn code_and_sentinel<'c>(code: &'c CodeWithSentinel<'_, Global>) -> (&'c [u8], u8) {
        // SAFETY: the byte after the code is readable through the pointer
        let through_pointer = unsafe { code.as_ptr().add(code.len()).read() };
        assert_eq!(through_pointer, code.sentinel());
        (code.as_slice(), through_pointer)
    }

    #[test]
    fn empty_and_empty_copy() {
        let placeholder = CodeWithSentinel::<Global>::empty();
        assert_eq!(code_and_sentinel(&placeholder), (&[][..], 0));
        assert!(!placeholder.is_copy());
        let copy = CodeWithSentinel::padded_copy(Global, &[]);
        assert_eq!(code_and_sentinel(&copy), (&[][..], 0));
        assert_eq!(copy.owned.as_ref().unwrap().len(), CODE_PADDING_BYTES);
    }

    /// A borrow covers exactly the code and the zero byte after it; nothing else is checked
    #[test]
    fn borrow_needs_a_zero_byte_inside_the_slice() {
        let span = [0x60, 0x01, 0x00];
        // The slice ends at the zero byte: the pointer is derived from all of it.
        let borrowed = CodeWithSentinel::<Global>::borrow_followed_by_zero(&span, 2).unwrap();
        assert_eq!(code_and_sentinel(&borrowed), (&span[..2], 0));
        assert_eq!(borrowed.as_ptr(), span.as_ptr());
        assert!(!borrowed.is_copy());
        assert!(CodeWithSentinel::<Global>::borrow_followed_by_zero(&span, 3).is_none());
        assert!(CodeWithSentinel::<Global>::borrow_followed_by_zero(&span, usize::MAX).is_none());
        assert!(CodeWithSentinel::<Global>::borrow_followed_by_zero(&span, 1).is_none());
        let empty = CodeWithSentinel::<Global>::borrow_followed_by_zero(&span[2..], 0).unwrap();
        assert_eq!(code_and_sentinel(&empty), (&[][..], 0));
    }

    /// The copy is independent of the code it was made from, and moving it (as frames move
    /// their parts) keeps its pointer valid
    #[test]
    fn copy_outlives_its_source_and_survives_moves() {
        let source = alloc::vec![0x5b, 0x60, 0x2a, 0x56];
        let copy = CodeWithSentinel::padded_copy(Global, &source);
        drop(source);
        let mut moved = alloc::vec::Vec::new();
        moved.push(copy);
        moved.push(CodeWithSentinel::empty());
        let copy = moved.swap_remove(0);
        let boxed = alloc::boxed::Box::new(copy);
        assert!(boxed.is_copy());
        assert_eq!(
            code_and_sentinel(&boxed),
            (&[0x5b, 0x60, 0x2a, 0x56][..], 0)
        );
        // SAFETY: a copy has `CODE_PADDING_BYTES` zero bytes after the code
        let padding =
            unsafe { core::slice::from_raw_parts(boxed.as_ptr().add(4), CODE_PADDING_BYTES) };
        assert!(padding.iter().all(|byte| *byte == 0));
    }

    /// Padded code (the Ethereum storage model): the pointer is derived from the whole buffer,
    /// which holds the padding
    #[test]
    fn borrow_padded_buffer() {
        let mut buffer = [0u8; 48];
        buffer[..3].copy_from_slice(&[0x60, 0x01, 0x7f]);
        // SAFETY: 45 zero bytes follow the code in `buffer`, which is not written while borrowed
        let code = unsafe { CodeWithSentinel::<Global>::borrow_padded(&buffer, 3) };
        assert_eq!(code_and_sentinel(&code), (&[0x60, 0x01, 0x7f][..], 0));
        // SAFETY: as above
        let padding =
            unsafe { core::slice::from_raw_parts(code.as_ptr().add(3), CODE_PADDING_BYTES) };
        assert!(padding.iter().all(|byte| *byte == 0));
    }
}
