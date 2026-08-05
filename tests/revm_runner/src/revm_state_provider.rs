use alloy::primitives::{Address, Bytes, B256, KECCAK256_EMPTY};
use basic_system::system_implementation::flat_storage_model::{
    address_into_special_storage_key, AccountProperties, ACCOUNT_PROPERTIES_STORAGE_ADDRESS,
};
use forward_system::run::convert_alloy::{FromAlloy, IntoAlloy};
use revm::{
    database_interface::DBErrorMarker,
    primitives::{StorageKey, StorageValue},
    state::{AccountInfo, Bytecode},
    DatabaseRef,
};
use std::{
    fmt,
    sync::{Arc, Mutex, MutexGuard},
};
use zk_ee::{common_structs::derive_flat_storage_key, utils::Bytes32};
use zksync_os_interface::{
    traits::{PreimageSource, ReadStorage},
    types::BlockHashes,
};

#[derive(Debug)]
pub enum RevmStateProviderError {
    StateViewPoisoned,
    MissingAccountPreimage {
        address: Address,
        hash: B256,
    },
    MalformedAccountPreimage {
        address: Address,
        hash: B256,
        expected_len: usize,
        actual_len: usize,
    },
    MissingCodePreimage {
        code_hash: B256,
    },
    MalformedCodePreimage {
        code_hash: B256,
        reason: String,
    },
}

impl fmt::Display for RevmStateProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StateViewPoisoned => write!(f, "state view lock is poisoned"),
            Self::MissingAccountPreimage { address, hash } => {
                write!(
                    f,
                    "missing account preimage for address {address:?} and hash {hash:?}"
                )
            }
            Self::MalformedAccountPreimage {
                address,
                hash,
                expected_len,
                actual_len,
            } => {
                write!(
                    f,
                    "malformed account preimage for address {address:?} and hash {hash:?}: expected {expected_len} bytes, got {actual_len}"
                )
            }
            Self::MissingCodePreimage { code_hash } => {
                write!(f, "missing bytecode preimage for hash {code_hash:?}")
            }
            Self::MalformedCodePreimage { code_hash, reason } => {
                write!(
                    f,
                    "malformed bytecode preimage for hash {code_hash:?}: {reason}"
                )
            }
        }
    }
}

impl std::error::Error for RevmStateProviderError {}

impl DBErrorMarker for RevmStateProviderError {}

/// Read-only view on a state from a specific block.
pub trait ViewState: ReadStorage + PreimageSource + Send + Clone {
    fn get_account(
        &mut self,
        address: Address,
    ) -> Result<Option<AccountProperties>, RevmStateProviderError> {
        let key = derive_flat_storage_key(
            &ACCOUNT_PROPERTIES_STORAGE_ADDRESS,
            &address_into_special_storage_key(&FromAlloy::from_alloy(address)),
        );
        let hash = self.read(key.into_alloy()).unwrap_or_default();
        if hash == B256::ZERO {
            return Ok(None);
        }

        let preimage = self
            .get_preimage(hash)
            .ok_or(RevmStateProviderError::MissingAccountPreimage { address, hash })?;
        let actual_len = preimage.len();
        let encoded: [u8; AccountProperties::ENCODED_SIZE] =
            preimage
                .try_into()
                .map_err(|_| RevmStateProviderError::MalformedAccountPreimage {
                    address,
                    hash,
                    expected_len: AccountProperties::ENCODED_SIZE,
                    actual_len,
                })?;

        Ok(Some(AccountProperties::decode(&encoded)))
    }
}

#[derive(Clone)]
pub struct RevmStateProvider<State>
where
    State: ViewState,
{
    state_view: Arc<Mutex<State>>,
    block_hashes: BlockHashes,
    state_block_number: u64,
}

impl<State> RevmStateProvider<State>
where
    State: ViewState,
{
    pub fn new(state_view: State, block_hashes: BlockHashes, state_block_number: u64) -> Self {
        Self {
            state_view: Arc::new(Mutex::new(state_view)),
            block_hashes,
            state_block_number,
        }
    }

    fn state_view(&self) -> Result<MutexGuard<'_, State>, RevmStateProviderError> {
        self.state_view
            .lock()
            .map_err(|_| RevmStateProviderError::StateViewPoisoned)
    }
}

impl<State> fmt::Debug for RevmStateProvider<State>
where
    State: ViewState,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RevmStateProvider")
            .field("block_hashes", &self.block_hashes)
            .field("state_block_number", &self.state_block_number)
            .finish()
    }
}

impl<State> DatabaseRef for RevmStateProvider<State>
where
    State: ViewState,
{
    /// The database error type.
    type Error = RevmStateProviderError;

    /// Gets basic account information.
    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        let props = {
            let mut state_view = self.state_view()?;
            state_view.get_account(address)?
        };

        props
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
                    let unpadded =
                        zksync_os_api::helpers::get_unpadded_code(bytecode.bytes_slice(), &props);
                    Some(Bytecode::new_legacy(Bytes::copy_from_slice(unpadded)))
                };

                Ok(AccountInfo {
                    nonce: props.nonce,
                    balance: props.balance,
                    code_hash: internal_code_hash,
                    account_id: None,
                    code,
                })
            })
            .transpose()
    }

    /// Gets account code by its hash.
    fn code_by_hash_ref(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        if code_hash == B256::ZERO || code_hash == KECCAK256_EMPTY {
            return Ok(Bytecode::default());
        }

        let mut state_view = self.state_view()?;
        let bytes = state_view
            .get_preimage(code_hash)
            .ok_or(RevmStateProviderError::MissingCodePreimage { code_hash })?;

        Bytecode::new_raw_checked(bytes.into()).map_err(|err| {
            RevmStateProviderError::MalformedCodePreimage {
                code_hash,
                reason: err.to_string(),
            }
        })
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
        let mut state_view = self.state_view()?;
        Ok(state_view
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
