//! Byte-order helpers for the RISC-V proving target.
//!
//! Airbender executes the Zbb `rev8` instruction (reverse the bytes of a word) in one
//! cycle, but no other Zbb instruction, so `+zbb` cannot be enabled for the whole target
//! and LLVM keeps lowering `swap_bytes` to an 11-instruction sequence. These helpers emit
//! `rev8` explicitly. `read_be_into_words` and `write_words_as_be_32` use aligned
//! words fully inside the object, shifted together with byte and halfword edges,
//! for unaligned pointers. They never access bytes outside the object. Only the
//! limb helpers retain a plain byte/chunk branch for unaligned pointers. These
//! helpers compile on every target, but callers use them only on riscv32.

/// Reverses the bytes of one word. On riscv32 this uses a scoped Zbb `rev8`.
#[inline(always)]
pub fn bswap32(value: u32) -> u32 {
    #[cfg(target_arch = "riscv32")]
    {
        let result: u32;
        // SAFETY: `rev8` only reads the input register and writes the output register.
        unsafe {
            core::arch::asm!(
                ".option push",
                ".option arch, +zbb",
                "rev8 {rd}, {rs1}",
                ".option pop",
                rd = lateout(reg) result,
                rs1 = in(reg) value,
                options(pure, nomem, nostack, preserves_flags),
            );
        }
        result
    }

    #[cfg(not(target_arch = "riscv32"))]
    {
        value.swap_bytes()
    }
}

/// Reverses `N` source bytes into the low bytes of aligned destination words.
/// The `N / 4` full words are the bytes from `src + N % 4`. They are read as words
/// when that address is 4-aligned. Otherwise the aligned words inside the source
/// are read and shifted together with the boundary bytes. The partial high word
/// is assembled from the first `N % 4` bytes and zero-padded. Every path writes
/// whole words, so a local destination is promoted per word. No path reads
/// outside `[src, src + N)`.
///
/// # Safety
/// `N` must be in `1..=32`. `src` must point to `N` readable bytes. `dst`
/// must be 4-aligned and writable for `4 * N.div_ceil(4)` bytes. Source and
/// destination must not overlap.
#[inline(always)]
pub unsafe fn read_be_into_words<const N: usize>(src: *const u8, dst: *mut u32) {
    const { assert!(N >= 1 && N <= 32) };

    unsafe {
        let partial = N % 4;
        let full_words = N / 4;
        let words_src = src.add(partial);
        let misalignment = words_src.addr() % 4;
        if misalignment == 0 {
            let src_words = words_src.cast::<u32>();
            for i in 0..full_words {
                dst.add(full_words - 1 - i)
                    .write(bswap32(src_words.add(i).read()));
            }
        } else {
            // Keeps the word test ahead of the misalignment dispatch. The hint must
            // be in this block itself (not behind the length test) to weight the branch.
            core::hint::cold_path();
            if full_words != 0 {
                read_be_words_unaligned::<N>(words_src, dst, misalignment);
            }
        }
        if partial != 0 {
            let mut value = 0u32;
            for i in 0..partial {
                value = (value << 8) | u32::from(src.add(i).read());
            }
            dst.add(full_words).write(value);
        }
    }
}

/// The unaligned path of [`read_be_into_words`]: reads the `N / 4` full words at
/// `src`, which is `misalignment` (1 to 3) bytes past a word boundary.
#[inline(always)]
unsafe fn read_be_words_unaligned<const N: usize>(
    src: *const u8,
    dst: *mut u32,
    misalignment: usize,
) {
    unsafe {
        match misalignment {
            1 => read_be_words_shifted::<N, 1>(src, dst),
            2 => read_be_words_shifted::<N, 2>(src, dst),
            // 3, the only other value of `addr % 4` here
            _ => {
                debug_assert_eq!(misalignment, 3);
                read_be_words_shifted::<N, 3>(src, dst)
            }
        }
    }
}

