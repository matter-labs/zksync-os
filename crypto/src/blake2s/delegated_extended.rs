use blake2s_u32::state_with_extended_control::*;

#[derive(Clone, Debug)]
pub struct Blake2s256 {
    state: Blake2RoundFunctionEvaluator,
    buffer_filled_bytes: usize,
}

pub unsafe fn initialize_blake2s_delegation_context() {}

use core::convert::AsRef;

use blake2s_u32::{BLAKE2S_BLOCK_SIZE_BYTES, BLAKE2S_BLOCK_SIZE_U32_WORDS};

impl Blake2s256 {
    #[inline(always)]
    fn new_impl() -> Self {
        let new = Self {
            state: Blake2RoundFunctionEvaluator::new(),
            buffer_filled_bytes: 0,
        };

        new
    }

    #[inline(always)]
    fn digest_impl(mut self, data: impl AsRef<[u8]>) -> [u8; 32] {
        self.update_impl(data);
        self.finalize_impl()
    }

    #[inline(always)]
    fn update_impl(&mut self, data: impl AsRef<[u8]>) {
        let mut data = data.as_ref();
        while data.len() > 0 {
            unsafe {
                let to_process = core::cmp::min(
                    data.len(),
                    BLAKE2S_BLOCK_SIZE_BYTES - self.buffer_filled_bytes,
                );
                let (src, rest) = data.split_at_unchecked(to_process);
                data = rest;
                spec_memcopy(&mut self.state.input_buffer, self.buffer_filled_bytes, src);
                self.buffer_filled_bytes += to_process;
            }

            if data.len() > 0 {
                debug_assert_eq!(self.buffer_filled_bytes, BLAKE2S_BLOCK_SIZE_BYTES);
                // run round function
                unsafe {
                    self.state
                        .run_round_function_with_byte_len::<false>(BLAKE2S_BLOCK_SIZE_BYTES, false);
                }
                self.buffer_filled_bytes = 0;
            }
        }
    }

    #[inline(always)]
    fn finalize_impl(mut self) -> [u8; 32] {
        // pad the buffer with 0s and run round function again,
        // then copy the output

        // NOTE: there is no branching here, as we would not otherwise process empty inputs

        unsafe {
            // write zeroes
            let start = self
                .state
                .input_buffer
                .as_mut_ptr()
                .cast::<u8>()
                .add(self.buffer_filled_bytes);
            let end = self.state.input_buffer.as_mut_ptr_range().end.cast::<u8>();
            core::hint::assert_unchecked(start <= end);
            core::ptr::write_bytes(start, 0, end.offset_from_unsigned(start));
            // and run round function
            self.state
                .run_round_function_with_byte_len::<false>(self.buffer_filled_bytes, true);

            core::mem::transmute_copy::<_, [u8; 32]>(self.state.read_state_for_output_ref())
        }
    }

    #[inline(always)]
    fn finalize_reset_impl(&mut self) -> [u8; 32] {
        // pad the buffer with 0s and run round function again,
        // then copy the output

        // NOTE: there is no branching here, as we would not otherwise process empty inputs

        unsafe {
            // write zeroes
            let start = self
                .state
                .input_buffer
                .as_mut_ptr()
                .cast::<u8>()
                .add(self.buffer_filled_bytes);
            let end = self.state.input_buffer.as_mut_ptr_range().end.cast::<u8>();
            core::hint::assert_unchecked(start <= end);
            core::ptr::write_bytes(start, 0, end.offset_from_unsigned(start));
            // and run round function
            self.state
                .run_round_function_with_byte_len::<false>(self.buffer_filled_bytes, true);

            let to_return =
                core::mem::transmute_copy::<_, [u8; 32]>(self.state.read_state_for_output_ref());

            self.state.reset();
            self.buffer_filled_bytes = 0;

            to_return
        }
    }
}

impl crate::MiniDigest for Blake2s256 {
    type HashOutput = [u8; 32];

    #[inline(always)]
    fn new() -> Self {
        Blake2s256::new_impl()
    }

    #[inline(always)]
    fn digest(input: impl AsRef<[u8]>) -> Self::HashOutput {
        let hasher = Self::new_impl();
        hasher.digest_impl(input)
    }

    #[inline(always)]
    fn update(&mut self, input: impl AsRef<[u8]>) {
        self.update_impl(input);
    }

    #[inline(always)]
    fn finalize(self) -> Self::HashOutput {
        self.finalize_impl()
    }

    #[inline(always)]
    fn finalize_reset(&mut self) -> Self::HashOutput {
        self.finalize_reset_impl()
    }
}

unsafe fn spec_memcopy(
    dst: &mut [u32; BLAKE2S_BLOCK_SIZE_U32_WORDS],
    dst_byte_offset: usize,
    src: &[u8],
) {
    if src.len() == 0 {
        return;
    }
    core::hint::assert_unchecked(src.len() <= 64);
    let dst = core::slice::from_raw_parts_mut(
        dst.as_mut_ptr().cast::<u8>().add(dst_byte_offset),
        src.len(),
    );
    core::hint::assert_unchecked(src.len() == dst.len());
    dst.copy_from_slice(src);
}
