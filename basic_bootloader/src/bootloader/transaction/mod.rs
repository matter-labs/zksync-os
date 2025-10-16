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
use rlp_encoded::AccessListForAddress;
#[cfg(feature = "pectra")]
use rlp_encoded::AuthorizationList;
use rlp_encoded::RlpEncodedTransaction;
use ruint::aliases::B160;
use ruint::aliases::U256;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::internal_error;
use zk_ee::oracle::query_ids::{TX_ENCODING_FORMAT_QUERY_ID, TX_FROM_QUERY_ID};
use zk_ee::oracle::simple_oracle_query::SimpleOracleQuery;
use zk_ee::oracle::usize_serialization::UsizeDeserializable;
use zk_ee::oracle::usize_serialization::UsizeSerializable;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::system::errors::runtime::RuntimeError;
use zk_ee::system::errors::system::SystemError;
use zk_ee::system::EthereumLikeTypes;
use zk_ee::system::IOSubsystemExt;
use zk_ee::system::Resources;
use zk_ee::system::System;
use zk_ee::utils::Bytes32;
use zk_ee::utils::UsizeAlignedByteBox;

pub mod abi_encoded;
pub mod rlp_encoded;
use self::abi_encoded::AbiEncodedTransaction;

#[cfg(feature = "pectra")]
pub mod authorization_list;

/// Unified transaction wrapper over RLP and ABI formats.
/// RLP transactions are used for regular Ethereum transactions,
/// while ABI transactions are used for ZKsync-specific transactions.
pub enum Transaction<A: Allocator> {
    /// RLP-encoded transactions.
    Rlp(RlpEncodedTransaction<A>),
    /// ABI-encoded ZKsync transaction.
    Abi(AbiEncodedTransaction<A>),
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
        let expected_chain_id = system.get_chain_id();

        // query the transaction encoding format from the oracle
        let format: TxEncodingFormat = TxEncodingFormatQuery::get(system.io.oracle(), &())?;

