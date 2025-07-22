use crate::kv_markers::*;
use crate::system::errors::internal::InternalError;
use crate::types_config::EthereumIOTypesConfig;
use crate::utils::*;

use super::state_root_view::StateRootView;

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "testing", derive(serde::Serialize, serde::Deserialize))]
pub struct BasicIOImplementerFSM<SR: StateRootView<EthereumIOTypesConfig>> {
    pub state_root_view: SR,
    pub pubdata_diffs_log_hash: Bytes32,
    pub num_pubdata_diffs_logs: u32,
    pub block_functionality_is_completed: bool,
}

impl<SR: StateRootView<EthereumIOTypesConfig>> UsizeSerializable for BasicIOImplementerFSM<SR> {
    const USIZE_LEN: usize = <SR as UsizeSerializable>::USIZE_LEN
        + <Bytes32 as UsizeSerializable>::USIZE_LEN
        + <u32 as UsizeSerializable>::USIZE_LEN
        + <bool as UsizeSerializable>::USIZE_LEN;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        ExactSizeChain::new(
            UsizeSerializable::iter(&self.state_root_view),
            ExactSizeChain::new(
                UsizeSerializable::iter(&self.pubdata_diffs_log_hash),
                ExactSizeChain::new(
                    UsizeSerializable::iter(&self.num_pubdata_diffs_logs),
                    UsizeSerializable::iter(&(self.block_functionality_is_completed)),
                ),
            ),
        )
    }
}

impl<SR: StateRootView<EthereumIOTypesConfig>> UsizeDeserializable for BasicIOImplementerFSM<SR> {
    const USIZE_LEN: usize = <Self as UsizeSerializable>::USIZE_LEN;
    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let state_root_view = UsizeDeserializable::from_iter(src)?;
        let pubdata_diffs_log_hash = UsizeDeserializable::from_iter(src)?;
        let num_pubdata_diffs_logs = UsizeDeserializable::from_iter(src)?;
        let block_functionality_is_completed = UsizeDeserializable::from_iter(src)?;

        let new = Self {
            state_root_view,
            pubdata_diffs_log_hash,
            num_pubdata_diffs_logs,
            block_functionality_is_completed,
        };

        Ok(new)
    }
}
