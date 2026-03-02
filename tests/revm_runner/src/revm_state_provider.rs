use crate::{
    convert_alloy::{FromAlloy, IntoAlloy},
    helpers::get_unpadded_code,
};
use alloy::primitives::{Address, B256, KECCAK256_EMPTY};
use basic_system::system_implementation::flat_storage_model::{
    address_into_special_storage_key, AccountProperties, ACCOUNT_PROPERTIES_STORAGE_ADDRESS,
};
use reth_revm::{
    db::DBErrorMarker,
    primitives::{StorageKey, StorageValue},
    state::{AccountInfo, Bytecode},
    DatabaseRef,
};
use std::fmt;
use zk_ee::{common_structs::derive_flat_storage_key, utils::Bytes32};
use zksync_os_interface::{
    traits::{PreimageSource, ReadStorage},
    types::BlockHashes,
};

/// Read-only view on a state from a specific block.
pub trait ViewState: ReadStorage + PreimageSource + Send + Clone {
    fn get_account(&mut self, address: Address) -> Option<AccountProperties> {
        let key = derive_flat_storage_key(
            &ACCOUNT_PROPERTIES_STORAGE_ADDRESS,
            &address_into_special_storage_key(&FromAlloy::from_alloy(address)),
        );
        let hash = self.read(key.into_alloy());
        if let Some(hash) = hash {
            if hash != B256::ZERO {
                Some(AccountProperties::decode(
                    &self.get_preimage(hash).unwrap().try_into().unwrap(),
                ))
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Get account's nonce by its address.
    ///
    /// Returns `None` if the account doesn't exist
    fn account_nonce(&mut self, address: Address) -> Option<u64> {
        self.get_account(address).map(|a| a.nonce)
    }
}

#[derive(Debug, Clone)]
pub struct RevmStateProvider<State>
where
    State: ViewState,
{
    state_view: State,
    block_hashes: BlockHashes,
    state_block_number: u64,
}

impl<State> RevmStateProvider<State>
where
    State: ViewState,
{
    pub fn new(state_view: State, block_hashes: BlockHashes, state_block_number: u64) -> Self {
        Self {
            state_view,
            block_hashes,
            state_block_number,
        }
    }
}

#[derive(Debug)]
pub struct RevmStateProviderError();

impl fmt::Display for RevmStateProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Revm state provider error")
    }
}

impl std::error::Error for RevmStateProviderError {}

impl DBErrorMarker for RevmStateProviderError {}

impl<State> DatabaseRef for RevmStateProvider<State>
where
    State: ViewState,
{
    /// The database error type.
    type Error = RevmStateProviderError;

    /// Gets basic account information.
    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        self.state_view
            .clone()
            .get_account(address)
            .map(|props| -> Result<_, Self::Error> {
                let internal_code_hash = {
                    if props.observable_bytecode_hash.is_zero() {
                        KECCAK256_EMPTY
                    } else {
                        props.observable_bytecode_hash.into_alloy()
                    }
                };

                let code = if props.bytecode_hash.is_zero() {
                    None
                } else {
                    let bytecode = self.code_by_hash_ref(props.bytecode_hash.into_alloy())?;
                    Some(get_unpadded_code(bytecode.bytes_slice(), &props))
                };

                Ok(AccountInfo {
                    nonce: props.nonce,
                    balance: props.balance,
                    code_hash: internal_code_hash,
                    code,
                })
            })
            .transpose()
    }

    /// Gets account code by its hash.
    fn code_by_hash_ref(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        Ok(self
            .state_view
            .clone()
            .get_preimage(code_hash)
            .map(|bytes| Bytecode::new_raw(bytes.into()))
            .unwrap_or_default())
    }

    /// Gets storage value of address at index.
    fn storage_ref(
        &self,
        address: Address,
        index: StorageKey,
    ) -> Result<StorageValue, Self::Error> {
        let storage_key: B256 = index.into();
        let storage_key = Bytes32::from_alloy(storage_key);
        let flat_key =
            derive_flat_storage_key(&ruint::aliases::B160::from_alloy(address), &storage_key);
        Ok(self
            .state_view
            .clone()
            .read(flat_key.into_alloy())
            .unwrap_or_default()
            .into())
    }

    /// Gets block hash by block number.
    fn block_hash_ref(&self, number: u64) -> Result<B256, Self::Error> {
        if let Some(diff) = self.state_block_number.checked_sub(number) {
            if diff < 256 {
                Ok(self.block_hashes.0[255 - diff as usize].into())
            } else {
                Ok(B256::default())
            }
        } else {
            Ok(B256::default())
        }
    }
}
