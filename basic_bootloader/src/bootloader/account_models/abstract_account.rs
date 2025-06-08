use core::marker::PhantomData;

use crate::bootloader::account_models::contract::Contract;
use crate::bootloader::account_models::eoa::EOA;
use crate::bootloader::account_models::AccountModel;
use crate::bootloader::account_models::{ExecutionResult, TxError};
use crate::bootloader::config::BasicBootloaderExecutionConfig;
use crate::bootloader::transaction::ZkSyncTransaction;
use crate::bootloader::Bytes32;
use crate::bootloader::StackFrame;
use ruint::aliases::B160;
use system_hooks::HooksStorage;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::memory::stack_trait::Stack;
use zk_ee::system::errors::FatalError;
use zk_ee::system::{
    EthereumLikeTypes, IOSubsystemExt, MemorySubsystemExt, System, SystemFrameSnapshot,
};

pub enum AA<S> {
    EOA(PhantomData<S>),
    Contract(PhantomData<S>),
}

impl<S: EthereumLikeTypes> AA<S>
where
    S::IO: IOSubsystemExt,
    S::Memory: MemorySubsystemExt,
{
    pub fn account_model_for_account(
        tx: &ZkSyncTransaction,
        is_contract: bool,
        aa_enabled: bool,
    ) -> Self {
        if tx.is_eip_712() && aa_enabled && is_contract {
            AA::Contract(PhantomData)
        } else {
            AA::EOA(PhantomData)
        }
    }

    pub fn charge_additional_intrinsic_gas(
        &self,
        resources: &mut S::Resources,
        transaction: &ZkSyncTransaction,
    ) -> Result<(), TxError> {
        match self {
            AA::EOA(_) => {
                <EOA as AccountModel<S>>::charge_additional_intrinsic_gas(resources, transaction)
            }
            AA::Contract(_) => <Contract as AccountModel<S>>::charge_additional_intrinsic_gas(
                resources,
                transaction,
            ),
        }
    }

    pub fn check_nonce_is_not_used(
        &self,
        account_data_nonce: u64,
        tx_nonce: u64,
    ) -> Result<(), TxError> {
        match self {
            AA::EOA(_) => {
                <EOA as AccountModel<S>>::check_nonce_is_not_used(account_data_nonce, tx_nonce)
            }
            AA::Contract(_) => {
                <Contract as AccountModel<S>>::check_nonce_is_not_used(account_data_nonce, tx_nonce)
            }
        }
    }

    #[allow(clippy::type_complexity)]
    #[allow(clippy::too_many_arguments)]
    pub fn validate<
        CS: Stack<StackFrame<S, SystemFrameSnapshot<S>>, S::Allocator>,
        Config: BasicBootloaderExecutionConfig,
    >(
        &self,
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
    ) -> Result<(), TxError> {
        match self {
            AA::EOA(_) => EOA::validate::<CS, Config>(
                system,
                system_functions,
                callstack,
                tx_hash,
                suggested_signed_hash,
                transaction,
                caller_ee_type,
                caller_is_code,
                caller_nonce,
                resources,
            ),
            AA::Contract(_) => Contract::validate::<CS, Config>(
                system,
                system_functions,
                callstack,
                tx_hash,
                suggested_signed_hash,
                transaction,
                caller_ee_type,
                caller_is_code,
                caller_nonce,
                resources,
            ),
        }
    }

    #[allow(clippy::type_complexity)]
    #[allow(clippy::too_many_arguments)]
    pub fn execute<CS: Stack<StackFrame<S, SystemFrameSnapshot<S>>, S::Allocator>>(
        &self,
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        callstack: &mut CS,
        tx_hash: Bytes32,
        suggested_signed_hash: Bytes32,
        transaction: &mut ZkSyncTransaction<'static>,
        current_tx_nonce: u64,
        resources: &mut S::Resources,
    ) -> Result<ExecutionResult<S>, FatalError> {
        match self {
            AA::EOA(_) => EOA::execute::<CS>(
                system,
                system_functions,
                callstack,
                tx_hash,
                suggested_signed_hash,
                transaction,
                current_tx_nonce,
                resources,
            ),
            AA::Contract(_) => Contract::execute::<CS>(
                system,
                system_functions,
                callstack,
                tx_hash,
                suggested_signed_hash,
                transaction,
                current_tx_nonce,
                resources,
            ),
        }
    }

    pub fn check_nonce_is_used_after_validation(
        &self,
        system: &mut System<S>,
        caller_ee_type: ExecutionEnvironmentType,
        resources: &mut S::Resources,
        tx_nonce: u64,
        from: B160,
    ) -> Result<(), TxError> {
        match self {
            AA::EOA(_) => EOA::check_nonce_is_used_after_validation(
                system,
                caller_ee_type,
                resources,
                tx_nonce,
                from,
            ),
            AA::Contract(_) => Contract::check_nonce_is_used_after_validation(
                system,
                caller_ee_type,
                resources,
                tx_nonce,
                from,
            ),
        }
    }

    #[allow(clippy::type_complexity)]
    #[allow(clippy::too_many_arguments)]
    pub fn pay_for_transaction<CS: Stack<StackFrame<S, SystemFrameSnapshot<S>>, S::Allocator>>(
        &self,
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        callstack: &mut CS,
        tx_hash: Bytes32,
        suggested_signed_hash: Bytes32,
        transaction: &mut ZkSyncTransaction<'static>,
        from: B160,
        caller_ee_type: ExecutionEnvironmentType,
        resources: &mut S::Resources,
    ) -> Result<(), TxError> {
        match self {
            AA::EOA(_) => EOA::pay_for_transaction::<CS>(
                system,
                system_functions,
                callstack,
                tx_hash,
                suggested_signed_hash,
                transaction,
                from,
                caller_ee_type,
                resources,
            ),
            AA::Contract(_) => Contract::pay_for_transaction::<CS>(
                system,
                system_functions,
                callstack,
                tx_hash,
                suggested_signed_hash,
                transaction,
                from,
                caller_ee_type,
                resources,
            ),
        }
    }
    #[allow(clippy::type_complexity)]
    #[allow(clippy::too_many_arguments)]
    pub fn pre_paymaster<CS: Stack<StackFrame<S, SystemFrameSnapshot<S>>, S::Allocator>>(
        &self,
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
    ) -> Result<(), TxError> {
        match self {
            AA::EOA(_) => EOA::pre_paymaster::<CS>(
                system,
                system_functions,
                callstack,
                tx_hash,
                suggested_signed_hash,
                transaction,
                from,
                paymaster,
                caller_ee_type,
                resources,
            ),
            AA::Contract(_) => Contract::pre_paymaster::<CS>(
                system,
                system_functions,
                callstack,
                tx_hash,
                suggested_signed_hash,
                transaction,
                from,
                paymaster,
                caller_ee_type,
                resources,
            ),
        }
    }
}
