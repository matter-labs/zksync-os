//!
//! This module contains account models implementations.
//!

mod abstract_account;
mod contract;
mod eoa;

use core::ops::Deref;

use crate::bootloader::errors::TxError;
use crate::bootloader::transaction::ZkSyncTransaction;
pub use abstract_account::AA;
use errors::FatalError;
use ruint::aliases::B160;
use system_hooks::HooksStorage;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::memory::stack_trait::Stack;
use zk_ee::system::EthereumLikeTypes;
use zk_ee::system::System;
use zk_ee::system::SystemFrameSnapshot;
use zk_ee::system::*;

use zk_ee::utils::Bytes32;

use super::StackFrame;

///
/// The execution step output
///
pub enum ExecutionOutput<S: EthereumLikeTypes> {
    /// return data
    Call(OSImmutableSlice<S>),
    /// return data, deployed contract address
    Create(OSImmutableSlice<S>, B160),
}

impl<S: EthereumLikeTypes> core::fmt::Debug for ExecutionOutput<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Call(t) => f
                .debug_tuple("ExecutionOutput::Call")
                .field(&t.deref())
                .finish(),
            Self::Create(t, a) => f
                .debug_tuple("ExecutionOutput::Create")
                .field(&t.deref())
                .field(&a)
                .finish(),
        }
    }
}

///
/// The execution step result
///
pub enum ExecutionResult<S: EthereumLikeTypes> {
    /// Transaction executed successfully
    Success { output: ExecutionOutput<S> },
    /// Transaction reverted
    Revert { output: OSImmutableSlice<S> },
}

impl<S: EthereumLikeTypes> core::fmt::Debug for ExecutionResult<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Success { output } => f
                .debug_struct("ExecutionResult::Success")
                .field("output", output)
                .finish(),
            Self::Revert { output } => f
                .debug_struct("ExecutionResult::Revert")
                .field("output", &output.deref())
                .finish(),
        }
    }
}

impl<S: EthereumLikeTypes> ExecutionResult<S> {
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

pub struct TxProcessingResult<S: EthereumLikeTypes> {
    pub result: ExecutionResult<S>,
    pub tx_hash: Bytes32,
    pub is_l1_tx: bool,
    pub is_upgrade_tx: bool,
    pub gas_used: u64,
    pub gas_refunded: u64,
}

impl<S: EthereumLikeTypes> core::fmt::Debug for TxProcessingResult<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TxProcessingResult")
            .field("result", &self.result)
            .field("tx_hash", &self.tx_hash)
            .field("is_l1_tx", &self.is_l1_tx)
            .field("is_upgrade_tx", &self.is_upgrade_tx)
            .field("gas_used", &self.gas_used)
            .field("gas_refunded", &self.gas_refunded)
            .finish()
    }
}

pub trait AccountModel<S: EthereumLikeTypes>
where
    S::IO: IOSubsystemExt,
    S::Memory: MemorySubsystemExt,
{
    ///
    /// Validate transaction
    ///
    /// `callstack` expected to be empty at the beginning and at the end of this function execution.
    /// It's passed to reuse memory between transactions.
    ///
    fn validate<CS: Stack<StackFrame<S, SystemFrameSnapshot<S>>, S::Allocator>>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        callstack: &mut CS,
        tx_hash: Bytes32,
        suggested_signed_hash: Bytes32,
        transaction: &mut ZkSyncTransaction<'static>,
        caller_ee_type: ExecutionEnvironmentType,
        caller_is_code: bool,
        caller_nonce: u64,
        resources: &mut S::Resources,
    ) -> Result<(), TxError>;

    ///
    /// Execute transaction
    ///
    /// `callstack` expected to be empty at the beginning and at the end of this function execution.
    /// It's passed to reuse memory between transactions.
    ///
    fn execute<CS: Stack<StackFrame<S, SystemFrameSnapshot<S>>, S::Allocator>>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        callstack: &mut CS,
        tx_hash: Bytes32,
        suggested_signed_hash: Bytes32,
        transaction: &mut ZkSyncTransaction<'static>,
        current_tx_nonce: u64,
        resources: &mut S::Resources,
    ) -> Result<ExecutionResult<S>, FatalError>;

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
    fn pay_for_transaction<CS: Stack<StackFrame<S, SystemFrameSnapshot<S>>, S::Allocator>>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        callstack: &mut CS,
        tx_hash: Bytes32,
        suggested_signed_hash: Bytes32,
        transaction: &mut ZkSyncTransaction<'static>,
        from: B160,
        caller_ee_type: ExecutionEnvironmentType,
        resources: &mut S::Resources,
    ) -> Result<(), TxError>;

    ///
    /// Prepare for paymaster
    ///
    fn pre_paymaster<CS: Stack<StackFrame<S, SystemFrameSnapshot<S>>, S::Allocator>>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        callstack: &mut CS,
        tx_hash: Bytes32,
        suggested_signed_hash: Bytes32,
        transaction: &mut ZkSyncTransaction<'static>,
        from: B160,
        paymaster: B160,
        caller_ee_type: ExecutionEnvironmentType,
        resources: &mut S::Resources,
    ) -> Result<(), TxError>;
}
