use alloc::vec::Vec;
use core::cell::UnsafeCell;
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

struct PreparedFriResponseState(UnsafeCell<Option<(Vec<usize>, usize)>>);

unsafe impl Sync for PreparedFriResponseState {}

impl PreparedFriResponseState {
    const fn new() -> Self {
        Self(UnsafeCell::new(None))
    }

    unsafe fn slot(&self) -> &mut Option<(Vec<usize>, usize)> {
        &mut *self.0.get()
    }
}

static PREPARED_FRI_RESPONSE: PreparedFriResponseState = PreparedFriResponseState::new();

pub fn set_prepared_fri_response(response: Vec<usize>) {
    unsafe {
        *PREPARED_FRI_RESPONSE.slot() = Some((response, 0));
    }
}

pub fn try_read_prepared_fri_word() -> Option<usize> {
    unsafe {
        let slot = PREPARED_FRI_RESPONSE.slot();
        let (buffer, position) = slot.as_mut()?;
        if *position >= buffer.len() {
            *slot = None;
            return None;
        }

        let word = buffer[*position];
        *position += 1;
        if *position >= buffer.len() {
            *slot = None;
        }

        Some(word)
    }
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
    type RawIterator<'a> = alloc::vec::IntoIter<usize>;

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
        // we can expect that length of the result is returned via read
        let remaining_len = NDS::csr_read_impl();
        let mut response = Vec::with_capacity(remaining_len);
        for _ in 0..remaining_len {
            response.push(NDS::csr_read_impl());
        }

        if query_type == FRI_PROOF_QUERY_ID {
            // The forward responder returns a count-prefixed stream:
            //   [oracle_stream_len, word_0, word_1, ..., word_N-1]
            // Strip the length prefix before storing so the verifier sees
            // only the payload words, matching the host-path behaviour.
            let payload = if response.is_empty() {
                Vec::new()
            } else {
                response[1..].to_vec()
            };
            set_prepared_fri_response(payload);
        }

        Ok(response.into_iter())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepared_fri_response_reads_in_order_and_clears() {
        set_prepared_fri_response(vec![11, 22]);
        assert_eq!(try_read_prepared_fri_word(), Some(11));
        assert_eq!(try_read_prepared_fri_word(), Some(22));
        assert_eq!(try_read_prepared_fri_word(), None);
    }

    #[test]
    fn empty_prepared_fri_response_is_cleared_immediately() {
        set_prepared_fri_response(Vec::new());
        assert_eq!(try_read_prepared_fri_word(), None);
        assert_eq!(try_read_prepared_fri_word(), None);
    }

    /// Verify that a count-prefixed FRI response [2, 0xaa, 0xbb] stores only
    /// [0xaa, 0xbb] in the prepared buffer — the length word must be stripped.
    ///
    /// `raw_query` has a const assert that `usize == u32` (RISC-V only), so
    /// this test exercises the stripping logic via `set_prepared_fri_response`
    /// directly, mirroring what `raw_query` does after the CSR round-trip.
    #[test]
    fn fri_prepared_buffer_strips_count_prefix() {
        // Simulate the full oracle response as received from the forward
        // responder: [count=2, payload_0=0xaa, payload_1=0xbb].
        let full_oracle_response: Vec<usize> = vec![2, 0xaa, 0xbb];

        // Replicate the stripping logic from raw_query.
        let payload = if full_oracle_response.is_empty() {
            Vec::new()
        } else {
            full_oracle_response[1..].to_vec()
        };
        set_prepared_fri_response(payload);

        assert_eq!(try_read_prepared_fri_word(), Some(0xaa));
        assert_eq!(try_read_prepared_fri_word(), Some(0xbb));
        assert_eq!(try_read_prepared_fri_word(), None);
    }
}
