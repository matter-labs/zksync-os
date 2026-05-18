use alloy::primitives::{Address, B256};
use rig::zksync_os_interface::types::BlockContext as BlockContextInterface;
use rig::{
    forward_system::run::ReadStorage,
    utils::AccountProperties,
    zk_ee::utils::Bytes32,
    zksync_os_interface::traits::{PreimageSource, ReadStorage as InterfaceReadStorage},
    BlockContext, Chain,
};
use zksync_os_revm_runner::{
    convert_alloy::{FromAlloy, IntoAlloy},
    revm_state_provider::ViewState,
};

#[derive(Clone)]
pub struct ChainStateView {
    pub chain: Chain,
}

impl PreimageSource for ChainStateView {
    fn get_preimage(&mut self, hash: B256) -> Option<Vec<u8>> {
        let hash = Bytes32::from_alloy(hash);
        self.chain.preimage_source.inner.get(&hash).cloned()
    }
}

impl InterfaceReadStorage for ChainStateView {
    fn read(&mut self, key: B256) -> Option<B256> {
        let key = Bytes32::from_alloy(key);
        let value = self.chain.state_tree.read(key);

        value.map(|v| v.into_alloy())
    }
}

impl ViewState for ChainStateView {
    fn get_account(&mut self, address: Address) -> Option<AccountProperties> {
        let address = ruint::aliases::B160::from_alloy(address);
        self.chain.get_account_properties_maybe(&address)
    }

    fn account_nonce(&mut self, address: Address) -> Option<u64> {
        self.get_account(address).map(|account| account.nonce)
    }
}

pub fn generate_block_context_interface(
    chain: &Chain,
    rig_block_context: &BlockContext,
) -> BlockContextInterface {
    BlockContextInterface {
        block_number: chain.next_block_number(),
        timestamp: rig_block_context.timestamp,
        eip1559_basefee: rig_block_context.eip1559_basefee,
        chain_id: chain.chain_id(),
        block_hashes: rig::zksync_os_interface::types::BlockHashes(chain.block_hashes()),
        pubdata_price: rig_block_context.pubdata_price,
        native_price: rig_block_context.native_price,
        coinbase: rig_block_context.coinbase.into_alloy(),
        gas_limit: rig_block_context.gas_limit,
        pubdata_limit: rig_block_context.pubdata_limit,
        mix_hash: rig_block_context.mix_hash,
        execution_version: 0, // TODO meaningless here
        blob_fee: Default::default(),
    }
}
