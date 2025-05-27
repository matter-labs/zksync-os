// Based on https://github.com/RustCrypto/sponges/blob/13e501fa4e9b7d23c550a53f75e27ec291afc32a/keccak/src/lib.rs implementation

#![cfg_attr(not(test), no_std)]
#![allow(clippy::new_without_default)]
#![allow(clippy::needless_borrow)]

//! Keccak [sponge function](https://en.wikipedia.org/wiki/Sponge_function).
//!
//! If you are looking for SHA-3 hash functions take a look at [`sha3`][1] and
//! [`tiny-keccak`][2] crates.
//!
//! To disable loop unrolling (e.g. for constraint targets) use `no_unroll`
//! feature.
//!
//! ```
//! // Test vectors are from KeccakCodePackage
//! let mut data = [0u64; 25];
//!
//! const_keccak256::keccak_f1600(&mut data);
//! assert_eq!(data, [
//!     0xF1258F7940E1DDE7, 0x84D5CCF933C0478A, 0xD598261EA65AA9EE, 0xBD1547306F80494D,
//!     0x8B284E056253D057, 0xFF97A42D7F8E6FD4, 0x90FEE5A0A44647C4, 0x8C5BDA0CD6192E76,
//!     0xAD30A6F71B19059C, 0x30935AB7D08FFC64, 0xEB5AA93F2317D635, 0xA9A6E6260D712103,
//!     0x81A57C16DBCF555F, 0x43B831CD0347C826, 0x01F22F1A11A5569F, 0x05E5635A21D9AE61,
//!     0x64BEFEF28CC970F2, 0x613670957BC46611, 0xB87C5A554FD00ECB, 0x8C3EE88A1CCF32C8,
//!     0x940C7922AE3A2614, 0x1841F924A2C509E4, 0x16F53526E70465C2, 0x75F644E97F30A13B,
//!     0xEAF1FF7B5CECA249,
//! ]);
//!
//! const_keccak256::keccak_f1600(&mut data);
//! assert_eq!(data, [
//!     0x2D5C954DF96ECB3C, 0x6A332CD07057B56D, 0x093D8D1270D76B6C, 0x8A20D9B25569D094,
//!     0x4F9C4F99E5E7F156, 0xF957B9A2DA65FB38, 0x85773DAE1275AF0D, 0xFAF4F247C3D810F7,
//!     0x1F1B9EE6F79A8759, 0xE4FECC0FEE98B425, 0x68CE61B6B9CE68A1, 0xDEEA66C4BA8F974F,
//!     0x33C43D836EAFB1F5, 0xE00654042719DBD9, 0x7CF8A9F009831265, 0xFD5449A6BF174743,
//!     0x97DDAD33D8994B40, 0x48EAD5FC5D0BE774, 0xE3B8C8EE55B7B03C, 0x91A0226E649E42E9,
//!     0x900E3129E7BADD7B, 0x202A9EC5FAA3CCE8, 0x5B3402464E1C3DB6, 0x609F4E62A44C1059,
//!     0x20D06CD26A8FBF5C,
//! ]);
//! ```
//!
//! [1]: https://docs.rs/sha3
//! [2]: https://docs.rs/tiny-keccak

#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![allow(non_upper_case_globals)]
#![warn(
    clippy::mod_module_files,
    clippy::unwrap_used,
    missing_docs,
    rust_2018_idioms,
    unused_lifetimes,
    unused_qualifications
)]

#[rustfmt::skip]
mod unroll;

const ALEN: usize = 17;
const ALEN_BYTES: usize = 136;
const PLEN: usize = 25;

const RHO: [u32; 24] = [
    1, 3, 6, 10, 15, 21, 28, 36, 45, 55, 2, 14, 27, 41, 56, 8, 25, 43, 62, 18, 39, 61, 20, 44,
];

const PI: [usize; 24] = [
    10, 7, 11, 17, 18, 3, 5, 16, 8, 21, 24, 4, 15, 23, 19, 13, 12, 2, 20, 14, 22, 9, 6, 1,
];

const RC: [u64; 24] = [
    0x0000000000000001,
    0x0000000000008082,
    0x800000000000808a,
    0x8000000080008000,
    0x000000000000808b,
    0x0000000080000001,
    0x8000000080008081,
    0x8000000000008009,
    0x000000000000008a,
    0x0000000000000088,
    0x0000000080008009,
    0x000000008000000a,
    0x000000008000808b,
    0x800000000000008b,
    0x8000000000008089,
    0x8000000000008003,
    0x8000000000008002,
    0x8000000000000080,
    0x000000000000800a,
    0x800000008000000a,
    0x8000000080008081,
    0x8000000000008080,
    0x0000000080000001,
    0x8000000080008008,
];

const KECCAK_F1600_ROUND_COUNT: usize = 24;