/// Reads the `W = N / 4` (at least one) big-endian words at `src`, which is `K`
/// bytes past a word boundary, into `W` words at `dst`. It reads only bytes in
/// `[src, src + 4 * W)`:
/// - The `W - 1` aligned words starting at `src + 4 - K` are read as words.
/// - The `4 - K` head bytes and the `K` tail bytes are read as bytes and
///   halfwords at 2-aligned addresses.
///
/// Output word `i` combines neighbouring input words with shifts. It is then
/// byte-reversed.
#[inline(always)]
unsafe fn read_be_words_shifted<const N: usize, const K: usize>(src: *const u8, dst: *mut u32) {
    if N / 4 == 0 {
        return;
    }
    const { assert!(K >= 1 && K <= 3) };

    let words = N / 4;
    debug_assert!(words != 0);
    let low_shift = 8 * K as u32;
    let high_shift = 32 - low_shift;
    unsafe {
        let inner = src.add(4 - K).cast::<u32>();
        // Source bytes [0, 4 - K) in little-endian order.
        let head = match K {
            1 => u32::from(src.read()) | (u32::from(src.add(1).cast::<u16>().read()) << 8),
            2 => u32::from(src.cast::<u16>().read()),
            _ => u32::from(src.read()),
        };
        // Source bytes [4 * W - K, 4 * W) in little-endian order, from an aligned address.
        let tail_bytes = src.add(4 * words).sub(K);
        let tail = match K {
            1 => u32::from(tail_bytes.read()),
            2 => u32::from(tail_bytes.cast::<u16>().read()),
            _ => {
                u32::from(tail_bytes.cast::<u16>().read())
                    | (u32::from(tail_bytes.add(2).read()) << 16)
            }
        };
        let mut low = head;
        for i in 0..words {
            let next = if i + 1 < words {
                inner.add(i).read()
            } else {
                tail
            };
            dst.add(words - 1 - i)
                .write(bswap32(low | (next << high_shift)));
            low = next >> low_shift;
        }
    }
}

/// Writes eight aligned source words as 32 big-endian bytes.
/// The word path is used when `dst` is 4-aligned. Otherwise the seven aligned
/// words inside `[dst, dst + 32)` are stored as shifted words, and the boundary
/// bytes use byte and halfword stores. No path writes outside `[dst, dst + 32)`.
///
/// # Safety
/// `src` must be 4-aligned and point to eight initialized words. `dst` must
/// point to 32 writable bytes. Source and destination must not overlap.
#[inline(always)]
pub unsafe fn write_words_as_be_32(src: *const u32, dst: *mut u8) {
    unsafe {
        match dst.addr() % 4 {
            0 => {
                let dst_words = dst.cast::<u32>();
                for i in 0..8 {
                    dst_words.add(i).write(bswap32(src.add(7 - i).read()));
                }
            }
            misalignment => {
                // Keeps the word test ahead of the misalignment dispatch.
                core::hint::cold_path();
                write_be_32_unaligned(src, dst, misalignment)
            }
        }
    }
}

/// The unaligned path of [`write_words_as_be_32`], where `dst` is
/// `misalignment` (1 to 3) bytes past a word boundary.
#[inline(always)]
unsafe fn write_be_32_unaligned(src: *const u32, dst: *mut u8, misalignment: usize) {
    unsafe {
        match misalignment {
            1 => write_be_32_shifted::<1>(src, dst),
            2 => write_be_32_shifted::<2>(src, dst),
            // 3, the only other value of `addr % 4` here
            _ => {
                debug_assert_eq!(misalignment, 3);
                write_be_32_shifted::<3>(src, dst)
            }
        }
    }
}

/// Writes eight aligned source words as 32 big-endian bytes at `dst`, which is
/// `K` bytes past a word boundary. Word `m` of the output is
/// `v[m] = bswap32(src[7 - m])`. The seven aligned words from `dst + 4 - K`
/// are stored as shifted pairs of `v`. The `4 - K` head bytes and `K` tail
/// bytes are stored as bytes and halfwords at 2-aligned addresses.
#[inline(always)]
unsafe fn write_be_32_shifted<const K: usize>(src: *const u32, dst: *mut u8) {
    const { assert!(K >= 1 && K <= 3) };

    let high_shift = 8 * K as u32;
    let low_shift = 32 - high_shift;
    unsafe {
        let first = bswap32(src.add(7).read());
        match K {
            1 => {
                dst.write(first as u8);
                dst.add(1).cast::<u16>().write((first >> 8) as u16);
            }
            2 => dst.cast::<u16>().write(first as u16),
            _ => dst.write(first as u8),
        }
        let inner = dst.add(4 - K).cast::<u32>();
        let mut previous = first;
        for i in 0..7 {
            let next = bswap32(src.add(6 - i).read());
            inner
                .add(i)
                .write((previous >> low_shift) | (next << high_shift));
            previous = next;
        }
        // The last `K` bytes; `dst + 32 - K` is aligned.
        let tail = previous >> low_shift;
        let tail_bytes = dst.add(32 - K);
        match K {
            1 => tail_bytes.write(tail as u8),
            2 => tail_bytes.cast::<u16>().write(tail as u16),
            _ => {
                tail_bytes.cast::<u16>().write(tail as u16);
                tail_bytes.add(2).write((tail >> 16) as u8);
            }
        }
    }
}

