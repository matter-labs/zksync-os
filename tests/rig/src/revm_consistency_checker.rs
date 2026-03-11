use alloy::primitives::{Address, B256};
use basic_system::system_implementation::flat_storage_model::AccountProperties;
use forward_system::run::convert_alloy::{FromAlloy, IntoAlloy};
use forward_system::run::ReadStorage as ForwardSystemReadStorage;
use zk_ee::utils::Bytes32;
use zksync_os_interface::traits::{PreimageSource, ReadStorage};
use zksync_os_revm_runner::revm_state_provider::{RevmStateProviderError, ViewState};

use crate::{BlockContext, Chain};

#[derive(Clone)]
pub struct ChainStateView<const RANDOMIZED_TREE: bool = false> {
    pub chain: Chain<RANDOMIZED_TREE>,
}

impl<const RANDOMIZED_TREE: bool> PreimageSource for ChainStateView<RANDOMIZED_TREE> {
    fn get_preimage(&mut self, hash: B256) -> Option<Vec<u8>> {
        let hash = Bytes32::from_alloy(hash);
        self.chain.preimage_source.inner.get(&hash).cloned()
    }
}

impl<const RANDOMIZED_TREE: bool> ReadStorage for ChainStateView<RANDOMIZED_TREE> {
    fn read(&mut self, key: B256) -> Option<B256> {
        let key = Bytes32::from_alloy(key);
        let value = self.chain.state_tree.read(key);

        value.map(|v| v.into_alloy())
    }
}

impl<const RANDOMIZED_TREE: bool> ViewState for ChainStateView<RANDOMIZED_TREE> {
    fn get_account(
        &mut self,
        address: Address,
    ) -> Result<Option<AccountProperties>, RevmStateProviderError> {
        let address = ruint::aliases::B160::from_alloy(address);
        Ok(self.chain.get_account_properties_maybe(&address))
    }
}

use zksync_os_interface::types::BlockContext as BlockContextInterface;
pub fn generate_block_context_interface<const RANDOMIZED_TREE: bool>(
    chain: &Chain<RANDOMIZED_TREE>,
    rig_block_context: &BlockContext,
) -> BlockContextInterface {
    BlockContextInterface {
        block_number: chain.next_block_number(),
        timestamp: rig_block_context.timestamp,
        eip1559_basefee: rig_block_context.eip1559_basefee,
        chain_id: chain.chain_id(),
        block_hashes: zksync_os_interface::types::BlockHashes(chain.block_hashes()),
        pubdata_price: rig_block_context.pubdata_price,
        native_price: rig_block_context.native_price,
        coinbase: rig_block_context.coinbase.into_alloy(),
        gas_limit: rig_block_context.gas_limit,
        pubdata_limit: rig_block_context.pubdata_limit,
        mix_hash: rig_block_context.mix_hash,
        execution_version: 0, // TODO meaningless here
        blob_fee: rig_block_context.blob_fee,
    }
}
