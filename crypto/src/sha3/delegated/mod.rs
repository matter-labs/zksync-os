#[cfg(not(target_endian = "little"))]
compile_error!("invalid arch - only intended for LE machines");

#[cfg(all(target_arch = "riscv32", feature = "keccak_special5"))]
mod precompile;
#[cfg(all(target_arch = "riscv32", feature = "keccak_special5"))]
pub(crate) use self::precompile::keccak_f1600;

#[cfg(any(
    not(all(target_arch = "riscv32", feature = "keccak_special5")),
    feature = "testing",
))]
mod precompile_logic_simulator;
#[cfg(any(
    not(all(target_arch = "riscv32", feature = "keccak_special5")),
    feature = "testing",
))]
pub(crate) use self::precompile_logic_simulator::keccak_f1600;

use crate::MiniDigest;

use common_constants::delegation_types::keccak_special5::KECCAK_SPECIAL5_STATE_AND_SCRATCH_U64_WORDS;

// NB: repr(align(256)) ensures that the lowest u16 of the pointer can fully address
//     all the words without carry, s.t. we can very cheaply offset the ptr in-circuit
#[allow(dead_code)]
#[derive(Debug, Clone)]
#[repr(align(256))]
pub(crate) struct AlignedState([u64; KECCAK_SPECIAL5_STATE_AND_SCRATCH_U64_WORDS]);

// NOTE: Sha3 and Keccak differ only in padding, so we can make it generic for free,
// whether we will need it in practice or not. We also do not use a separate buffer for input,
// and instead XOR input directly into the state

const BUFFER_SIZE_U64_WORDS: usize = 17;
const BUFFER_SIZE_U32_WORDS: usize = BUFFER_SIZE_U64_WORDS * 2;
const BUFFER_SIZE_BYTES: usize = 17 * core::mem::size_of::<u64>();

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Keccak256Core<const SHA3: bool = false> {
    state: AlignedState,
    filled_bytes: usize,
}

#[allow(dead_code)]
pub type Keccak256 = Keccak256Core<false>;
#[allow(dead_code)]
pub type Sha3_256 = Keccak256Core<true>;

impl<const SHA3: bool> Keccak256Core<SHA3> {
    #[inline(always)]
    unsafe fn absorb_unaligned(&mut self, input: &mut &[u8]) {
        let unalignment = self.filled_bytes % core::mem::size_of::<u32>();
        if unalignment == 0 {
            return;
        }
        let to_absorb: usize =
            core::cmp::min(core::mem::size_of::<u32>() - unalignment, input.len());
        let (slice_to_absorb, rest) = input.split_at_unchecked(to_absorb);
        *input = rest;

        let mut buffer = [0u8; core::mem::size_of::<u32>()];
        let dst = buffer
            .get_unchecked_mut(unalignment..)
            .get_unchecked_mut(..to_absorb);
        core::hint::assert_unchecked(slice_to_absorb.len() == dst.len());
        dst.copy_from_slice(slice_to_absorb);

        let u32_word_idx = self.filled_bytes / core::mem::size_of::<u32>();
        let dst_word = self.state.0.as_mut_ptr().cast::<u32>().add(u32_word_idx);
        dst_word.write(dst_word.read() ^ u32::from_le_bytes(buffer));

        self.filled_bytes += to_absorb;
    }

