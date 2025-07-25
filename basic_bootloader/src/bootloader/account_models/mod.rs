//!
//! This module contains account models implementations.
//!

mod abstract_account;
mod contract;
mod eoa;

use crate::bootloader::errors::TxError;
use crate::bootloader::runner::RunnerMemoryBuffers;
use crate::bootloader::transaction::ZkSyncTransaction;
pub use abstract_account::AA;
use ruint::aliases::B160;
use system_hooks::HooksStorage;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::system::EthereumLikeTypes;
use zk_ee::system::System;
use zk_ee::system::*;

use zk_ee::utils::Bytes32;

use super::errors::BootloaderSubsystemError;

/// The execution step output
#[derive(Debug)]
pub enum ExecutionOutput<'a> {
    /// return data
    Call(&'a [u8]),
    /// return data, deployed contract address
    Create(&'a [u8], B160),
}

/// The execution step result
#[derive(Debug)]
pub enum ExecutionResult<'a> {
    /// Transaction executed successfully
    Success { output: ExecutionOutput<'a> },
    /// Transaction reverted
    Revert { output: &'a [u8] },
}

impl<'a> ExecutionResult<'a> {
    pub fn reverted(self) -> Self {
        match self {
            Self::Success {
                output: ExecutionOutput::Call(r),
            }
            | Self::Success {
                output: ExecutionOutput::Create(r, _),
            } => Self::Revert { output: r },
            a => a,
        }
    }
}

#[derive(Debug)]
pub struct TxProcessingResult<'a> {
    pub result: ExecutionResult<'a>,
    pub tx_hash: Bytes32,
    pub is_l1_tx: bool,
    pub is_upgrade_tx: bool,
    pub gas_used: u64,
    pub gas_refunded: u64,
    #[cfg(feature = "report_native")]
    pub native_used: u64,
}

pub trait AccountModel<S: EthereumLikeTypes>
where
    S::IO: IOSubsystemExt,
{
    /// Validate transaction
    fn validate(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        memories: RunnerMemoryBuffers,
        tx_hash: Bytes32,
        suggested_signed_hash: Bytes32,
        transaction: &mut ZkSyncTransaction,
        caller_ee_type: ExecutionEnvironmentType,
        caller_is_code: bool,
        caller_nonce: u64,
        resources: &mut S::Resources,
    ) -> Result<(), TxError>;

    /// Execute transaction
    fn execute<'a>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        memories: RunnerMemoryBuffers<'a>,
        tx_hash: Bytes32,
        suggested_signed_hash: Bytes32,
        transaction: &mut ZkSyncTransaction,
        current_tx_nonce: u64,
        resources: &mut S::Resources,
    ) -> Result<ExecutionResult<'a>, BootloaderSubsystemError>;

    ///
    /// Charge any additional intrinsic gas.
    ///
    fn charge_additional_intrinsic_gas(
        resources: &mut S::Resources,
        transaction: &ZkSyncTransaction,
    ) -> Result<(), TxError>;

    ///
    /// Checks that the tx's nonce hasn't been used yet.
    ///
    fn check_nonce_is_not_used(account_data_nonce: u64, tx_nonce: u64) -> Result<(), TxError>;

    ///
    /// Check that the tx's nonce has been used.
    ///
    fn check_nonce_is_used_after_validation(
        system: &mut System<S>,
        caller_ee_type: ExecutionEnvironmentType,
        resources: &mut S::Resources,
        tx_nonce: u64,
        from: B160,
    ) -> Result<(), TxError>;

    ///
    /// Pay for the transaction's fees
    ///
    fn pay_for_transaction(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        memories: RunnerMemoryBuffers,
        tx_hash: Bytes32,
        suggested_signed_hash: Bytes32,
        transaction: &mut ZkSyncTransaction,
        from: B160,
        caller_ee_type: ExecutionEnvironmentType,
        resources: &mut S::Resources,
    ) -> Result<(), TxError>;

    ///
    /// Prepare for paymaster
    ///
    fn pre_paymaster(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        memories: RunnerMemoryBuffers,
        tx_hash: Bytes32,
        suggested_signed_hash: Bytes32,
        transaction: &mut ZkSyncTransaction,
        from: B160,
        paymaster: B160,
        caller_ee_type: ExecutionEnvironmentType,
        resources: &mut S::Resources,
    ) -> Result<(), TxError>;
}
