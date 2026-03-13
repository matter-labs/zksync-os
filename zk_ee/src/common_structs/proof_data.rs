use crate::oracle::usize_serialization::{WordDeserializable, WordSerializable, WordSink};
use crate::system::errors::internal::InternalError;
use crate::types_config::EthereumIOTypesConfig;

use super::state_root_view::StateRootView;

///
/// During proof run we need extra data to validate provided inputs against chain state commitment before the block.
///
/// We'll validate reads/apply writes against `state_root_view` and validate that block timestamp is greater than `last_block_timestamp`.
/// At the end we'll calculate chain state commitment before using this fields and other metadata values(block number, hashes) used during execution.
///
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProofData<SR: StateRootView<EthereumIOTypesConfig>> {
    pub state_root_view: SR,
    pub last_block_timestamp: u64,
}

impl<SR: StateRootView<EthereumIOTypesConfig>> WordSerializable for ProofData<SR> {
    fn word_len(&self) -> usize {
        self.state_root_view.word_len() + self.last_block_timestamp.word_len()
    }

    fn write_words(&self, out: &mut impl WordSink) {
        self.state_root_view.write_words(out);
        self.last_block_timestamp.write_words(out);
    }
}

impl<SR: StateRootView<EthereumIOTypesConfig>> WordDeserializable for ProofData<SR> {
    fn read_words(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let state_root_view = WordDeserializable::read_words(src)?;
        let last_block_timestamp = WordDeserializable::read_words(src)?;
        let new = Self {
            state_root_view,
            last_block_timestamp,
        };

        Ok(new)
    }
}
