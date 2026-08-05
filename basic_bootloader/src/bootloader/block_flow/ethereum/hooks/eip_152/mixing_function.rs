#[inline(always)]
pub fn rotate_right<const AMT: u32>(value: u64) -> u64 {
    value.rotate_right(AMT)
}

pub(crate) const BLAKE2S_BLOCK_SIZE_U64_WORDS: usize = 16;
pub(crate) const BLAKE2S_STATE_WIDTH_IN_U64_WORDS: usize = 8;
pub(crate) const BLAKE2S_EXTENDED_STATE_WIDTH_IN_U64_WORDS: usize = 16;

pub const SIGMAS: [[usize; 16]; 10] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
    [11, 8, 12, 0, 5, 2, 15, 13, 10, 14, 3, 6, 7, 1, 9, 4],
    [7, 9, 3, 1, 13, 12, 11, 14, 2, 6, 5, 10, 4, 0, 15, 8],
    [9, 0, 5, 7, 2, 4, 10, 15, 14, 1, 11, 12, 6, 8, 3, 13],
    [2, 12, 6, 10, 0, 11, 8, 3, 4, 13, 7, 5, 15, 14, 1, 9],
    [12, 5, 1, 15, 14, 13, 4, 10, 0, 7, 6, 3, 9, 2, 8, 11],
    [13, 11, 7, 14, 12, 1, 3, 9, 5, 0, 15, 4, 8, 6, 2, 10],
    [6, 15, 14, 9, 11, 3, 0, 8, 12, 2, 13, 7, 1, 4, 10, 5],
    [10, 2, 8, 4, 7, 6, 1, 5, 15, 11, 9, 14, 3, 12, 13, 0],
];

pub const BLAKE2B_IV: [u64; 8] = [
    0x6a09e667f3bcc908,
    0xbb67ae8584caa73b,
    0x3c6ef372fe94f82b,
    0xa54ff53a5f1d36f1,
    0x510e527fade682d1,
    0x9b05688c2b3e6c1f,
    0x1f83d9abfb41bd6b,
    0x5be0cd19137e2179,
];

#[inline(always)]
pub(crate) fn g_function(
    v: &mut [u64; BLAKE2S_BLOCK_SIZE_U64_WORDS],
    a: usize,
    b: usize,
    c: usize,
    d: usize,
    x: u64,
    y: u64,
) {
    v[a] = v[a].wrapping_add(v[b]).wrapping_add(x);
    v[d] = rotate_right::<32>(v[d] ^ v[a]);
    v[c] = v[c].wrapping_add(v[d]);
    v[b] = rotate_right::<24>(v[b] ^ v[c]);
    v[a] = v[a].wrapping_add(v[b]).wrapping_add(y);
    v[d] = rotate_right::<16>(v[d] ^ v[a]);
    v[c] = v[c].wrapping_add(v[d]);
    v[b] = rotate_right::<63>(v[b] ^ v[c]);
}

#[inline(always)]
pub fn mixing_function(
    state: &mut [u64; BLAKE2S_EXTENDED_STATE_WIDTH_IN_U64_WORDS],
    message_block: &[u64; BLAKE2S_BLOCK_SIZE_U64_WORDS],
    sigma: &[usize; 16],
) {
    // mix rows and columns
    unsafe {
        g_function(
            state,
            0,
            4,
            8,
            12,
            *message_block.get_unchecked(sigma[0]),
            *message_block.get_unchecked(sigma[1]),
        );
        g_function(
            state,
            1,
            5,
            9,
            13,
            *message_block.get_unchecked(sigma[2]),
            *message_block.get_unchecked(sigma[3]),
        );
        g_function(
            state,
            2,
            6,
            10,
            14,
            *message_block.get_unchecked(sigma[4]),
            *message_block.get_unchecked(sigma[5]),
        );
        g_function(
            state,
            3,
            7,
            11,
            15,
            *message_block.get_unchecked(sigma[6]),
            *message_block.get_unchecked(sigma[7]),
        );

        g_function(
            state,
            0,
            5,
            10,
            15,
            *message_block.get_unchecked(sigma[8]),
            *message_block.get_unchecked(sigma[9]),
        );
        g_function(
            state,
            1,
            6,
            11,
            12,
            *message_block.get_unchecked(sigma[10]),
            *message_block.get_unchecked(sigma[11]),
        );
        g_function(
            state,
            2,
            7,
            8,
            13,
            *message_block.get_unchecked(sigma[12]),
            *message_block.get_unchecked(sigma[13]),
        );
        g_function(
            state,
            3,
            4,
            9,
            14,
            *message_block.get_unchecked(sigma[14]),
            *message_block.get_unchecked(sigma[15]),
        );
    }
}

#[inline(always)]
pub(crate) fn round_function_for_num_rounds(
    extended_state: &mut [u64; BLAKE2S_EXTENDED_STATE_WIDTH_IN_U64_WORDS],
    message_block: &[u64; BLAKE2S_BLOCK_SIZE_U64_WORDS],
    num_rounds: usize,
) {
    // full rounds
    for i in 0..num_rounds {
        let sigma = &SIGMAS[i % 10];
        mixing_function(extended_state, message_block, sigma);
    }
}
