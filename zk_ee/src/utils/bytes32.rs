use crate::oracle::usize_serialization::{WordDeserializable, WordSerializable, WordSink};
use crate::system::errors::internal::InternalError;
use crate::internal_error;
use core::mem::MaybeUninit;
use ruint::aliases::{B160, U256};

#[cfg(target_pointer_width = "32")]
pub const BYTES32_USIZE_SIZE: usize = 8;

#[cfg(target_pointer_width = "64")]
pub const BYTES32_USIZE_SIZE: usize = 4;

#[repr(align(8))]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Bytes32 {
    inner: [usize; BYTES32_USIZE_SIZE],
}

const _: () = const {
    assert!(core::mem::size_of::<Bytes32>() == 32);
    assert!(core::mem::align_of::<Bytes32>() >= core::mem::align_of::<usize>());
};

// Comparison: byte-lexicographic on the underlying 32-byte view.
//
// On the RISC-V proving target, `<[u8]>::cmp` lowers to `compiler-builtins`'
// generic byte-by-byte `memcmp` — a hot path through `find_key_index` on
// BTreeMap lookups. We replace it with a word-by-word equality scan over the
// storage `usize`s (`cmp_word_chunked` below); on the first differing word
// we resolve byte-lex order by walking the bytes of just that word — cheaper
// than `swap_bytes()` on RV32 without the Zbb extension.
//
// On the host (forward mode), libc's `memcmp` is already SIMD-vectorized
// (SSE2/AVX/NEON), so the byte path beats a word loop in pure Rust. The
// `Ord` impl picks the right strategy via `#[cfg]`. The chunked helper is
// always compiled so its correctness is testable on the host as well.

impl Bytes32 {
    /// Word-chunked byte-lex compare. Used as the `Ord` impl on RV32 where
    /// the alternative is `compiler-builtins`' generic byte-by-byte `memcmp`.
    /// Observable ordering is identical to `as_u8_array_ref().cmp(...)` on
    /// either endianness.
    // Compiled on every target so the equivalence tests can validate it on the
    // host, but only wired into `Ord` on RV32 — non-test host builds see no
    // callers, hence the `dead_code` allow.
    #[cfg_attr(not(target_arch = "riscv32"), allow(dead_code))]
    #[inline]
    fn cmp_word_chunked(&self, other: &Self) -> core::cmp::Ordering {
        for i in 0..BYTES32_USIZE_SIZE {
            let a = self.inner[i];
            let b = other.inner[i];
            if a != b {
                // `to_ne_bytes()` returns the bytes of the word in their in-memory
                // order — identical to the slice produced by `as_u8_array_ref()`
                // for the same offsets, on either endianness.
                let a_bytes = a.to_ne_bytes();
                let b_bytes = b.to_ne_bytes();
                let mut j = 0;
                while j < core::mem::size_of::<usize>() {
                    if a_bytes[j] != b_bytes[j] {
                        return a_bytes[j].cmp(&b_bytes[j]);
                    }
                    j += 1;
                }
                // a != b guarantees at least one byte differs; this branch
                // is dead. Fall through to keep the function panic-free.
                return core::cmp::Ordering::Equal;
            }
        }
        core::cmp::Ordering::Equal
    }
}

#[cfg(target_arch = "riscv32")]
impl Ord for Bytes32 {
    #[inline]
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.cmp_word_chunked(other)
    }
}

#[cfg(not(target_arch = "riscv32"))]
impl Ord for Bytes32 {
    #[inline]
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.as_u8_array_ref().cmp(other.as_u8_array_ref())
    }
}

impl PartialOrd for Bytes32 {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl core::fmt::Debug for Bytes32 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "0x")?;
        for word in self.inner.iter() {
            #[cfg(target_pointer_width = "32")]
            write!(f, "{:08x}", word.to_be())?;

            #[cfg(target_pointer_width = "64")]
            write!(f, "{:016x}", word.to_be())?;
        }

        Ok(())
    }
}

impl Bytes32 {
    pub const ZERO: Self = Self {
        inner: [0usize; BYTES32_USIZE_SIZE],
    };

    pub const MAX: Self = Self {
        inner: [usize::MAX; BYTES32_USIZE_SIZE],
    };

    #[inline(always)]
    pub fn uninit() -> MaybeUninit<Self> {
        MaybeUninit::uninit()
    }

    pub fn from_byte_fill(byte: u8) -> Self {
        let mut buffer = 0usize.to_ne_bytes();
        buffer.fill(byte);
        let init_value = usize::from_ne_bytes(buffer);
        Self {
            inner: [init_value; BYTES32_USIZE_SIZE],
        }
    }

    #[inline(always)]
    pub const fn zero() -> Self {
        Self {
            inner: [0usize; BYTES32_USIZE_SIZE],
        }
    }

    #[inline(always)]
    pub fn from_array(array: [u8; 32]) -> Self {
        unsafe { core::mem::transmute_copy(&array) }
    }