    #[inline(always)]
    unsafe fn absorb_aligned(&mut self, input: &mut &[u8]) {
        if input.is_empty() {
            return;
        }
        debug_assert_eq!(self.filled_bytes % core::mem::size_of::<u32>(), 0);
        debug_assert_ne!(self.filled_bytes, BUFFER_SIZE_BYTES);
        debug_assert_eq!(
            (BUFFER_SIZE_BYTES - self.filled_bytes) % core::mem::size_of::<u32>(),
            0
        );

        let (u32_chunks, rest) = input.as_chunks::<4>();
        *input = rest;
        let max_words_to_absorb =
            (BUFFER_SIZE_BYTES - self.filled_bytes) / core::mem::size_of::<u32>();

        let words_to_absorb = core::cmp::min(max_words_to_absorb, u32_chunks.len());
        let u32_word_idx = self.filled_bytes / core::mem::size_of::<u32>();

        let mut dst = self.state.0.as_mut_ptr().cast::<u32>().add(u32_word_idx);

        let (fill_to_end_maybe, more) = u32_chunks.split_at_unchecked(words_to_absorb);
        let mut it = fill_to_end_maybe.iter();
        for _ in 0..words_to_absorb {
            dst.write(dst.read() ^ u32::from_le_bytes(*it.next().unwrap_unchecked()));
            dst = dst.add(1);
        }
        self.filled_bytes += words_to_absorb * core::mem::size_of::<u32>();
        if self.filled_bytes == BUFFER_SIZE_BYTES {
            self.filled_bytes = 0;
            keccak_f1600(&mut self.state);
        }

        // then as many full fills as possible
        let (full_buffer_fills, partial_fills) = more.as_chunks::<BUFFER_SIZE_U32_WORDS>();
        for src in full_buffer_fills.iter() {
            debug_assert_eq!(self.filled_bytes, 0);
            let dst = self
                .state
                .0
                .as_mut_ptr()
                .cast::<[u32; BUFFER_SIZE_U32_WORDS]>()
                .as_mut_unchecked();
            core::hint::assert_unchecked(src.len() == dst.len());
            for (src, dst) in src.iter().zip(dst.iter_mut()) {
                *dst ^= u32::from_le_bytes(*src);
            }
            keccak_f1600(&mut self.state);
        }

        // and partial fill again
        let words_to_absorb = partial_fills.len();
        if words_to_absorb > 0 {
            debug_assert_eq!(self.filled_bytes, 0);
        }
        debug_assert!(words_to_absorb < BUFFER_SIZE_U32_WORDS);
        let mut it = partial_fills.iter();
        let mut dst = self.state.0.as_mut_ptr().cast::<u32>();
        for _ in 0..words_to_absorb {
            dst.write(dst.read() ^ u32::from_le_bytes(*it.next().unwrap_unchecked()));
            dst = dst.add(1);
        }
        self.filled_bytes += words_to_absorb * core::mem::size_of::<u32>();
        // can not trigger a permutation
    }

    #[inline(always)]
    unsafe fn absorb_tail(&mut self, input: &[u8]) {
        if input.is_empty() {
            return;
        }
        debug_assert!(input.len() < core::mem::size_of::<u32>());
        debug_assert_eq!(self.filled_bytes % core::mem::size_of::<u32>(), 0);
        let to_absorb = input.len();
        let mut buffer = [0u8; core::mem::size_of::<u32>()];
        buffer.get_unchecked_mut(..to_absorb).copy_from_slice(input);
        let u32_word_idx = self.filled_bytes / core::mem::size_of::<u32>();
        let dst = self.state.0.as_mut_ptr().cast::<u32>().add(u32_word_idx);
        dst.write(dst.read() ^ u32::from_le_bytes(buffer));
        self.filled_bytes += to_absorb;
    }
}

impl<const SHA3: bool> MiniDigest for Keccak256Core<SHA3> {
    type HashOutput = [u8; 32];

    #[inline(always)]
    fn new() -> Self {
        Self {
            state: AlignedState([0; KECCAK_SPECIAL5_STATE_AND_SCRATCH_U64_WORDS]),
            filled_bytes: 0,
        }
    }

    // #[inline(always)]
    #[inline(never)]
    fn update(&mut self, input: impl AsRef<[u8]>) {
        let mut input = input.as_ref();

        if input.is_empty() {
            return;
        }

        // NOTE: reading unaligned u64/u32 to XOR bytes with the state is the same as copying it into aligned
        // buffer first and then XORing anyway, so we will do it on the fly

        unsafe {
            self.absorb_unaligned(&mut input);
            if self.filled_bytes == BUFFER_SIZE_BYTES {
                self.filled_bytes = 0;
                keccak_f1600(&mut self.state);
            }
            // absorb aligned will permut internellay if needed
            self.absorb_aligned(&mut input);

            // final absorb unaligned can not trigger permutation
            self.absorb_tail(input);

            debug_assert_ne!(self.filled_bytes, BUFFER_SIZE_BYTES);
        };
    }

    #[inline(always)]
    fn finalize(mut self) -> Self::HashOutput {
        keccak_pad::<SHA3>(&mut self.state.0, self.filled_bytes);
        keccak_f1600(&mut self.state);
        unsafe { self.state.0.as_ptr().cast::<[u8; 32]>().read() }
    }

