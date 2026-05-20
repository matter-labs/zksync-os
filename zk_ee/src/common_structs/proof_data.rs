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

impl<SR: StateRootView<EthereumIOTypesConfig> + crate::oracle::word_layout::WordLayout>
    crate::oracle::word_layout::WordLayout for ProofData<SR>
{
    const WORD_COUNT: Option<usize> = match (
        <SR as crate::oracle::word_layout::WordLayout>::WORD_COUNT,
        <u64 as crate::oracle::word_layout::WordLayout>::WORD_COUNT,
    ) {
        (Some(a), Some(b)) => Some(a + b),
        _ => None,
    };

    fn write_words(&self, w: &mut impl FnMut(u32)) {
        self.state_root_view.write_words(w);
        self.last_block_timestamp.write_words(w);
    }

    fn read_words(r: &mut impl FnMut() -> u32) -> Self {
        Self {
            state_root_view: crate::oracle::word_layout::WordLayout::read_words(r),
            last_block_timestamp: crate::oracle::word_layout::WordLayout::read_words(r),
        }
    }
}
