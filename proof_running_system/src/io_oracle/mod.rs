use airbender_guest::transport::Transport;
use zk_ee::{
    oracle::{word_layout::WordLayout, IOOracle},
    system::errors::internal::InternalError,
};

pub struct ProvingOracle<T: Transport> {
    transport: T,
}

impl<T: Transport> ProvingOracle<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }
}

impl<T: Transport + 'static> IOOracle for ProvingOracle<T> {
    #[inline(always)]
    fn query<I: WordLayout, O: WordLayout>(
        &mut self,
        _query_type: u32,
        _input: &I,
    ) -> Result<O, InternalError> {
        Ok(O::read_words(&mut || self.transport.read_word()))
    }
}
