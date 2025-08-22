use crate::bootloader::account_models::{AccountModel, ExecutionOutput, ExecutionResult};
use crate::bootloader::config::BasicBootloaderExecutionConfig;
use crate::bootloader::constants::PREPARE_FOR_PAYMASTER_SELECTOR;
use crate::bootloader::constants::{
    EXECUTE_SELECTOR, PAY_FOR_TRANSACTION_SELECTOR, VALIDATE_SELECTOR,
};
use crate::bootloader::errors::{AAMethod, BootloaderSubsystemError, InvalidTransaction, TxError};
use crate::bootloader::runner::RunnerMemoryBuffers;
use crate::bootloader::transaction::ZkSyncTransaction;
use crate::bootloader::{BasicBootloader, Bytes32};
use crate::require;
use core::fmt::Write;
use ruint::aliases::B160;
use system_hooks::HooksStorage;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::system::tracer::Tracer;
use zk_ee::system::{logger::Logger, *};

pub struct Contract;

impl<S: EthereumLikeTypes> AccountModel<S> for Contract
where
    S::IO: IOSubsystemExt,
{
    fn validate<Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        memories: RunnerMemoryBuffers,
        tx_hash: Bytes32,
        suggested_signed_hash: Bytes32,
        transaction: &mut ZkSyncTransaction,
        _caller_ee_type: ExecutionEnvironmentType,
        _caller_is_code: bool,
        _caller_nonce: u64,
        resources: &mut S::Resources,
        tracer: &mut impl Tracer<S>,
    ) -> Result<(), TxError> {
        let from = transaction.from.read();

        let _ = system
            .get_logger()
            .write_fmt(format_args!("About to start AA validation\n"));

        let CompletedExecution {
            resources_returned,
            result,
        } = BasicBootloader::call_account_method(
            system,
            system_functions,
            memories,
            transaction,
            tx_hash,
            suggested_signed_hash,
            from,
            VALIDATE_SELECTOR,
            resources,
            tracer,
        )
        .map_err(TxError::oon_as_validation)?;

        let reverted = result.failed();
        let return_values = result.return_values();

        let returndata_slice = return_values.returndata;
        *resources = resources_returned;

        let res: Result<(), TxError> = if reverted {
            Err(TxError::Validation(InvalidTransaction::Revert {
                method: AAMethod::AccountValidate,
                output: None, // TODO
            }))
        } else if returndata_slice.len() != 32 {
            Err(TxError::Validation(
                InvalidTransaction::InvalidReturndataLength,
            ))
        } else if &returndata_slice[..4] != VALIDATE_SELECTOR {
            Err(TxError::Validation(InvalidTransaction::InvalidMagic))
        } else {
            Ok(())
        };

        // system.purge_return_memory();

        res
    }

    fn execute<'a>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        memories: RunnerMemoryBuffers<'a>,
        tx_hash: Bytes32,
        suggested_signed_hash: Bytes32,
        transaction: &mut ZkSyncTransaction,
        _current_tx_nonce: u64,
        resources: &mut S::Resources,
        tracer: &mut impl Tracer<S>,
    ) -> Result<ExecutionResult<'a>, BootloaderSubsystemError> {
        let _ = system
            .get_logger()
            .write_fmt(format_args!("About to start AA execution\n"));

        let from = transaction.from.read();

        let CompletedExecution {
            resources_returned,
            result,
        } = BasicBootloader::call_account_method(
            system,
            system_functions,
            memories,
            transaction,
            tx_hash,
            suggested_signed_hash,
            from,
            EXECUTE_SELECTOR,
            resources,
            tracer,
        )?;

        let reverted = result.failed();
        let return_values = result.return_values();

        let resources_after_main_tx = resources_returned;

        let returndata_region = return_values.returndata;

        let _ = system
            .get_logger()
            .log_data(returndata_region.iter().copied());

        let _ = system
            .get_logger()
            .write_fmt(format_args!("Main TX body successful = {}\n", !reverted));

        let _ = system.get_logger().write_fmt(format_args!(
            "Resources to refund = {resources_after_main_tx:?}\n"
        ));

        *resources = resources_after_main_tx;

        let res = if reverted {
            ExecutionResult::Revert {
                output: returndata_region,
            }
        } else {
            ExecutionResult::Success {
                output: ExecutionOutput::Call(returndata_region),
            }
        };

        Ok(res)
    }

    ///
    /// For contract account, we allow a tx nonce to be greater or equal to
    /// the account's nonce.
    ///
    fn check_nonce_is_not_used(account_data_nonce: u64, tx_nonce: u64) -> Result<(), TxError> {
        if tx_nonce < account_data_nonce {
            return Err(TxError::Validation(InvalidTransaction::NonceUsedAlready));
        }
        Ok(())
    }

    fn check_nonce_is_used_after_validation(
        system: &mut System<S>,
        caller_ee_type: ExecutionEnvironmentType,
        resources: &mut S::Resources,
        tx_nonce: u64,
        from: B160,
    ) -> Result<(), TxError> {
        // Check that the account's nonce is > tx_nonce
        let acc_nonce = system.io.read_nonce(caller_ee_type, resources, &from)?;
        require!(
            acc_nonce > tx_nonce,
            TxError::Validation(InvalidTransaction::NonceNotIncreased),
            system
        )
    }

    fn pay_for_transaction(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        memories: RunnerMemoryBuffers,
        tx_hash: Bytes32,
        suggested_signed_hash: Bytes32,
        transaction: &mut ZkSyncTransaction,
        from: B160,
        _caller_ee_type: ExecutionEnvironmentType,
        resources: &mut S::Resources,
        tracer: &mut impl Tracer<S>,
    ) -> Result<(), TxError> {
        let _ = system
            .get_logger()
            .write_fmt(format_args!("About to start AA fee payment\n"));

        let CompletedExecution {
            resources_returned,
            result,
        } = BasicBootloader::call_account_method(
            system,
            system_functions,
            memories,
            transaction,
            tx_hash,
            suggested_signed_hash,
            from,
            PAY_FOR_TRANSACTION_SELECTOR,
            resources,
            tracer,
        )
        .map_err(TxError::oon_as_validation)?;

        let reverted = result.failed();

        *resources = resources_returned;

        let res: Result<(), TxError> = if reverted {
            Err(TxError::Validation(InvalidTransaction::Revert {
                method: AAMethod::AccountPayForTransaction,
                output: None, // TODO
            }))
        } else {
            Ok(())
        };

        // system.purge_return_memory();

        res
    }

    fn pre_paymaster(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        memories: RunnerMemoryBuffers,
        tx_hash: Bytes32,
        suggested_signed_hash: Bytes32,
        transaction: &mut ZkSyncTransaction,
        from: B160,
        _paymaster: B160,
        _caller_ee_type: ExecutionEnvironmentType,
        resources: &mut S::Resources,
        tracer: &mut impl Tracer<S>,
    ) -> Result<(), TxError> {
        let _ = system
            .get_logger()
            .write_fmt(format_args!("About to start call to prepareForPaymaster\n"));

        let CompletedExecution {
            resources_returned,
            result,
        } = BasicBootloader::call_account_method(
            system,
            system_functions,
            memories,
            transaction,
            tx_hash,
            suggested_signed_hash,
            from,
            PREPARE_FOR_PAYMASTER_SELECTOR,
            resources,
            tracer,
        )
        .map_err(TxError::oon_as_validation)?;
        *resources = resources_returned;

        let reverted = result.failed();

        let res: Result<(), TxError> = if reverted {
            Err(TxError::Validation(InvalidTransaction::Revert {
                method: AAMethod::AccountPrePaymaster,
                output: None, // todo
            }))
        } else {
            Ok(())
        };

        // system.purge_return_memory();
        res
    }

    fn charge_additional_intrinsic_gas(
        _resources: &mut S::Resources,
        _transaction: &ZkSyncTransaction,
    ) -> Result<(), TxError> {
        Ok(())
    }
}
