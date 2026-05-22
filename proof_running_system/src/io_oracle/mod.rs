use zk_ee::{
    oracle::word_serialization::{WordDeserializable, WordSerializable},
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

impl<NDS: NonDeterminismCSRSourceImplementation> IOOracle for CsrBasedIOOracle<NDS> {
    type RawIterator<'a> = CsrBasedIOOracleIterator<NDS>;

    fn raw_query<'a, I: WordSerializable + WordDeserializable>(
        &'a mut self,
        query_type: u32,
        input: &I,
    ) -> Result<Self::RawIterator<'a>, InternalError> {
        const {
            assert!(core::mem::size_of::<usize>() == core::mem::size_of::<u32>());
        }
        NDS::csr_write_impl(query_type as usize);
        // write length
        let iterator_len = input.word_len();
        NDS::csr_write_impl(iterator_len);
        // write content
        let mut remaining_len = iterator_len;
        struct CsrWordSink<'a, I: NonDeterminismCSRSourceImplementation> {
            remaining_len: &'a mut usize,
            _marker: core::marker::PhantomData<I>,
        }

        impl<I: NonDeterminismCSRSourceImplementation> zk_ee::oracle::word_serialization::WordSink
            for CsrWordSink<'_, I>
        {
            fn write_word(&mut self, word: usize) {
                assert!(*self.remaining_len != 0);
                I::csr_write_impl(word);
                *self.remaining_len -= 1;
            }
        }

        let mut sink = CsrWordSink::<NDS> {
            remaining_len: &mut remaining_len,
            _marker: core::marker::PhantomData,
        };
        input.write_words(&mut sink);
        assert!(remaining_len == 0);
        // we can expect that length of the result is returned via read
        let remaining_len = NDS::csr_read_impl();
        let it = CsrBasedIOOracleIterator::<NDS> {
            remaining: remaining_len,
            _marker: core::marker::PhantomData,
        };

        Ok(it)
    }
}