    #[inline(always)]
    fn finalize_reset(&mut self) -> Self::HashOutput {
        keccak_pad::<SHA3>(&mut self.state.0, self.filled_bytes);
        keccak_f1600(&mut self.state);
        let output = unsafe { self.state.0.as_ptr().cast::<[u8; 32]>().read() };
        for dst in self.state.0.iter_mut() {
            *dst = 0;
        }
        self.filled_bytes = 0;

        output
    }

    #[inline(always)]
    fn digest(input: impl AsRef<[u8]>) -> Self::HashOutput {
        let mut hasher = Self::new();
        hasher.update(input);
        hasher.finalize()
    }
}

#[allow(dead_code)]
#[inline(always)]
fn keccak_pad<const SHA3: bool>(
    state: &mut [u64; KECCAK_SPECIAL5_STATE_AND_SCRATCH_U64_WORDS],
    len_filled_bytes: usize,
) {
    let pos_padding_start_u64 = len_filled_bytes / 8;
    let padding_start = {
        let len_leftover_bytes = len_filled_bytes % 8;
        (if SHA3 { 0x06 } else { 0x01 }) << (len_leftover_bytes * 8)
    };
    state[pos_padding_start_u64] ^= padding_start;
    state[16] ^= 0x80000000_00000000; // last bit is always there
}

#[cfg(any(test, feature = "sha3_tests"))]
pub mod tests {
    #[test]
    fn keccak_f1600() {
        keccak_f1600_test();
    }

    #[test]
    #[should_panic]
    fn bad_keccak_f1600() {
        bad_keccak_f1600_test();
    }

    #[test]
    fn mini_digest() {
        mini_digest_test();
    }

    #[test]
    fn hash_chain() {
        hash_chain_test();
    }

    #[allow(dead_code)]
    pub fn bad_keccak_f1600_test() {
        use super::*;
        let state_first = [
            0xF1258F7940E1DDE7,
            0x84D5CCF933C0478A,
            0xD598261EA65AA9EE,
            0xBD1547306F80494D,
            0x8B284E056253D057,
            0xFF97A42D7F8E6FD4,
            0x90FEE5A0A44647C4,
            0x8C5BDA0CD6192E76,
            0xAD30A6F71B19059C,
            0x30935AB7D08FFC64,
            0xEB5AA93F2317D635,
            0xA9A6E6260D712103,
            0x81A57C16DBCF555F,
            0x43B831CD0347C826,
            0x01F22F1A11A5569F,
            0x05E5635A21D9AE61,
            0x64BEFEF28CC970F2,
            0x613670957BC46611,
            0xB87C5A554FD00ECB,
            0x8C3EE88A1CCF32C8,
            0x940C7922AE3A2614,
            0x1841F924A2C509E4,
            0x16F53526E70465C2,
            0x75F644E97F30A13B,
            0xEAF1FF7B5CECA249,
        ];
        let state_second = [
            0x2D5C954DF96ECB3C,
            0x6A332CD07057B56D,
            0x093D8D1270D76B6C,
            0x8A20D9B25569D094,
            0x4F9C4F99E5E7F156,
            0xF957B9A2DA65FB38,
            0x85773DAE1275AF0D,
            0xFAF4F247C3D810F7,
            0x1F1B9EE6F79A8759,
            0xE4FECC0FEE98B425,
            0x68CE61B6B9CE68A1,
            0xDEEA66C4BA8F974F,
            0x33C43D836EAFB1F5,
            0xE00654042719DBD9,
            0x7CF8A9F009831265,
            0xFD5449A6BF174743,
            0x97DDAD33D8994B40,
            0x48EAD5FC5D0BE774,
            0xE3B8C8EE55B7B03C,
            0x91A0226E649E42E9,
            0x900E3129E7BADD7B,
            0x202A9EC5FAA3CCE8,
            0x5B3402464E1C3DB6,
            0x609F4E62A44C1059,
            0x1, //0x20D06CD26A8FBF5C,
        ];

        let mut state = super::AlignedState([0; KECCAK_SPECIAL5_STATE_AND_SCRATCH_U64_WORDS]);
        state.0[..25].copy_from_slice(&state_first);
        super::keccak_f1600(&mut state);
        assert!(state.0[..25] == state_second);
    }

