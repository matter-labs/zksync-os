use zk_ee::oracle::word_layout::{WordLayout, WordSource};
use zk_ee::oracle::IOOracle;
use zk_ee::system::errors::internal::InternalError;

pub struct ProvingOracle<S: WordSource> {
    source: S,
}

impl<S: WordSource> ProvingOracle<S> {
    pub fn new(source: S) -> Self {
        Self { source }
    }
}

impl<S: WordSource> IOOracle for ProvingOracle<S> {
    #[inline(always)]
    fn query<I: WordLayout, O: WordLayout>(
        &mut self,
        _query_type: u32,
        _input: &I,
    ) -> Result<O, InternalError> {
        Ok(O::read_words(&mut || self.source.read_word()))
    }

    #[inline(always)]
    fn query_into<I: WordLayout, O: WordLayout>(
        &mut self,
        _query_type: u32,
        _input: &I,
        output: &mut O,
    ) -> Result<(), InternalError> {
        output.read_words_into(&mut || self.source.read_word());
        Ok(())
    }

    fn query_byte_box<I: WordLayout, A: core::alloc::Allocator>(
        &mut self,
        _query_type: u32,
        _input: &I,
        allocator: A,
    ) -> Result<zk_ee::utils::UsizeAlignedByteBox<A>, InternalError> {
        use zk_ee::utils::{UsizeAlignedByteBox, USIZE_SIZE};
        // Read Vec<u8> wire format: u32 byte_len + ceil(byte_len/4) packed u32 words.
        // Write directly into UsizeAlignedByteBox's usize-aligned backing.
        let byte_len = self.source.read_word() as usize;
        let u32_word_count = byte_len.div_ceil(4);
        let mut box_buf = UsizeAlignedByteBox::preallocated_in(byte_len, allocator);

        // Read u32 words from transport and pack into usize-aligned backing.
        // On riscv32 (usize=u32): 1:1 mapping. On x86_64 (usize=u64): pack 2 per usize.
        let dst_ptr = box_buf.inner_mut_ptr();
        cfg_if::cfg_if! {
            if #[cfg(target_pointer_width = "32")] {
                for i in 0..u32_word_count {
                    unsafe {
                        (dst_ptr as *mut u32).add(i).write(self.source.read_word());
                    }
                }
            } else {
                let mut i = 0;
                while i < u32_word_count {
                    let lo = self.source.read_word() as u64;
                    let hi = if i + 1 < u32_word_count {
                        self.source.read_word() as u64
                    } else {
                        0
                    };
                    unsafe {
                        (dst_ptr as *mut u64).add(i / 2).write(lo | (hi << 32));
                    }
                    i += 2;
                }
            }
        }
        box_buf.mark_initialized(byte_len);
        Ok(box_buf)
    }
}