/// Returns eight aligned source words as 32 big-endian bytes, by value and without
/// branches, so that a local destination can stay in registers.
///
/// # Safety
/// `src` must be 4-aligned and point to eight initialized words.
#[inline(always)]
pub unsafe fn words_to_be_bytes_32(src: *const u32) -> [u8; 32] {
    let words: [u32; 8] = core::array::from_fn(|i| bswap32(unsafe { src.add(7 - i).read() }));
    // SAFETY: `[u32; 8]` and `[u8; 32]` have the same size, and every bit pattern is valid.
    unsafe { core::mem::transmute::<[u32; 8], [u8; 32]>(words) }
}

/// Reads a big-endian integer into little-endian `u64` limbs.
/// The word path is used when `bytes` is 4-aligned; otherwise it reads byte chunks.
#[inline(always)]
pub fn be_bytes_to_le_limbs(bytes: &[u8; 32]) -> [u64; 4] {
    if bytes.as_ptr().addr().is_multiple_of(4) {
        let words = bytes.as_ptr().cast::<u32>();
        core::array::from_fn(|i| {
            let c = 3 - i;
            // SAFETY: the source is 4-aligned and contains eight readable words.
            let hi = bswap32(unsafe { words.add(2 * c).read() });
            let lo = bswap32(unsafe { words.add(2 * c + 1).read() });
            (u64::from(hi) << 32) | u64::from(lo)
        })
    } else {
        let chunks = bytes.as_chunks::<8>().0;
        core::array::from_fn(|i| u64::from_be_bytes(chunks[3 - i]))
    }
}

/// Writes little-endian `u64` limbs as a big-endian integer.
/// The word path is used when `out` is 4-aligned; otherwise it writes byte chunks.
#[inline(always)]
pub fn le_limbs_to_be_bytes(limbs: &[u64; 4], out: &mut [u8; 32]) {
    if out.as_ptr().addr().is_multiple_of(4) {
        let words = out.as_mut_ptr().cast::<u32>();
        for c in 0..4 {
            let limb = limbs[3 - c];
            // SAFETY: the destination is 4-aligned and contains eight writable words.
            unsafe {
                words.add(2 * c).write(bswap32((limb >> 32) as u32));
                words.add(2 * c + 1).write(bswap32(limb as u32));
            }
        }
    } else {
        for c in 0..4 {
            out[8 * c..8 * c + 8].copy_from_slice(&limbs[3 - c].to_be_bytes());
        }
    }
}

