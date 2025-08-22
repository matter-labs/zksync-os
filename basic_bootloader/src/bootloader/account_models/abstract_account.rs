use core::marker::PhantomData;

use crate::bootloader::account_models::contract::Contract;
use crate::bootloader::account_models::eoa::EOA;
use crate::bootloader::account_models::AccountModel;
use crate::bootloader::account_models::{ExecutionResult, TxError};
use crate::bootloader::errors::BootloaderSubsystemError;
use crate::bootloader::runner::RunnerMemoryBuffers;
use crate::bootloader::transaction::ZkSyncTransaction;
use crate::bootloader::BasicBootloaderExecutionConfig;
use crate::bootloader::Bytes32;
use ruint::aliases::B160;
use system_hooks::HooksStorage;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::system::tracer::Tracer;
use zk_ee::system::{EthereumLikeTypes, IOSubsystemExt, System};

pub enum AA<S> {
    EOA(PhantomData<S>),
    Contract(PhantomData<S>),
}

impl<S: EthereumLikeTypes> AA<S>
where
    S::IO: IOSubsystemExt,
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
    pub fn validate<Config: BasicBootloaderExecutionConfig>(
        &self,
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
        tracer: &mut impl Tracer<S>,
    ) -> Result<(), TxError> {
        match self {
            AA::EOA(_) => EOA::validate::<Config>(
                system,
                system_functions,
                memories,
                tx_hash,
                suggested_signed_hash,
                transaction,
                caller_ee_type,
                caller_is_code,
                caller_nonce,
                resources,
                tracer,
            ),
            AA::Contract(_) => Contract::validate::<Config>(
                system,
                system_functions,
                memories,
                tx_hash,
                suggested_signed_hash,
                transaction,
                caller_ee_type,
                caller_is_code,
                caller_nonce,
                resources,
                tracer,
            ),
        }
    }

    #[allow(clippy::type_complexity)]
    #[allow(clippy::too_many_arguments)]
    pub fn execute<'a>(
        &self,
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        memories: RunnerMemoryBuffers<'a>,
        tx_hash: Bytes32,
        suggested_signed_hash: Bytes32,
        transaction: &mut ZkSyncTransaction,
        current_tx_nonce: u64,
        resources: &mut S::Resources,
        tracer: &mut impl Tracer<S>,
    ) -> Result<ExecutionResult<'a>, BootloaderSubsystemError> {
        match self {
            AA::EOA(_) => EOA::execute(
                system,
                system_functions,
                memories,
                tx_hash,
                suggested_signed_hash,
                transaction,
                current_tx_nonce,
                resources,
                tracer,
            ),
            AA::Contract(_) => Contract::execute(
                system,
                system_functions,
                memories,
                tx_hash,
                suggested_signed_hash,
                transaction,
                current_tx_nonce,
                resources,
                tracer,
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
    pub fn pay_for_transaction(
        &self,
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        memories: RunnerMemoryBuffers<'_>,
        tx_hash: Bytes32,
        suggested_signed_hash: Bytes32,
        transaction: &mut ZkSyncTransaction,
        from: B160,
        caller_ee_type: ExecutionEnvironmentType,
        resources: &mut S::Resources,
        tracer: &mut impl Tracer<S>,
    ) -> Result<(), TxError> {
        match self {
            AA::EOA(_) => EOA::pay_for_transaction(
                system,
                system_functions,
                memories,
                tx_hash,
                suggested_signed_hash,
                transaction,
                from,
                caller_ee_type,
                resources,
                tracer,
            ),
            AA::Contract(_) => Contract::pay_for_transaction(
                system,
                system_functions,
                memories,
                tx_hash,
                suggested_signed_hash,
                transaction,
                from,
                caller_ee_type,
                resources,
                tracer,
            ),
        }
    }
    #[allow(clippy::type_complexity)]
    #[allow(clippy::too_many_arguments)]
    pub fn pre_paymaster(
        &self,
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
        tracer: &mut impl Tracer<S>,
    ) -> Result<(), TxError> {
        match self {
            AA::EOA(_) => EOA::pre_paymaster(
                system,
                system_functions,
                memories,
                tx_hash,
                suggested_signed_hash,
                transaction,
                from,
                paymaster,
                caller_ee_type,
                resources,
                tracer,
            ),
            AA::Contract(_) => Contract::pre_paymaster(
                system,
                system_functions,
                memories,
                tx_hash,
                suggested_signed_hash,
                transaction,
                from,
                paymaster,
                caller_ee_type,
                resources,
                tracer,
            ),
        }
    }
}