#[allow(unused_assignments)]
/// Generic Keccak-p1600 sponge function
pub const fn keccak_f1600(state: &mut [u64; PLEN]) {
    let mut round = 0;
    while round < KECCAK_F1600_ROUND_COUNT {
        let rc = RC[round];

        let mut array = [0u64; 5];

        // Theta
        unroll5!(x, {
            unroll5!(y, {
                array[x] ^= state[5 * y + x];
            });
        });

        unroll5!(x, {
            unroll5!(y, {
                let t1 = array[(x + 4) % 5];
                let t2 = array[(x + 1) % 5].rotate_left(1);
                state[5 * y + x] ^= t1 ^ t2;
            });
        });

        // Rho and pi
        let mut last = state[1];
        unroll24!(x, {
            array[0] = state[PI[x]];
            state[PI[x]] = last.rotate_left(RHO[x]);
            last = array[0];
        });

        // Chi
        unroll5!(y_step, {
            let y = 5 * y_step;

            unroll5!(x, {
                array[x] = state[y + x];
            });

            unroll5!(x, {
                let t1 = !array[(x + 1) % 5];
                let t2 = array[(x + 2) % 5];
                state[y + x] = array[x] ^ (t1 & t2);
            });
        });

        // Iota
        state[0] ^= rc;

        round += 1;
    }
}

#[derive(Clone, Copy)]
struct Keccak256Buffer {
    buffer: [u64; ALEN],
    filled: usize,
}

#[derive(Clone, Copy)]
struct Keccak256State {
    words: [u64; PLEN],
}

/// Keccak256 hasher
#[derive(Clone, Copy)]
pub struct Keccak256 {
    state: Keccak256State,
    buffer: Keccak256Buffer,
}

impl Keccak256Buffer {
    const unsafe fn append(&mut self, input: &[u8]) {
        #[cfg(target_endian = "big")]
        compile_error!("BE archs are not supported");

        debug_assert!(input.len() <= ALEN_BYTES - self.filled);
        core::hint::assert_unchecked(self.filled < ALEN_BYTES - 1);
        core::hint::assert_unchecked(ALEN_BYTES - self.filled >= input.len());

        let (src, len) = (input.as_ptr(), input.len());
        let dst = self.buffer.as_mut_ptr().cast::<u8>();
        core::ptr::copy_nonoverlapping(src, dst, len);
        self.filled += len;
    }

    const unsafe fn pad(&mut self) {
        #[cfg(target_endian = "big")]
        compile_error!("BE archs are not supported");

        core::hint::assert_unchecked(self.filled < ALEN_BYTES);

        let dst = self.buffer.as_mut_ptr().cast::<u8>().add(self.filled);
        core::ptr::write(dst, 0x01);

        let padding = ALEN_BYTES - self.filled;
        if padding != 0 {
            let dst = dst.add(1);
            core::ptr::write_bytes(dst, 0u8, padding);
        }

        self.buffer[ALEN - 1] ^= 0x80000000_00000000;
    }
}

impl Keccak256 {
    /// New hasher
    pub const fn new() -> Self {
        #[cfg(target_endian = "big")]
        compile_error!("BE archs are not supported");

        Self {
            buffer: Keccak256Buffer {
                buffer: [0u64; ALEN],
                filled: 0,
            },
            state: Keccak256State {
                words: [0u64; PLEN],
            },
        }
    }

    const fn absorb_from_buffer(&mut self) {
        #[cfg(target_endian = "big")]
        compile_error!("BE archs are not supported");

        let mut word = 0;
        while word < ALEN {
            self.state.words[word] ^= self.buffer.buffer[word];
            word += 1;
        }
        self.buffer.filled = 0;
    }

    /// Hash bytes
    pub const fn update(&mut self, mut input: &[u8]) {
        #[cfg(target_endian = "big")]
        compile_error!("BE archs are not supported");

        while input.len() >= ALEN_BYTES - self.buffer.filled {
            let to_take = ALEN_BYTES - self.buffer.filled;
            let (to_absorb, rest) = unsafe { input.split_at_unchecked(to_take) };
            input = rest;
            unsafe {
                self.buffer.append(to_absorb);
            }
            debug_assert!(self.buffer.filled == ALEN_BYTES);
            // actually absorb from buffer
            self.absorb_from_buffer();
            keccak_f1600(&mut self.state.words);
        }

        // put rest into the buffer
        unsafe {
            self.buffer.append(input);
            debug_assert!(self.buffer.filled < ALEN_BYTES);
        }
    }

    /// Output a hash
    pub const fn finalize(mut self) -> [u8; 32] {
        #[cfg(target_endian = "big")]
        compile_error!("BE archs are not supported");

        unsafe {
            self.buffer.pad();
        }
        self.absorb_from_buffer();
        keccak_f1600(&mut self.state.words);

        let result = [
            self.state.words[0],
            self.state.words[1],
            self.state.words[2],
            self.state.words[3],
        ];

        unsafe { core::mem::transmute(result) }
    }
}

/// Compute Keccak256 digest for byte string
pub const fn keccak256_digest(input: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    hasher.update(&input);
    hasher.finalize()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_trivial() {
        for len in [0, 32, 135, 136, 200, 271, 272] {
            let mut hasher = Keccak256::new();
            let input = vec![0u8; len];
            hasher.update(&input);
            let _output = hasher.finalize();
        }
    }
}
