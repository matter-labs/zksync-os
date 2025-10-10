use super::errors::TxError;
use crate::bootloader::BootloaderSubsystemError;
use crate::bootloader::InvalidTransaction;
use core::alloc::Allocator;
use ethereum_tx_format::AccessListForAddress;
#[cfg(feature = "pectra")]
use ethereum_tx_format::AuthorizationList;
use ethereum_tx_format::EthereumTransaction;
use ruint::aliases::B160;
use ruint::aliases::U256;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::internal_error;
use zk_ee::oracle::query_ids::{TX_ENCODING_FORMAT_QUERY_ID, TX_FROM_QUERY_ID};
use zk_ee::oracle::TxEncodingFormat;
use zk_ee::system::errors::runtime::RuntimeError;
use zk_ee::system::errors::system::SystemError;
use zk_ee::system::EthereumLikeTypes;
use zk_ee::system::IOSubsystemExt;
use zk_ee::system::Resources;
use zk_ee::system::System;
use zk_ee::utils::Bytes32;
use zk_ee::utils::UsizeAlignedByteBox;

mod ethereum_tx_format;
pub mod zk_transaction;
use self::zk_transaction::ZkSyncTransaction;

#[cfg(feature = "pectra")]
pub mod authorization_list;

pub enum Transaction<A: Allocator> {
    Ethereum(EthereumTransaction<A>),
    Zk(ZkSyncTransaction<A>),
}

impl<A: Allocator> Transaction<A> {
    pub fn try_from_buffer<
        S: EthereumLikeTypes<
            Metadata = zk_ee::system::metadata::zk_metadata::ZkMetadata,
            Allocator = A,
        >,
    >(
        buffer: UsizeAlignedByteBox<A>,
        system: &mut System<S>,
    ) -> Result<Self, TxError>
    where
        S::IO: IOSubsystemExt,
    {
        use zk_ee::oracle::IOOracle;
        let expected_chain_id = system.get_chain_id();

        let format: TxEncodingFormat = match system
            .io
            .oracle()
            .query_with_empty_input(TX_ENCODING_FORMAT_QUERY_ID)
        {
            Ok(format) => format,
            Err(e) => {
                return Err(e.into());
            }
        };

        match format {
            TxEncodingFormat::Eth => {
                let from: B160 = match system.io.oracle().query_with_empty_input(TX_FROM_QUERY_ID) {
                    Ok(format) => format,
                    Err(e) => {
                        return Err(e.into());
                    }
                };
                let tx = EthereumTransaction::parse_from_buffer(buffer, expected_chain_id, from)?;
                Ok(Self::Ethereum(tx))
            }
            TxEncodingFormat::Zk => {
                let tx = ZkSyncTransaction::try_from_buffer(buffer)
                    .map_err(|_| TxError::Validation(InvalidTransaction::InvalidEncoding))?;
                Ok(Self::Zk(tx))
            }
        }
    }

    pub fn is_upgrade(&self) -> bool {
        match self {
            Self::Ethereum(_) => false,
            Self::Zk(tx) => tx.tx_type.read() == ZkSyncTransaction::<A>::UPGRADE_TX_TYPE,
        }
    }

    pub fn is_l1_l2(&self) -> bool {
        match self {
            Self::Ethereum(_) => false,
            Self::Zk(tx) => tx.tx_type.read() == ZkSyncTransaction::<A>::L1_L2_TX_TYPE,
        }
    }

    pub fn nonce(&self) -> U256 {
        match self {
            Self::Ethereum(tx) => U256::from(tx.nonce()),
            Self::Zk(tx) => tx.nonce.read(),
        }
    }

    pub fn gas_limit(&self) -> u64 {
        match self {
            Self::Ethereum(tx) => tx.gas_limit(),
            Self::Zk(tx) => tx.gas_limit.read(),
        }
    }

    pub fn max_fee_per_gas(&self) -> &U256 {
        match self {
            Self::Ethereum(tx) => tx.max_fee_per_gas(),
            Self::Zk(tx) => &tx.max_fee_per_gas.read_ref(),
        }
    }

    pub fn max_priority_fee_per_gas(&self) -> Option<&U256> {
        match self {
            Self::Ethereum(tx) => tx.max_priority_fee_per_gas(),
            Self::Zk(tx) => Some(&tx.max_priority_fee_per_gas.read_ref()),
        }
    }

    pub fn gas_per_pubdata_limit(&self) -> U256 {
        match self {
            Self::Ethereum(_) => U256::ZERO,
            Self::Zk(tx) => U256::from(tx.gas_per_pubdata_limit.read()),
        }
    }

    pub fn calldata(&self) -> &[u8] {
        match self {
            Self::Ethereum(tx) => tx.calldata(),
            Self::Zk(tx) => tx.calldata(),
        }
    }

