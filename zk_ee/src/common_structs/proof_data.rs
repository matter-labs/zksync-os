use crate::oracle::usize_serialization::{UsizeDeserializable, UsizeSerializable};
use crate::system::errors::internal::InternalError;
use crate::types_config::EthereumIOTypesConfig;
use crate::utils::exact_size_chain::ExactSizeChain;

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

impl<SR: StateRootView<EthereumIOTypesConfig>> UsizeSerializable for ProofData<SR> {
    const USIZE_LEN: usize =
        <SR as UsizeSerializable>::USIZE_LEN + <u64 as UsizeSerializable>::USIZE_LEN;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        ExactSizeChain::new(
            UsizeSerializable::iter(&self.state_root_view),
            UsizeSerializable::iter(&self.last_block_timestamp),
        )
    }
}

impl<SR: StateRootView<EthereumIOTypesConfig>> UsizeDeserializable for ProofData<SR> {
    const USIZE_LEN: usize = <Self as UsizeSerializable>::USIZE_LEN;
    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let state_root_view = UsizeDeserializable::from_iter(src)?;
        let last_block_timestamp = UsizeDeserializable::from_iter(src)?;
        let new = Self {
            state_root_view,
            last_block_timestamp,
        };

        Ok(new)
    }
}
