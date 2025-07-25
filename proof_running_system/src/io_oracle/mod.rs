use zk_ee::{
    kv_markers::UsizeSerializable, system::errors::internal::InternalError, system_io_oracle::*,
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

impl<I: NonDeterminismCSRSourceImplementation> IOOracle for CsrBasedIOOracle<I> {
    type MarkerTiedIterator<'a> = CsrBasedIOOracleIterator<I>;

    fn create_oracle_access_iterator<'a, M: OracleIteratorTypeMarker>(
        &'a mut self,
        init_value: M::Params,
    ) -> Result<Self::MarkerTiedIterator<'a>, InternalError> {
        I::csr_write_impl(M::ID as usize);
        let iter_to_write = UsizeSerializable::iter(&init_value);
        // write length
        let iterator_len = iter_to_write.len();
        assert!(iterator_len == <M::Params as UsizeSerializable>::USIZE_LEN);
        I::csr_write_impl(iterator_len);
        // write content
        let mut remaining_len = iterator_len;
        for value in iter_to_write {
            assert!(iterator_len != 0);
            I::csr_write_impl(value);
            remaining_len -= 1;
        }
        assert!(remaining_len == 0);
        // we can expect that length of the result is returned via read
        let remaining_len = I::csr_read_impl();
        let it = CsrBasedIOOracleIterator::<I> {
            remaining: remaining_len,
            _marker: core::marker::PhantomData,
        };

        Ok(it)
    }
}