    pub fn value(&self) -> &U256 {
        match self {
            Self::Ethereum(tx) => tx.value(),
            Self::Zk(tx) => &tx.value.read_ref(),
        }
    }

    pub fn from(&self) -> &B160 {
        match self {
            Self::Ethereum(tx) => tx.from(),
            Self::Zk(tx) => &tx.from.read_ref(),
        }
    }

    pub fn transaction_hash<R: Resources>(
        &mut self,
        chain_id: u64,
        resources: &mut R,
    ) -> Result<Bytes32, TxError> {
        match self {
            Self::Ethereum(tx) => tx.transaction_hash(resources),
            Self::Zk(tx) => tx
                .calculate_hash(chain_id, resources)
                .map(Bytes32::from_array),
        }
    }

    pub fn signed_hash<R: Resources>(&mut self, chain_id: u64) -> Result<Bytes32, TxError> {
        // Caller should charge native for this hash
        let mut inf_resources = R::FORMAL_INFINITE;
        match self {
            Self::Ethereum(tx) => Ok(*tx.hash_for_signature_verification()),
            Self::Zk(tx) => tx
                .calculate_signed_hash(chain_id, &mut inf_resources)
                .map(Bytes32::from_array),
        }
    }

    pub fn required_balance(&self) -> Option<U256> {
        match self {
            Self::Ethereum(tx) => tx.required_balance(),
            Self::Zk(tx) => tx.required_balance(),
        }
    }

    pub fn sig_parity_r_s<'a>(&'a self) -> (bool, &'a [u8], &'a [u8]) {
        match self {
            Self::Ethereum(tx) => tx.sig_parity_r_s(),
            Self::Zk(tx) => tx.sig_parity_r_s(),
        }
    }

    pub fn to(&self) -> Option<B160> {
        match self {
            Self::Ethereum(tx) => tx.destination(),
            Self::Zk(tx) => Some(tx.to.read()),
        }
    }

    ///
    /// Returns Some(to_ee_type) if the transaction is a deployment
    ///
    pub fn is_deployment(&self) -> Option<ExecutionEnvironmentType> {
        match self {
            Self::Ethereum(tx) => {
                if tx.destination().is_none() {
                    Some(ExecutionEnvironmentType::EVM)
                } else {
                    None
                }
            }
            Self::Zk(tx) => {
                // Checked in the structure validation that `to` is null
                if !tx.reserved[1].read().is_zero() {
                    Some(ExecutionEnvironmentType::EVM)
                } else {
                    None
                }
            }
        }
    }

    ///
    /// Validate access list while warming up accounts and
    /// storage slots.
    /// TODO: move somewhere else, as in V2
    ///
    pub fn parse_and_warm_up_access_list<
        S: EthereumLikeTypes<
            Metadata = zk_ee::system::metadata::zk_metadata::ZkMetadata,
            Allocator = A,
        >,
    >(
        &self,
        system: &mut System<S>,
        resources: &mut S::Resources,
    ) -> Result<(), TxError>
    where
        S::IO: IOSubsystemExt,
    {
        match self {
            Self::Zk(_) => Ok(()),
            Self::Ethereum(tx) => {
                if let Some(iter) = tx.access_list_iter() {
                    for AccessListForAddress {
                        address,
                        slots_list,
                    } in iter
                    {
                        system.io.touch_account(
                            ExecutionEnvironmentType::NoEE,
                            resources,
                            &address,
                            true,
                        )?;
                        for key in slots_list.iter() {
                            let key = key?;
                            system.io.storage_touch(
                                ExecutionEnvironmentType::NoEE,
                                resources,
                                &address,
                                &Bytes32::from_array(*key),
                                true,
                            )?;
                        }
                    }
                }
                Ok(())
            }
        }
    }

    #[cfg(feature = "pectra")]
    pub fn authorization_list(&self) -> Option<AuthorizationList<'_>> {
        match self {
            Self::Zk(_) => None,
            Self::Ethereum(tx) => tx.authorization_list(),
        }
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        match self {
            Self::Zk(tx) => tx.len(),
            Self::Ethereum(tx) => tx.len(),
        }
    }
}

pub fn charge_keccak<R: Resources>(len: usize, resources: &mut R) -> Result<(), TxError> {
    let native_cost = basic_system::system_functions::keccak256::keccak256_native_cost::<R>(len);
    resources
        .charge(&R::from_native(native_cost))
        .map_err(|e| match e {
            SystemError::LeafRuntime(RuntimeError::OutOfErgs(_)) => {
                internal_error!("Charging for keccak is not supposed to consume ergs").into()
            }
            SystemError::LeafDefect(e) => BootloaderSubsystemError::LeafDefect(e),
            SystemError::LeafRuntime(RuntimeError::FatalRuntimeError(e)) => {
                BootloaderSubsystemError::LeafRuntime(RuntimeError::FatalRuntimeError(e))
            }
        })
        .map_err(TxError::oon_as_validation)
}
