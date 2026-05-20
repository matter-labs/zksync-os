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
}
