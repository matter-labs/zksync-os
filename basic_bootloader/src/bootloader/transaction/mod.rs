//! Transaction facade for the bootloader.
//!
//! This module provides a single `Transaction<A>` enum that wraps either an
//! Ethereum-style RLP encoded transactions or an ABI-encoded ZKsync transaction.
//! It exposes a uniform API for parsing, introspection, hashing,
//! and pre-execution warming, so the rest of the bootloader does not need to care about
//! the concrete format.
//!

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

pub mod ethereum_tx_format;
pub mod zksync_transaction;
use self::zksync_transaction::ZKsyncTransaction;

#[cfg(feature = "pectra")]
pub mod authorization_list;

/// Unified transaction wrapper over Ethereum and ZKsync formats.
pub enum Transaction<A: Allocator> {
    /// RLP-encoded Ethereum transactions.
    Ethereum(EthereumTransaction<A>),
    /// ABI-encoded ZKsync transaction.
    ZKsync(ZKsyncTransaction<A>),
}

impl<A: Allocator> Transaction<A> {
    /// Parse a transaction from a raw buffer using the system IO oracle.
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

        // query the transaction encoding format from the oracle
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
                // RLP-encoded transactions don't include the `from` field, so we need to query it from the oracle.
                let from: B160 = match system.io.oracle().query_with_empty_input(TX_FROM_QUERY_ID) {
                    Ok(format) => format,
                    Err(e) => {
                        return Err(e.into());
                    }
                };
                let tx = EthereumTransaction::parse_from_buffer(buffer, expected_chain_id, from)?;
                Ok(Self::Ethereum(tx))
            }
            TxEncodingFormat::ZKsync => {
                let tx = ZKsyncTransaction::try_from_buffer(buffer)
                    .map_err(|_| TxError::Validation(InvalidTransaction::InvalidEncoding))?;
                Ok(Self::ZKsync(tx))
            }
        }
    }

    /// Returns true if this transaction is an upgrade transaction.
    pub fn is_upgrade(&self) -> bool {
        match self {
            Self::Ethereum(_) => false,
            Self::ZKsync(tx) => tx.tx_type.read() == ZKsyncTransaction::<A>::UPGRADE_TX_TYPE,
        }
    }

    /// Returns true if this transaction is an L1->L2 transaction.
    pub fn is_l1_l2(&self) -> bool {
        match self {
            Self::Ethereum(_) => false,
            Self::ZKsync(tx) => tx.tx_type.read() == ZKsyncTransaction::<A>::L1_L2_TX_TYPE,
        }
    }

    /// Returns the transaction nonce as U256.
    pub fn nonce(&self) -> U256 {
        match self {
            Self::Ethereum(tx) => U256::from(tx.nonce()),
            Self::ZKsync(tx) => tx.nonce.read(),
        }
    }

    /// Returns the gas limit.
    pub fn gas_limit(&self) -> u64 {
        match self {
            Self::Ethereum(tx) => tx.gas_limit(),
            Self::ZKsync(tx) => tx.gas_limit.read(),
        }
    }

    /// Returns the max fee per gas reference.
    pub fn max_fee_per_gas(&self) -> &U256 {
        match self {
            Self::Ethereum(tx) => tx.max_fee_per_gas(),
            Self::ZKsync(tx) => &tx.max_fee_per_gas.read_ref(),
        }
    }

    /// Returns the optional max priority fee per gas reference.
    pub fn max_priority_fee_per_gas(&self) -> Option<&U256> {
        match self {
            Self::Ethereum(tx) => tx.max_priority_fee_per_gas(),
            Self::ZKsync(tx) => Some(&tx.max_priority_fee_per_gas.read_ref()),
        }
    }

    /// Returns the gas per pubdata limit.
    pub fn gas_per_pubdata_limit(&self) -> U256 {
        match self {
            Self::Ethereum(_) => U256::ZERO,
            Self::ZKsync(tx) => U256::from(tx.gas_per_pubdata_limit.read()),
        }
    }

    /// Returns calldata bytes.
    pub fn calldata(&self) -> &[u8] {
        match self {
            Self::Ethereum(tx) => tx.calldata(),
            Self::ZKsync(tx) => tx.calldata(),
        }
    }

    /// Returns the value field reference.
    pub fn value(&self) -> &U256 {
        match self {
            Self::Ethereum(tx) => tx.value(),
            Self::ZKsync(tx) => &tx.value.read_ref(),
        }
    }

    /// Returns the sender address reference.
    pub fn from(&self) -> &B160 {
        match self {
            Self::Ethereum(tx) => tx.from(),
            Self::ZKsync(tx) => &tx.from.read_ref(),
        }
    }

    /// Computes the transaction hash used for indexing or inclusion.
    pub fn transaction_hash<R: Resources>(
        &mut self,
        chain_id: u64,
        resources: &mut R,
    ) -> Result<Bytes32, TxError> {
        match self {
            Self::Ethereum(tx) => tx.transaction_hash(resources),
            Self::ZKsync(tx) => tx
                .calculate_hash(chain_id, resources)
                .map(Bytes32::from_array),
        }
    }

    /// Returns the signing hash for signature verification.
    pub fn signed_hash<R: Resources>(&mut self, chain_id: u64) -> Result<Bytes32, TxError> {
        // Caller should charge native for this hash
        let mut inf_resources = R::FORMAL_INFINITE;
        match self {
            Self::Ethereum(tx) => Ok(*tx.hash_for_signature_verification()),
            Self::ZKsync(tx) => tx
                .calculate_signed_hash(chain_id, &mut inf_resources)
                .map(Bytes32::from_array),
        }
    }

    /// Returns the minimum balance required to accept the transaction.
    pub fn required_balance(&self) -> Option<U256> {
        match self {
            Self::Ethereum(tx) => tx.required_balance(),
            Self::ZKsync(tx) => tx.required_balance(),
        }
    }

    /// Returns the signature as `(y_parity, r, s)` borrowed from the underlying tx.
    pub fn sig_parity_r_s<'a>(&'a self) -> (bool, &'a [u8], &'a [u8]) {
        match self {
            Self::Ethereum(tx) => tx.sig_parity_r_s(),
            Self::ZKsync(tx) => tx.sig_parity_r_s(),
        }
    }

    /// Returns the destination address if present, or None for contract creation.
    pub fn to(&self) -> Option<B160> {
        match self {
            Self::Ethereum(tx) => tx.destination(),
            Self::ZKsync(tx) => Some(tx.to.read()),
        }
    }

    /// Returns Some(EVM) if this is a deployment, otherwise None.
    pub fn is_deployment(&self) -> Option<ExecutionEnvironmentType> {
        match self {
            Self::Ethereum(tx) => {
                if tx.destination().is_none() {
                    Some(ExecutionEnvironmentType::EVM)
                } else {
                    None
                }
            }
            Self::ZKsync(tx) => {
                // Checked in the structure validation that `to` is null
                if !tx.reserved[1].read().is_zero() {
                    Some(ExecutionEnvironmentType::EVM)
                } else {
                    None
                }
            }
        }
    }

    pub fn access_list_iter<'a>(
        &'a self,
    ) -> Option<impl Iterator<Item = AccessListForAddress<'a>> + Clone> {
        match self {
            Self::Ethereum(tx) => tx.access_list_iter(),
            Self::ZKsync(_) => None,
        }
    }

    /// Returns the authorization list if present.
    #[cfg(feature = "pectra")]
    pub fn authorization_list(&self) -> Option<AuthorizationList<'_>> {
        match self {
            Self::ZKsync(_) => None,
            Self::Ethereum(tx) => tx.authorization_list(),
        }
    }

    /// Returns the encoded byte length of the transaction.
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        match self {
            Self::ZKsync(tx) => tx.len(),
            Self::Ethereum(tx) => tx.len(),
        }
    }
}

/// Charge native resources for a Keccak-256 over `len` bytes.
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