        match format {
            TxEncodingFormat::Rlp => {
                // RLP-encoded transactions don't include the `from` field, so we need to query it from the oracle.
                // This is so that sequencer can skip ecrecover (for simulation, for example).
                let from = TxFromQuery::get(system.io.oracle(), &())?;
                let tx = RlpEncodedTransaction::parse_from_buffer(buffer, expected_chain_id, from)?;
                Ok(Self::Rlp(tx))
            }
            TxEncodingFormat::Abi => {
                let tx = AbiEncodedTransaction::try_from_buffer(buffer)
                    .map_err(|_| TxError::Validation(InvalidTransaction::InvalidEncoding))?;
                Ok(Self::Abi(tx))
            }
        }
    }

    /// Returns true if this transaction is an upgrade transaction.
    pub fn is_upgrade(&self) -> bool {
        match self {
            Self::Rlp(_) => false,
            Self::Abi(tx) => tx.tx_type.read() == AbiEncodedTransaction::<A>::UPGRADE_TX_TYPE,
        }
    }

    /// Returns true if this transaction is an L1->L2 transaction.
    pub fn is_l1_l2(&self) -> bool {
        match self {
            Self::Rlp(_) => false,
            Self::Abi(tx) => tx.tx_type.read() == AbiEncodedTransaction::<A>::L1_L2_TX_TYPE,
        }
    }

    /// Returns the transaction nonce as U256.
    pub fn nonce(&self) -> U256 {
        match self {
            Self::Rlp(tx) => U256::from(tx.nonce()),
            Self::Abi(tx) => tx.nonce.read(),
        }
    }

    /// Returns the gas limit.
    pub fn gas_limit(&self) -> u64 {
        match self {
            Self::Rlp(tx) => tx.gas_limit(),
            Self::Abi(tx) => tx.gas_limit.read(),
        }
    }

    /// Returns the max fee per gas reference.
    pub fn max_fee_per_gas(&self) -> &U256 {
        match self {
            Self::Rlp(tx) => tx.max_fee_per_gas(),
            Self::Abi(tx) => &tx.max_fee_per_gas.read_ref(),
        }
    }

    /// Returns the optional max priority fee per gas reference.
    pub fn max_priority_fee_per_gas(&self) -> Option<&U256> {
        match self {
            Self::Rlp(tx) => tx.max_priority_fee_per_gas(),
            Self::Abi(tx) => Some(&tx.max_priority_fee_per_gas.read_ref()),
        }
    }

    /// Returns the gas per pubdata limit.
    pub fn gas_per_pubdata_limit(&self) -> U256 {
        match self {
            Self::Rlp(_) => U256::ZERO,
            Self::Abi(tx) => U256::from(tx.gas_per_pubdata_limit.read()),
        }
    }

    /// Returns calldata bytes.
    pub fn calldata(&self) -> &[u8] {
        match self {
            Self::Rlp(tx) => tx.calldata(),
            Self::Abi(tx) => tx.calldata(),
        }
    }

    /// Returns the value field reference.
    pub fn value(&self) -> &U256 {
        match self {
            Self::Rlp(tx) => tx.value(),
            Self::Abi(tx) => &tx.value.read_ref(),
        }
    }

    /// Returns the sender address reference.
    pub fn from(&self) -> &B160 {
        match self {
            Self::Rlp(tx) => tx.from(),
            Self::Abi(tx) => &tx.from.read_ref(),
        }
    }

    /// Computes the transaction hash used for indexing or inclusion.
    pub fn transaction_hash<R: Resources>(
        &mut self,
        chain_id: u64,
        resources: &mut R,
    ) -> Result<Bytes32, TxError> {
        match self {
            Self::Rlp(tx) => tx.transaction_hash(resources),
            Self::Abi(tx) => tx
                .calculate_hash(chain_id, resources)
                .map(Bytes32::from_array),
        }
    }

    /// Returns the signing hash for signature verification.
    pub fn signed_hash<R: Resources>(&mut self, chain_id: u64) -> Result<Bytes32, TxError> {
        // Caller should charge native for this hash
        let mut inf_resources = R::FORMAL_INFINITE;
        match self {
            Self::Rlp(tx) => Ok(*tx.hash_for_signature_verification()),
            Self::Abi(tx) => tx
                .calculate_signed_hash(chain_id, &mut inf_resources)
                .map(Bytes32::from_array),
        }
    }

    /// Returns the minimum balance required to accept the transaction.
    pub fn required_balance(&self) -> Option<U256> {
        match self {
            Self::Rlp(tx) => tx.required_balance(),
            Self::Abi(tx) => tx.required_balance(),
        }
    }

    /// Returns the signature as `(y_parity, r, s)` borrowed from the underlying tx.
    pub fn sig_parity_r_s<'a>(&'a self) -> (bool, &'a [u8], &'a [u8]) {
        match self {
            Self::Rlp(tx) => tx.sig_parity_r_s(),
            Self::Abi(tx) => tx.sig_parity_r_s(),
        }
    }

    /// Returns the destination address if present, or None for contract creation.
    pub fn to(&self) -> Option<B160> {
        match self {
            Self::Rlp(tx) => tx.destination(),
            Self::Abi(tx) => Some(tx.to.read()),
        }
    }

    /// Returns Some(EVM) if this is a deployment, otherwise None.
    pub fn is_deployment(&self) -> Option<ExecutionEnvironmentType> {
        match self {
            Self::Rlp(tx) => {
                if tx.destination().is_none() {
                    Some(ExecutionEnvironmentType::EVM)
                } else {
                    None
                }
            }
            Self::Abi(tx) => {
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
            Self::Rlp(tx) => tx.access_list_iter(),
            Self::Abi(_) => None,
        }
    }

    /// Returns the authorization list if present.
    #[cfg(feature = "pectra")]
    pub fn authorization_list(&self) -> Option<AuthorizationList<'_>> {
        match self {
            Self::Abi(_) => None,
            Self::Rlp(tx) => tx.authorization_list(),
        }
    }

    /// Returns the encoded byte length of the transaction.
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        match self {
            Self::Abi(tx) => tx.len(),
            Self::Rlp(tx) => tx.len(),
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

#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature = "testing", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum TxEncodingFormat {
    Abi = 0,
    Rlp = 1,
}

impl UsizeDeserializable for TxEncodingFormat {
    const USIZE_LEN: usize = 1;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let byte = <u8 as UsizeDeserializable>::from_iter(src)?;
        if byte == TxEncodingFormat::Abi as u8 {
            Ok(TxEncodingFormat::Abi)
        } else if byte == TxEncodingFormat::Rlp as u8 {
            Ok(TxEncodingFormat::Rlp)
        } else {
            Err(internal_error!("Unsupported tx encoding format"))
        }
    }
}

impl UsizeSerializable for TxEncodingFormat {
    const USIZE_LEN: usize = <Self as UsizeDeserializable>::USIZE_LEN;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        cfg_if::cfg_if!(
            if #[cfg(target_endian = "big")] {
                compile_error!("unsupported architecture: big endian arch is not supported")
            } else if #[cfg(target_pointer_width = "32")] {
                let low = *self as usize;
                let high = 0;
                return [low, high].into_iter();
            } else if #[cfg(target_pointer_width = "64")] {
                #[allow(clippy::needless_return)]
                return core::iter::once(*self as usize);
            } else {
                compile_error!("unsupported architecture")
            }
        );
    }
}

pub struct TxEncodingFormatQuery;

impl SimpleOracleQuery for TxEncodingFormatQuery {
    type Input = ();
    type Output = TxEncodingFormat;

    const QUERY_ID: u32 = TX_ENCODING_FORMAT_QUERY_ID;
}

pub struct TxFromQuery;

impl SimpleOracleQuery for TxFromQuery {
    type Input = ();
    type Output = B160;

    const QUERY_ID: u32 = TX_FROM_QUERY_ID;
}