/// Reverses all bytes of `W` words in place by swapping and reversing words.
///
/// # Safety
/// `p` must be 4-aligned and point to `W` initialized, writable words.
#[inline(always)]
pub unsafe fn bytereverse_words_in_place<const W: usize>(p: *mut u32) {
    unsafe {
        for i in 0..W / 2 {
            let left = p.add(i).read();
            let right = p.add(W - 1 - i).read();
            p.add(i).write(bswap32(right));
            p.add(W - 1 - i).write(bswap32(left));
        }
        if !W.is_multiple_of(2) {
            let middle = p.add(W / 2);
            middle.write(bswap32(middle.read()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! for_lengths {
        ($check:ident) => {
            $check::<1>();
            $check::<2>();
            $check::<3>();
            $check::<4>();
            $check::<5>();
            $check::<7>();
            $check::<8>();
            $check::<9>();
            $check::<12>();
            $check::<16>();
            $check::<17>();
            $check::<20>();
            $check::<24>();
            $check::<31>();
            $check::<32>();
        };
    }

    #[test]
    fn bswap_words() {
        assert_eq!(bswap32(0x1234_5678), 0x7856_3412);
    }

    /// A 32-byte-aligned scratch buffer, so that `offset` is also the misalignment.
    #[repr(C, align(32))]
    struct Scratch([u8; 64]);

    fn payloads() -> impl Iterator<Item = [u8; 32]> {
        let mut state = 0x1234_5678u32;
        let patterned = core::array::from_fn(|i| (i * 37 + 11) as u8);
        core::iter::once(patterned)
            .chain(core::iter::once([0xFF; 32]))
            .chain((0..64).map(move |_| {
                core::array::from_fn(|_| {
                    state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    (state >> 24) as u8
                })
            }))
    }

    /// Checks `read_be_into_words::<N>` at every source misalignment against a
    /// reference byte loop. The source is passed as a pointer derived from a
    /// reference to exactly its `N` bytes, so Miri rejects any read outside them.
    /// Sentinels around the destination catch stray writes.
    fn check_read<const N: usize>() {
        let rounded = N.div_ceil(4) * 4;
        for payload in payloads() {
            for offset in 0..=3 {
                for fill in [0x00u8, 0xA5] {
                    let mut src = Scratch([0x5A; 64]);
                    src.0[offset..offset + N].copy_from_slice(&payload[..N]);
                    let source: &[u8] = &src.0[offset..offset + N];
                    assert_eq!(source.as_ptr().addr() % 4, offset);

                    let mut dst_words = [u32::from_ne_bytes([fill; 4]); 16];
                    // The helper may write only the first `rounded` bytes.
                    let window = dst_words[..rounded / 4].as_mut_ptr();
                    // SAFETY: the rounded range is within the destination window.
                    unsafe { window.cast::<u8>().add(N).write_bytes(0, rounded - N) };

                    // SAFETY: `source` has N readable bytes, the window is aligned and has
                    // room for rounded bytes, the partial word is zero, and they do not overlap.
                    unsafe { read_be_into_words::<N>(source.as_ptr(), window) };

                    let mut expected = [fill; 64];
                    expected[N..rounded].fill(0);
                    for i in 0..N {
                        expected[N - 1 - i] = payload[i];
                    }
                    // SAFETY: `[u32; 16]` and `[u8; 64]` have the same size, any bits are valid.
                    let actual = unsafe { core::mem::transmute::<[u32; 16], [u8; 64]>(dst_words) };
                    assert_eq!(
                        actual, expected,
                        "N = {N}, offset = {offset}, fill = {fill:#x}"
                    );
                    assert!(src.0[..offset].iter().all(|&b| b == 0x5A));
                    assert!(src.0[offset + N..].iter().all(|&b| b == 0x5A));
                }
            }
        }
    }

    #[test]
    fn read_be_into_words_alignment_and_sentinels() {
        for_lengths!(check_read);
    }

    #[test]
    fn read_be_into_words_word_multiples_at_all_misalignments() {
        check_read::<4>();
        check_read::<8>();
        check_read::<12>();
        check_read::<20>();
        check_read::<28>();
        check_read::<32>();
    }

    /// Checks `write_words_as_be_32` at every destination misalignment against a
    /// reference byte loop. The destination is passed as a pointer derived from a
    /// reference to exactly its 32 bytes, so Miri rejects any access outside them,
    /// and sentinels around it catch stray writes.
    #[test]
    fn write_words_as_be_32_alignment_and_sentinels() {
        for payload in payloads() {
            let mut src = [0u32; 8];
            // SAFETY: `src` is 32 writable bytes, disjoint from `payload`.
            unsafe {
                core::ptr::copy_nonoverlapping(payload.as_ptr(), src.as_mut_ptr().cast(), 32)
            };
            for offset in 0..=3 {
                let mut dst = Scratch([0xA5; 64]);
                let window: &mut [u8] = &mut dst.0[4 + offset..36 + offset];
                assert_eq!(window.as_ptr().addr() % 4, offset);
                // SAFETY: `src` is aligned and contains eight words. `window` is 32 writable
                // bytes of a distinct allocation.
                unsafe { write_words_as_be_32(src.as_ptr(), window.as_mut_ptr()) };
                let mut expected = [0xA5; 64];
                for i in 0..32 {
                    expected[4 + offset + i] = payload[31 - i];
                }
                assert_eq!(dst.0, expected, "offset = {offset}");
            }
            // SAFETY: `src` is aligned and contains eight words.
            let by_value = unsafe { words_to_be_bytes_32(src.as_ptr()) };
            let mut reversed = payload;
            reversed.reverse();
            assert_eq!(by_value, reversed);
        }
    }

    fn check_in_place<const W: usize>() {
        let mut words = core::array::from_fn::<_, W, _>(|i| {
            u32::from_ne_bytes(core::array::from_fn(|j| (i * 4 + j * 37 + 11) as u8))
        });
        let original = words;
        // SAFETY: words is W aligned, initialized, writable u32 values.
        unsafe { bytereverse_words_in_place::<W>(words.as_mut_ptr()) };
        let result = words.as_ptr().cast::<u8>();
        let source = original.as_ptr().cast::<u8>();
        for i in 0..W * 4 {
            // SAFETY: both byte indices are within their arrays.
            assert_eq!(unsafe { result.add(i).read() }, unsafe {
                source.add(W * 4 - 1 - i).read()
            });
        }
    }

    #[test]
    fn bytereverse_words_in_place_even_and_odd() {
        check_in_place::<1>();
        check_in_place::<2>();
        check_in_place::<7>();
        check_in_place::<8>();
    }

    #[test]
    fn limb_helpers_alignment_and_sentinels() {
        #[repr(align(8))]
        struct Scratch([u8; 48]);

        let mut state = 0x1234_5678u32;
        for _ in 0..256 {
            let mut src = Scratch([0xA5; 48]);
            let mut payload = [0u8; 32];
            for byte in &mut payload {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                *byte = (state >> 24) as u8;
            }
            let reference = core::array::from_fn::<_, 4, _>(|i| {
                u64::from_be_bytes(payload[(3 - i) * 8..(4 - i) * 8].try_into().unwrap())
            });
            for input_offset in 0..=3 {
                src.0.fill(0xA5);
                src.0[4 + input_offset..36 + input_offset].copy_from_slice(&payload);
                let input: &[u8; 32] = src.0[4 + input_offset..36 + input_offset]
                    .try_into()
                    .unwrap();
                let limbs = be_bytes_to_le_limbs(input);
                assert_eq!(limbs, reference);

                for output_offset in 0..=3 {
                    let mut dst = Scratch([0xA5; 48]);
                    let output: &mut [u8; 32] = dst
                        .0
                        .get_mut(4 + output_offset..36 + output_offset)
                        .unwrap()
                        .try_into()
                        .unwrap();
                    le_limbs_to_be_bytes(&limbs, output);
                    let mut expected = [0xA5; 48];
                    for (chunk, limb) in expected[4 + output_offset..36 + output_offset]
                        .chunks_exact_mut(8)
                        .zip(reference.iter().rev())
                    {
                        chunk.copy_from_slice(&limb.to_be_bytes());
                    }
                    assert_eq!(dst.0, expected);
                    assert!(src.0[..4 + input_offset].iter().all(|&byte| byte == 0xA5));
                    assert!(src.0[36 + input_offset..].iter().all(|&byte| byte == 0xA5));
                }
            }
        }
    }

    #[test]
    fn limb_helpers_are_inverse_at_all_offsets() {
        #[repr(align(8))]
        struct Scratch([u8; 36]);

        let limbs = [
            0x0123_4567_89AB_CDEF,
            0xFEDC_BA98_7654_3210,
            0xA5A5_5A5A_1357_2468,
            0xDEAD_BEEF_CAFE_BABE,
        ];
        for input_offset in 0..=3 {
            let mut input = Scratch([0xA5; 36]);
            let input_bytes: &mut [u8; 32] = input
                .0
                .get_mut(input_offset..input_offset + 32)
                .unwrap()
                .try_into()
                .unwrap();
            le_limbs_to_be_bytes(&limbs, input_bytes);
            for output_offset in 0..=3 {
                let mut output = Scratch([0xA5; 36]);
                let output_bytes: &mut [u8; 32] = output
                    .0
                    .get_mut(output_offset..output_offset + 32)
                    .unwrap()
                    .try_into()
                    .unwrap();
                le_limbs_to_be_bytes(&be_bytes_to_le_limbs(input_bytes), output_bytes);
                assert_eq!(output_bytes, input_bytes);
                assert_eq!(be_bytes_to_le_limbs(output_bytes), limbs);
                assert!(output.0[..output_offset].iter().all(|&b| b == 0xA5));
                assert!(output.0[output_offset + 32..].iter().all(|&b| b == 0xA5));
            }
            assert!(input.0[..input_offset].iter().all(|&b| b == 0xA5));
            assert!(input.0[input_offset + 32..].iter().all(|&b| b == 0xA5));
        }
    }
}
