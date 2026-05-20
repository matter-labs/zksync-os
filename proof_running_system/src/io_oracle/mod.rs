use zk_ee::{
    oracle::{word_layout::WordLayout, IOOracle},
    system::errors::internal::InternalError,
};

pub trait WordSource: 'static {
    fn read_word(&mut self) -> u32;
}

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
}
