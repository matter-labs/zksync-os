use alloy::primitives::{Address, B256, U256};
use basic_system::system_implementation::flat_storage_model::AccountProperties;
use forward_system::run::convert_alloy::{FromAlloy, IntoAlloy};
use forward_system::run::ReadStorage as ForwardSystemReadStorage;
use zk_ee::system::metadata::zk_metadata::BlockHashes;
use zk_ee::utils::Bytes32;
use zksync_os_interface::traits::{AnyBlockContext, PreimageSource, ReadStorage};
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

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct BlockContextInterface {
    pub chain_id: u64,
    pub block_number: u64,
    pub block_hashes: BlockHashes,
    pub timestamp: u64,
    pub eip1559_basefee: U256,
    pub pubdata_price: U256,
    pub native_price: U256,
    pub coinbase: Address,
    pub gas_limit: u64,
    pub pubdata_limit: u64,
    pub mix_hash: U256,
    pub execution_version: u32,
    pub blob_fee: U256,
}

impl AnyBlockContext for BlockContextInterface {
    fn chain_id(&self) -> u64 {
        self.chain_id
    }

    fn block_number(&self) -> u64 {
        self.block_number
    }

    fn block_hashes(&self) -> &[U256; 256] {
        &self.block_hashes.0
    }

    fn timestamp(&self) -> u64 {
        self.timestamp
    }

    fn eip1559_basefee(&self) -> U256 {
        self.eip1559_basefee
    }

    fn pubdata_price(&self) -> U256 {
        self.pubdata_price
    }

    fn native_price(&self) -> U256 {
        self.native_price
    }

    fn coinbase(&self) -> Address {
        self.coinbase
    }

    fn gas_limit(&self) -> u64 {
        self.gas_limit
    }

    fn pubdata_limit(&self) -> u64 {
        self.pubdata_limit
    }

    fn mix_hash(&self) -> U256 {
        self.mix_hash
    }

    fn blob_fee(&self) -> U256 {
        self.blob_fee
    }

    fn is_gateway(&self) -> bool {
        false
    }
}

pub fn generate_block_context_interface<const RANDOMIZED_TREE: bool>(
    chain: &Chain<RANDOMIZED_TREE>,
    rig_block_context: &BlockContext,
) -> BlockContextInterface {
    BlockContextInterface {
        block_number: chain.next_block_number(),
        timestamp: rig_block_context.timestamp,
        eip1559_basefee: rig_block_context.eip1559_basefee,
        chain_id: chain.chain_id(),
        block_hashes: BlockHashes(chain.block_hashes()),
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
