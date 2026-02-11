use alloy::primitives::{Address, B256};
use basic_system::system_implementation::flat_storage_model::AccountProperties;
use forward_system::run::convert_alloy::{FromAlloy, IntoAlloy};
use forward_system::run::ReadStorage as ForwardSystemReadStorage;
use zk_ee::utils::Bytes32;
use zksync_os_interface::traits::{PreimageSource, ReadStorage};
use zksync_os_revm_runner::revm_state_provider::ViewState;

use crate::Chain;

#[derive(Clone)]
pub struct ChainStateView {
    pub chain: Chain,
}

impl PreimageSource for ChainStateView {
    fn get_preimage(&mut self, hash: B256) -> Option<Vec<u8>> {
        let hash: Bytes32 = hash.from_alloy();
        self.chain.preimage_source.inner.get(&hash).cloned()
    }
}

impl ReadStorage for ChainStateView {
    fn read(&mut self, key: B256) -> Option<B256> {
        let key: Bytes32 = key.from_alloy();
        let value = self.chain.state_tree.read(key);

        value.map(|v| v.into_alloy())
    }
}

impl ViewState for ChainStateView {
    fn get_account(&mut self, address: Address) -> Option<AccountProperties> {
        let address = address.from_alloy();
        self.chain.get_account_properties_maybe(&address)
    }

    fn account_nonce(&mut self, address: Address) -> Option<u64> {
        let account = self.get_account(address);

        account.map(|account| account.nonce)
    }
}