    #[inline]
    pub const fn num_trailing_nonzero_bytes(&self) -> usize {
        #[cfg(target_endian = "big")]
        compile_error!("unsupported architecture: big endian arch is not supported");

        let mut result = 32;
        let mut i = 0;
        while i < BYTES32_USIZE_SIZE {
            let word = self.inner[i];
            if word == 0 {
                result -= core::mem::size_of::<usize>() as u32;
            } else {
                // NOTE - we should BE it, so it's TRAILING
                result -= word.trailing_zeros() / 8;
                break;
            }

            i += 1;
        }

        result as usize
    }

    #[allow(clippy::needless_as_bytes)]
    pub const fn from_hex(input: &str) -> Self {
        const fn hex_to_digit(c: u8) -> u8 {
            match c {
                b'A'..=b'F' => c - b'A' + 10,
                b'a'..=b'f' => c - b'a' + 10,
                b'0'..=b'9' => c - b'0',
                _ => {
                    unreachable!()
                }
            }
        }

        assert!(input.len() == 64);
        assert!(input.as_bytes().len() == 64); // ASCII check in essence
        let mut result = Self::ZERO;
        let mut idx = 0;
        let dst = result.as_u8_array_mut();
        let src = input.as_bytes().as_chunks::<2>().0;
        while idx < 32 {
            let dst = &mut dst[idx];
            let [high, low] = src[idx];
            let high = hex_to_digit(high);
            let low = hex_to_digit(low);
            *dst = (high << 4) | low;

            idx += 1;
        }

        result
    }

    pub fn is_zero(&self) -> bool {
        self.inner.iter().all(|el| *el == 0)
    }

    fn as_usize_array_mut(&mut self) -> &mut [usize; BYTES32_USIZE_SIZE] {
        &mut self.inner
    }

    #[cfg(target_pointer_width = "32")]
    fn as_u32_array_ref(&self) -> &[u32; 8] {
        unsafe { &*(&self.inner as *const usize).cast::<[u32; 8]>() }
    }

    #[cfg(target_pointer_width = "64")]
    pub fn as_u64_array_ref(&self) -> &[u64; 4] {
        unsafe { &*(&self.inner as *const usize).cast::<[u64; 4]>() }
    }

    pub fn as_u8_ref(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts((&self.inner as *const usize).cast::<u8>(), 32) }
    }

    pub fn as_u8_mut(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut((&mut self.inner as *mut usize).cast::<u8>(), 32) }
    }

    pub const fn as_u8_array(self) -> [u8; 32] {
        unsafe { core::mem::transmute(self) }
    }

    pub const fn as_u8_array_ref(&self) -> &[u8; 32] {
        unsafe { &*(&self.inner as *const usize).cast::<[u8; 32]>() }
    }

    pub const fn as_u8_array_mut(&mut self) -> &mut [u8; 32] {
        unsafe { &mut *(&mut self.inner as *mut usize).cast::<[u8; 32]>() }
    }

    pub fn bytereverse(&mut self) {
        cfg_if::cfg_if!(
            if #[cfg(target_endian = "big")] {
                compile_error!("unsupported architecture: big endian arch is not supported")
            } else if #[cfg(target_pointer_width = "32")] {
                self.inner.swap(0, 7);
                self.inner.swap(1, 6);
                self.inner.swap(2, 5);
                self.inner.swap(3, 4);
                for el in self.inner.iter_mut() {
                    *el = el.to_be();
                }
                return;
            } else if #[cfg(target_pointer_width = "64")] {
                self.inner.swap(0, 3);
                self.inner.swap(1, 2);
                for el in self.inner.iter_mut() {
                    // NOTE: we are on LE
                    *el = el.swap_bytes();
                }
                return;
            } else {
                compile_error!("unsupported architecture")
            }
        );
    }

    pub fn into_u256_le(self) -> U256 {
        cfg_if::cfg_if!(
            if #[cfg(target_endian = "big")] {
                compile_error!("unsupported architecture: big endian arch is not supported")
            } else {
                unsafe {
                    #[allow(clippy::missing_transmute_annotations)]
                    return core::mem::transmute(self);
                }
            }
        );
    }

    pub fn into_u256_be(self) -> U256 {
        U256::from_be_bytes(self.as_u8_array())
    }

    pub fn from_u256_le(value: &U256) -> Self {
        cfg_if::cfg_if!(
            if #[cfg(target_endian = "big")] {
                compile_error!("unsupported architecture: big endian arch is not supported")
            } else {
                unsafe {
                    #[allow(clippy::missing_transmute_annotations)]
                    return core::mem::transmute_copy(value);
                }
            }
        );
    }

    pub fn from_u256_be(value: &U256) -> Self {
        Self::from_array(value.to_be_bytes())
    }
}

// here we assume left-padding of zeroes for future
#[allow(clippy::from_over_into)]
impl Into<B160> for Bytes32 {
    fn into(self) -> B160 {
        // let's hope compiler optimizes it out
        let mut bytes = [0u8; 20];
        bytes.copy_from_slice(&self.as_u8_array_ref()[12..]);
        B160::from_be_bytes(bytes)
    }
}

