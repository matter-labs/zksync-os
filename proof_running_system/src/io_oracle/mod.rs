use zk_ee::{
    oracle::query_ids::FRI_PROOF_QUERY_ID,
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
        if query_type == FRI_PROOF_QUERY_ID {
            // FRI proof sidecar responses are count-prefixed:
            //   [oracle_stream_len, word_0, word_1, ..., word_N-1].
            //
            // The host-side CSR bridge transports each host `usize` as two
            // 32-bit reads. Consume the outer response length and the
            // count-prefix pair here. The remaining packed proof words are then
            // read directly by the Airbender verifier as low/high CSR halves.
            let response_len = NDS::csr_read_impl();
            assert!(response_len != 0);
            let oracle_stream_len = NDS::csr_read_impl();
            let oracle_stream_len_high = NDS::csr_read_impl();
            assert!(oracle_stream_len_high == 0);
            assert!(2 * (1 + oracle_stream_len.div_ceil(2)) == response_len);
            return Ok(CsrBasedIOOracleIterator::<NDS> {
                remaining: 0,
                _marker: core::marker::PhantomData,
            });
        }

        // We can expect that length of the result is returned via read.
        let remaining_len = NDS::csr_read_impl();
        let it = CsrBasedIOOracleIterator::<NDS> {
            remaining: remaining_len,
            _marker: core::marker::PhantomData,
        };

        Ok(it)
    }
}
