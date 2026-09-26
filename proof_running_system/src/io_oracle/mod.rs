use zk_ee::{
    oracle::memory_io::{MemoryOracle, WordChannel, WordChannelOracle},
    oracle::usize_serialization::{UsizeDeserializable, UsizeSerializable},
    oracle::IOOracle,
    system::errors::internal::InternalError,
};

pub trait NonDeterminismCSRSourceImplementation: 'static + Clone + Copy + core::fmt::Debug {
    fn csr_read_impl() -> usize;
    fn csr_write_impl(value: usize);
}

#[derive(Clone, Copy, Debug)]
pub struct CsrBasedIOOracle<I: NonDeterminismCSRSourceImplementation> {
    _marker: core::marker::PhantomData<I>,
}

pub struct CsrBasedIOOracleIterator<I: NonDeterminismCSRSourceImplementation> {
    remaining: usize,
    _marker: core::marker::PhantomData<I>,
}

impl<I: NonDeterminismCSRSourceImplementation> Iterator for CsrBasedIOOracleIterator<I> {
    type Item = usize;
    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            None
        } else {
            self.remaining -= 1;
            Some(I::csr_read_impl())
        }
    }
}

impl<I: NonDeterminismCSRSourceImplementation> ExactSizeIterator for CsrBasedIOOracleIterator<I> {
    fn len(&self) -> usize {
        self.remaining
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DummyCSRImpl;

impl NonDeterminismCSRSourceImplementation for DummyCSRImpl {
    fn csr_read_impl() -> usize {
        0
    }
    fn csr_write_impl(_value: usize) {}
}
impl<I: NonDeterminismCSRSourceImplementation> CsrBasedIOOracle<I> {
    pub fn init() -> Self {
        Self {
            _marker: core::marker::PhantomData,
        }
    }
}

/// The non-determinism CSR as the word channel of the memory-based protocol.
#[derive(Clone, Copy, Debug)]
pub struct CsrWordChannel<I: NonDeterminismCSRSourceImplementation> {
    _marker: core::marker::PhantomData<I>,
}

impl<I: NonDeterminismCSRSourceImplementation> WordChannel for CsrWordChannel<I> {
    #[inline(always)]
    fn write_word(&mut self, word: usize) {
        I::csr_write_impl(word)
    }

    #[inline(always)]
    fn read_word(&mut self) -> usize {
        I::csr_read_impl()
    }
}

impl<NDS: NonDeterminismCSRSourceImplementation> CsrBasedIOOracle<NDS> {
    #[inline(always)]
    fn word_channel_oracle() -> WordChannelOracle<CsrWordChannel<NDS>> {
        WordChannelOracle::new(CsrWordChannel {
            _marker: core::marker::PhantomData,
        })
    }
}

impl<NDS: NonDeterminismCSRSourceImplementation> MemoryOracle for CsrBasedIOOracle<NDS> {
    #[inline(always)]
    fn send_query(&mut self, query_id: u32, input_word: usize) -> Result<(), InternalError> {
        Self::word_channel_oracle().send_query(query_id, input_word)
    }

    #[inline(always)]
    fn read_word(&mut self) -> Result<u32, InternalError> {
        Self::word_channel_oracle().read_word()
    }

    #[inline(always)]
    unsafe fn write_words(&mut self, dst: *mut u32, num_words: usize) -> Result<(), InternalError> {
        // SAFETY: guaranteed by the caller
        unsafe { Self::word_channel_oracle().write_words(dst, num_words) }
    }
}

impl<NDS: NonDeterminismCSRSourceImplementation> IOOracle for CsrBasedIOOracle<NDS> {
    type RawIterator<'a> = CsrBasedIOOracleIterator<NDS>;

    fn raw_query<'a, I: UsizeSerializable + UsizeDeserializable>(
        &'a mut self,
        query_type: u32,
        input: &I,
    ) -> Result<Self::RawIterator<'a>, InternalError> {
        const {
            assert!(core::mem::size_of::<usize>() == core::mem::size_of::<u32>());
        }
        NDS::csr_write_impl(query_type as usize);
        let iter_to_write = UsizeSerializable::iter(input);
        // write length
        let iterator_len = iter_to_write.len();
        assert!(iterator_len == <I as UsizeSerializable>::USIZE_LEN);
        NDS::csr_write_impl(iterator_len);
        // write content
        let mut remaining_len = iterator_len;
        for value in iter_to_write {
            assert!(remaining_len != 0);
            NDS::csr_write_impl(value);
            remaining_len -= 1;
        }
        assert!(remaining_len == 0);
        // We can expect that length of the result is returned via read.
        let remaining_len = NDS::csr_read_impl();
        let it = CsrBasedIOOracleIterator::<NDS> {
            remaining: remaining_len,
            _marker: core::marker::PhantomData,
        };

        Ok(it)
    }
}