impl From<B160> for Bytes32 {
    fn from(value: B160) -> Self {
        let mut new = Bytes32::zero();
        new.as_u8_array_mut()[12..].copy_from_slice(&value.to_be_bytes::<{ B160::BYTES }>()[..]);

        new
    }
}

impl From<[u8; 32]> for Bytes32 {
    fn from(value: [u8; 32]) -> Self {
        Self::from_array(value)
    }
}

impl WordSerializable for Bytes32 {
    fn word_len(&self) -> usize {
        BYTES32_USIZE_SIZE
    }

    fn write_words(&self, out: &mut impl WordSink) {
        cfg_if::cfg_if!(
            if #[cfg(target_endian = "big")] {
                compile_error!("unsupported architecture: big endian arch is not supported")
            } else if #[cfg(target_pointer_width = "32")] {
                for word in self.as_u32_array_ref() {
                    out.write_word(*word as usize);
                }
            } else if #[cfg(target_pointer_width = "64")] {
                for word in self.as_u64_array_ref() {
                    out.write_word(*word as usize);
                }
            } else {
                compile_error!("unsupported architecture")
            }
        );
    }
}

impl WordDeserializable for Bytes32 {
    fn read_words(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        if src.len() < BYTES32_USIZE_SIZE {
            return Err(internal_error!("Bytes32 deserialization failed: too short"));
        }
        let mut new = Bytes32::ZERO;
        for dst in new.as_usize_array_mut().iter_mut() {
            *dst = unsafe { src.next().unwrap_unchecked() };
        }

        Ok(new)
    }

    unsafe fn init_from_words(
        this: &mut MaybeUninit<Self>,
        src: &mut impl ExactSizeIterator<Item = usize>,
    ) -> Result<(), InternalError> {
        if src.len() < BYTES32_USIZE_SIZE {
            return Err(internal_error!("Bytes32 deserialization failed: too short"));
        }
        // Initialize
        let value: &mut Self = this.write(Self::ZERO);
        for dst in value.as_usize_array_mut().iter_mut() {
            *dst = src.next().unwrap_unchecked()
        }

        Ok(())
    }
}

#[cfg(test)]
mod cmp_tests {
    use super::Bytes32;
    use core::cmp::Ordering;

    fn reference_cmp(a: &Bytes32, b: &Bytes32) -> Ordering {
        a.as_u8_array_ref().cmp(b.as_u8_array_ref())
    }

    #[test]
    fn cmp_matches_byte_lex_on_handcrafted_pairs() {
        let cases: &[([u8; 32], [u8; 32])] = &[
            ([0; 32], [0; 32]),
            ([0; 32], [1; 32]),
            ([0xFF; 32], [0; 32]),
            ([0xFF; 32], [0xFF; 32]),
            (
                [
                    1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0,
                ],
                [
                    2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0,
                ],
            ),
            (
                [
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 1,
                ],
                [
                    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 2,
                ],
            ),
            (
                [
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 0xAA, 14, 15, 16, 17, 18, 19, 20, 21,
                    22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32,
                ],
                [
                    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 0xBB, 14, 15, 16, 17, 18, 19, 20, 21,
                    22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32,
                ],
            ),
        ];

        for (a, b) in cases {
            let lhs = Bytes32::from_array(*a);
            let rhs = Bytes32::from_array(*b);
            // Exercise the chunked helper directly so this test validates the
            // RV32 codepath even when the host build's `Ord` uses libc memcmp.
            assert_eq!(
                lhs.cmp_word_chunked(&rhs),
                reference_cmp(&lhs, &rhs),
                "mismatch on {:?} vs {:?}",
                a,
                b
            );
            assert_eq!(
                rhs.cmp_word_chunked(&lhs),
                reference_cmp(&rhs, &lhs),
                "reverse mismatch on {:?} vs {:?}",
                a,
                b
            );
        }
    }

    #[test]
    fn cmp_matches_byte_lex_on_pseudorandom_pairs() {
        // Deterministic LCG; exercises diff at every byte position.
        let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
        fn step(seed: &mut u64) -> u64 {
            *seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            *seed
        }

        for _ in 0..4096 {
            let mut a = [0u8; 32];
            let mut b = [0u8; 32];
            for chunk in a.chunks_exact_mut(8) {
                chunk.copy_from_slice(&step(&mut seed).to_le_bytes());
            }
            for chunk in b.chunks_exact_mut(8) {
                chunk.copy_from_slice(&step(&mut seed).to_le_bytes());
            }

            // Sometimes share a prefix so we hit varying mismatch positions.
            let prefix_len = (step(&mut seed) as usize) % 33;
            b[..prefix_len].copy_from_slice(&a[..prefix_len]);

            let lhs = Bytes32::from_array(a);
            let rhs = Bytes32::from_array(b);
            assert_eq!(
                lhs.cmp_word_chunked(&rhs),
                reference_cmp(&lhs, &rhs),
                "mismatch on {:?} vs {:?}",
                a,
                b
            );
        }
    }
}