    #[allow(dead_code)]
    pub fn keccak_f1600_test() {
        use super::*;
        let state_first = [
            0xF1258F7940E1DDE7,
            0x84D5CCF933C0478A,
            0xD598261EA65AA9EE,
            0xBD1547306F80494D,
            0x8B284E056253D057,
            0xFF97A42D7F8E6FD4,
            0x90FEE5A0A44647C4,
            0x8C5BDA0CD6192E76,
            0xAD30A6F71B19059C,
            0x30935AB7D08FFC64,
            0xEB5AA93F2317D635,
            0xA9A6E6260D712103,
            0x81A57C16DBCF555F,
            0x43B831CD0347C826,
            0x01F22F1A11A5569F,
            0x05E5635A21D9AE61,
            0x64BEFEF28CC970F2,
            0x613670957BC46611,
            0xB87C5A554FD00ECB,
            0x8C3EE88A1CCF32C8,
            0x940C7922AE3A2614,
            0x1841F924A2C509E4,
            0x16F53526E70465C2,
            0x75F644E97F30A13B,
            0xEAF1FF7B5CECA249,
        ];
        let state_second = [
            0x2D5C954DF96ECB3C,
            0x6A332CD07057B56D,
            0x093D8D1270D76B6C,
            0x8A20D9B25569D094,
            0x4F9C4F99E5E7F156,
            0xF957B9A2DA65FB38,
            0x85773DAE1275AF0D,
            0xFAF4F247C3D810F7,
            0x1F1B9EE6F79A8759,
            0xE4FECC0FEE98B425,
            0x68CE61B6B9CE68A1,
            0xDEEA66C4BA8F974F,
            0x33C43D836EAFB1F5,
            0xE00654042719DBD9,
            0x7CF8A9F009831265,
            0xFD5449A6BF174743,
            0x97DDAD33D8994B40,
            0x48EAD5FC5D0BE774,
            0xE3B8C8EE55B7B03C,
            0x91A0226E649E42E9,
            0x900E3129E7BADD7B,
            0x202A9EC5FAA3CCE8,
            0x5B3402464E1C3DB6,
            0x609F4E62A44C1059,
            0x20D06CD26A8FBF5C,
        ];

        let mut state = super::AlignedState([0; KECCAK_SPECIAL5_STATE_AND_SCRATCH_U64_WORDS]);
        state.0[..25].copy_from_slice(&state_first);
        super::keccak_f1600(&mut state);
        assert!(state.0[..25] == state_second);
    }

    #[allow(dead_code)]
    pub fn mini_digest_test() {
        use super::*;
        use ark_std::rand::Rng;
        let mut rng = ark_std::test_rng();
        let mut formal_keccak256 = <sha3::Keccak256 as sha3::Digest>::new();
        let mut formal_sha3 = <sha3::Sha3_256 as sha3::Digest>::new();
        let mut my_keccak256 = Keccak256::new();
        let mut my_sha3 = Sha3_256::new();
        let mut msg = [0; u8::MAX as usize];

        for _try in 0..1 << 10 {
            let num_chunks = rng.r#gen::<u8>();
            for _chunk in 0..num_chunks {
                let len = rng.r#gen::<u8>() as usize;
                for i in 0..len {
                    msg[i] = rng.r#gen::<u8>();
                }
                sha3::Digest::update(&mut formal_keccak256, &msg[..len]);
                sha3::Digest::update(&mut formal_sha3, &msg[..len]);
                my_keccak256.update(&msg[..len]);
                my_sha3.update(&msg[..len]);
            }
            assert!(
                &sha3::Digest::finalize_reset(&mut formal_keccak256)[..]
                    == my_keccak256.finalize_reset()
            );
            assert!(
                &sha3::Digest::finalize_reset(&mut formal_sha3)[..] == &my_sha3.finalize_reset()
            );
        }
    }

    #[allow(dead_code)]
    pub fn hash_chain_test() {
        use super::*;
        let mut state = AlignedState([0; KECCAK_SPECIAL5_STATE_AND_SCRATCH_U64_WORDS]);
        for _ in 0..2000 {
            super::keccak_f1600(&mut state);
        }
        core::hint::black_box(state);
    }
}
