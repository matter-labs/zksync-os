use super::gas_helpers::get_resources_for_tx;
use super::transaction::ZkSyncTransaction;
use super::*;
use crate::bootloader::account_models::ExecutionResult;
use crate::bootloader::account_models::AA;
use crate::bootloader::config::BasicBootloaderExecutionConfig;
use crate::bootloader::constants::UPGRADE_TX_NATIVE_PER_GAS;
use crate::bootloader::errors::TxError::Validation;
use crate::bootloader::errors::{InvalidTransaction, TxError};
use crate::bootloader::runner::RunnerMemoryBuffers;
use crate::{require, require_internal};
use constants::L1_TX_INTRINSIC_NATIVE_COST;
use constants::L1_TX_NATIVE_PRICE;
use constants::L2_TX_INTRINSIC_NATIVE_COST;
use constants::SIMULATION_NATIVE_PER_GAS;
use constants::{
    L1_TX_INTRINSIC_L2_GAS, L1_TX_INTRINSIC_PUBDATA, L2_TX_INTRINSIC_GAS, L2_TX_INTRINSIC_PUBDATA,
    MAX_BLOCK_GAS_LIMIT,
};
use evm_interpreter::ERGS_PER_GAS;
use gas_helpers::check_enough_resources_for_pubdata;
use gas_helpers::get_resources_to_charge_for_pubdata;
use system_hooks::addresses_constants::BOOTLOADER_FORMAL_ADDRESS;
use system_hooks::HooksStorage;
use zk_ee::system::errors::{FatalError, InternalError, SystemError, UpdateQueryError};
use zk_ee::system::{EthereumLikeTypes, Resources};

/// Return value of validation step
#[derive(Default)]
struct ValidationResult {
    validation_pubdata: u64,
}

impl<S: EthereumLikeTypes> BasicBootloader<S>
where
    S::IO: IOSubsystemExt,
{
    ///
    /// Process transaction.
    ///
    /// We are passing callstack from outside to reuse its memory space between different transactions.
    /// It's expected to be empty.
    ///
    pub fn process_transaction<'a, Config: BasicBootloaderExecutionConfig>(
        initial_calldata_buffer: &mut [u8],
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        memories: RunnerMemoryBuffers<'a>,
        is_first_tx: bool,
    ) -> Result<TxProcessingResult<'a>, TxError> {
        let transaction = ZkSyncTransaction::try_from_slice(initial_calldata_buffer)
            .map_err(|_| TxError::Validation(InvalidTransaction::InvalidEncoding))?;

        // Safe to unwrap here, as this should have been validated in the
        // previous call.
        let tx_type = transaction.tx_type.read();

        match tx_type {
            ZkSyncTransaction::UPGRADE_TX_TYPE => {
                if !is_first_tx {
                    Err(Validation(InvalidTransaction::UpgradeTxNotFirst))
                } else {
                    Self::process_l1_transaction(
                        system,
                        system_functions,
                        memories,
                        transaction,
                        false,
                    )
                }
            }
            ZkSyncTransaction::L1_L2_TX_TYPE => {
                Self::process_l1_transaction(system, system_functions, memories, transaction, true)
            }
            _ => Self::process_l2_transaction::<Config>(
                system,
                system_functions,
                memories,
                transaction,
            ),
        }
    }

    fn process_l1_transaction<'a>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        memories: RunnerMemoryBuffers<'a>,
        transaction: ZkSyncTransaction,
        is_priority_op: bool,
    ) -> Result<TxProcessingResult<'a>, TxError> {
        // The work done by the bootloader (outside of EE or EOA specific
        // computation) is charged as part of the intrinsic gas cost.
        let gas_limit = transaction.gas_limit.read();

        // The invariant that the user deposited more than the value needed
        // for the transaction must be enforced on L1, but we double-check it here
        // Note, that for now the property of block.base <= tx.maxFeePerGas does not work
        // for L1->L2 transactions. For now, these transactions are processed with the same gasPrice
        // they were provided on L1. In the future, we may apply a new logic for it.
        let gas_price = transaction.max_fee_per_gas.read();

        // For L1->L2 transactions we always use the pubdata price provided by the transaction.
        // This is needed to ensure DDoS protection. All the excess expenditure
        // will be refunded to the user.
        let gas_per_pubdata = transaction.gas_per_pubdata_limit.read();

        // For L1->L2 txs, we use a constant native price to avoid censorship.
        let native_price = L1_TX_NATIVE_PRICE;
        let native_per_gas = if is_priority_op {
            let mut native_per_gas = U256::from(gas_price);
            U256::div_ceil(&mut native_per_gas, &native_price);
            native_per_gas
        } else {
            UPGRADE_TX_NATIVE_PER_GAS
        };
        let native_per_pubdata = U256::from(gas_per_pubdata)
            .checked_mul(&native_per_gas)
            .ok_or(InternalError("gpp*npg"))?;

        let (mut resources, withheld_resources) = get_resources_for_tx::<S>(
            gas_limit,
            &native_per_pubdata,
            &native_per_gas,
            transaction.calldata(),
            L1_TX_INTRINSIC_L2_GAS,
            L1_TX_INTRINSIC_PUBDATA,
            L1_TX_INTRINSIC_NATIVE_COST,
        )?;

        let tx_internal_cost = gas_price
            .checked_mul(gas_limit as u128)
            .ok_or(InternalError("gp*gl l1"))?;
        let value = transaction.value.read();
        let total_deposited = transaction.reserved[0].read();
        let needed_amount = value
            .checked_add(&U256::from(tx_internal_cost))
            .ok_or(InternalError("v+tic"))?;
        require_internal!(
            total_deposited >= needed_amount,
            "Deposited amount too low",
            system
        )?;

        // TODO: l1 transaction preparation (marking factory deps)
        let chain_id = system.get_chain_id();

        let (tx_hash, preparation_out_of_resources): (Bytes32, bool) =
            match transaction.calculate_hash(chain_id, &mut resources) {
                Ok(h) => (h.into(), false),
                Err(FatalError::Internal(e)) => return Err(e.into()),
                Err(FatalError::OutOfNativeResources) => {
                    resources.exhaust_ergs();
                    // We need to compute the hash anyways, we do with inf resources
                    let mut inf_resources = S::Resources::FORMAL_INFINITE;
                    (
                        transaction
                            .calculate_hash(chain_id, &mut inf_resources)
                            .expect("must succeed")
                            .into(),
                        true,
                    )
                }
            };

        let result = if !preparation_out_of_resources {
            // Take a snapshot in case we need to revert due to out of native.
            let rollback_handle = system.start_global_frame()?;

            // Tx execution
            let from = transaction.from.read();
            let to = transaction.to.read();
            match Self::execute_l1_transaction_and_notify_result(
                system,
                system_functions,
                memories,
                &transaction,
                from,
                to,
                &value,
                &native_per_pubdata,
                &mut resources,
                withheld_resources,
            ) {
                Ok(r) => {
                    match r {
                        ExecutionResult::Success { .. } => system.finish_global_frame(None)?,
                        ExecutionResult::Revert { .. } => {
                            system.finish_global_frame(Some(&rollback_handle))?
                        }
                    }
                    r
                }
                // Out of native is converted to a top-level revert and
                // gas is exhausted.
                Err(FatalError::OutOfNativeResources) => {
                    resources.exhaust_ergs();
                    system.finish_global_frame(Some(&rollback_handle))?;
                    ExecutionResult::Revert { output: &[] }
                }
                Err(FatalError::Internal(e)) => return Err(e.into()),
            }
        } else {
            ExecutionResult::Revert { output: &[] }
        };

        // Compute gas to refund
        // TODO: consider operator refund
        let (_pubdata_spent, to_charge_for_pubdata) =
            get_resources_to_charge_for_pubdata(system, &native_per_pubdata, None)?;
        #[allow(unused_variables)]
        let (_, gas_used) = Self::compute_gas_refund(
            system,
            to_charge_for_pubdata,
            gas_limit,
            &native_per_gas,
            &mut resources,
        )?;

        // Mint fee to bootloader
        // We already checked that total_gas_refund <= gas_limit
        let pay_to_operator = U256::from(gas_used)
            .checked_mul(&U256::from(gas_price))
            .ok_or(InternalError("gu*gp"))?;
        let mut inf_resources = S::Resources::FORMAL_INFINITE;

        BasicBootloader::mint_token(
            system,
            &pay_to_operator,
            &BOOTLOADER_FORMAL_ADDRESS,
            &mut inf_resources,
        )
        .map_err(|e| match e {
            SystemError::OutOfErgs => InternalError("Out of ergs on infinite ergs"),
            SystemError::OutOfNativeResources => InternalError("Out of native on infinite"),
            SystemError::Internal(i) => i,
        })?;

        // Refund
        let to_refund_recipient = match result {
            ExecutionResult::Revert { .. } => {
                // Upgrade transactions must always succeed
                if !is_priority_op {
                    return Err(Validation(InvalidTransaction::UpgradeTxFailed));
                }
                // If the transaction reverts, then minting the msg.value to the
                // user has been reverted as well, so we can simply mint everything
                // that the user has deposited to the refund recipient
                total_deposited
                    .checked_sub(&pay_to_operator)
                    .ok_or(InternalError("td-pto"))
            }
            ExecutionResult::Success { .. } => {
                // If the transaction succeeds, then it is assumed that msg.value
                // was transferred correctly.
                // However, the remaining value deposited will be given to
                // the refund recipient.
                let value_plus_fee = value
                    .checked_add(&pay_to_operator)
                    .ok_or(InternalError("v+pto"))?;
                total_deposited
                    .checked_sub(&value_plus_fee)
                    .ok_or(InternalError("td-vpf"))
            }
        }?;
        if to_refund_recipient > U256::ZERO {
            let refund_recipient = u256_to_b160_checked(transaction.reserved[1].read());
            BasicBootloader::mint_token(
                system,
                &to_refund_recipient,
                &refund_recipient,
                &mut inf_resources,
            )
            .map_err(|e| match e {
                SystemError::OutOfErgs => InternalError("Out of ergs on infinite ergs"),
                SystemError::OutOfNativeResources => InternalError("Out of native on infinite"),
                SystemError::Internal(i) => i,
            })?;
        }

        // Emit log
        let success = matches!(result, ExecutionResult::Success { .. });
        let mut inf_resources = S::Resources::FORMAL_INFINITE;
        system.io.emit_l1_l2_tx_log(
            ExecutionEnvironmentType::NoEE,
            &mut inf_resources,
            tx_hash,
            success,
        )?;

        Ok(TxProcessingResult {
            result,
            tx_hash,
            is_l1_tx: is_priority_op,
            is_upgrade_tx: !is_priority_op,
            gas_used,
            gas_refunded: 0,
            #[cfg(feature = "report_native")]
            native_used: 0,
        })
    }

    fn execute_l1_transaction_and_notify_result<'a>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        memories: RunnerMemoryBuffers<'a>,
        transaction: &ZkSyncTransaction,
        from: B160,
        to: B160,
        value: &U256,
        native_per_pubdata: &U256,
        resources: &mut S::Resources,
        withheld_resources: S::Resources,
    ) -> Result<ExecutionResult<'a>, FatalError> {
        let _ = system
            .get_logger()
            .write_fmt(format_args!("Executing L1 transaction\n"));

        let gas_price = U256::from(transaction.max_fee_per_gas.read());
        system.set_tx_context(from, &gas_price);

        // Start a frame, to revert minting of value if execution fails
        let rollback_handle = system.start_global_frame()?;

        // First we mint value
        if value.is_zero() == false {
            resources
                .with_infinite_ergs(|inf_resources| {
                    BasicBootloader::mint_token(system, &value, &from, inf_resources)
                })
                .map_err(|e| match e {
                    SystemError::OutOfErgs => {
                        FatalError::Internal(InternalError("Out of ergs on infinite ergs"))
                    }
                    SystemError::OutOfNativeResources => FatalError::OutOfNativeResources,
                    SystemError::Internal(i) => FatalError::Internal(i),
                })?;
        }

        let resources_for_tx = resources.clone();

        // transaction is in managed region, so we can recast it back
        let calldata = transaction.calldata();

        // TODO: add support for deployment transactions,
        // probably unify with execution logic for EOA

        let CompletedExecution {
            resources_returned,
            reverted,
            return_values,
            ..
        } = BasicBootloader::run_single_interaction(
            system,
            system_functions,
            memories,
            calldata,
            &from,
            &to,
            resources_for_tx,
            &value,
            false,
        )?;
        *resources = resources_returned;
        system.finish_global_frame(reverted.then_some(&rollback_handle))?;

        let _ = system
            .get_logger()
            .write_fmt(format_args!("Main TX body successful = {}\n", !reverted));

        let returndata_region = return_values.returndata;

        let execution_result = if reverted {
            ExecutionResult::Revert {
                output: returndata_region,
            }
        } else {
            ExecutionResult::Success {
                output: ExecutionOutput::Call(returndata_region),
            }
        };

        // After the transaction is executed, we reclaim the withheld resources.
        // This is needed to ensure correct "gas_used" calculation, also these
        // resources could be spent for pubdata.
        resources.reclaim_withheld(withheld_resources);

        let execution_result =
            if !check_enough_resources_for_pubdata(system, native_per_pubdata, resources, None)? {
                let _ = system
                    .get_logger()
                    .write_fmt(format_args!("Not enough gas for pubdata after execution\n"));
                execution_result.reverted()
            } else {
                execution_result
            };

        Ok(execution_result)
    }

    fn process_l2_transaction<'a, Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        mut memories: RunnerMemoryBuffers<'a>,
        mut transaction: ZkSyncTransaction,
    ) -> Result<TxProcessingResult<'a>, TxError> {
        let from = transaction.from.read();
        let gas_limit = transaction.gas_limit.read();
        let calldata = transaction.calldata();

        // Validate that the transaction's gas limit is not larger than
        // the block's gas limit.
        let block_gas_limit = system.get_gas_limit();
        // First, check block gas limit can be represented as ergs.
        require!(
            block_gas_limit <= MAX_BLOCK_GAS_LIMIT,
            InvalidTransaction::BlockGasLimitTooHigh,
            system
        )?;
        let tx_gas_limit = transaction.gas_limit.read();
        require!(
            tx_gas_limit <= block_gas_limit,
            InvalidTransaction::CallerGasLimitMoreThanBlock,
            system
        )?;

        let gas_per_pubdata = system.get_gas_per_pubdata();
        let native_price = system.get_native_price();
        let gas_price = Self::get_gas_price(
            system,
            transaction.max_fee_per_gas.read(),
            transaction.max_priority_fee_per_gas.read(),
        )?;
        if native_price.is_zero() {
            return Err(InternalError("Native price cannot be 0").into());
        };
        let native_per_gas = if cfg!(feature = "resources_for_tester") {
            U256::from(crate::bootloader::constants::TESTER_NATIVE_PER_GAS as u64)
        } else if Config::ONLY_SIMULATE {
            SIMULATION_NATIVE_PER_GAS
        } else {
            let mut native_per_gas = gas_price.clone();
            U256::div_ceil(&mut native_per_gas, &native_price);
            native_per_gas
        };
        let native_per_pubdata = gas_per_pubdata
            .checked_mul(&native_per_gas)
            .ok_or(InternalError("gpp*npg"))?;

        let (mut resources, withheld_resources) = get_resources_for_tx::<S>(
            gas_limit,
            &native_per_pubdata,
            &native_per_gas,
            calldata,
            L2_TX_INTRINSIC_GAS,
            L2_TX_INTRINSIC_PUBDATA,
            L2_TX_INTRINSIC_NATIVE_COST,
        )?;
        let initial_resources = resources.clone();

        // we will read all account properties needed for future execution
        // The work done by the bootloader (outside of EE or EOA specific
        // computation) is charged as part of the intrinsic gas cost.
        let (caller_is_code, caller_ee_type, caller_nonce) = {
            let account_data = resources.with_infinite_ergs(|inf_resources| {
                system.io.read_account_properties(
                    ExecutionEnvironmentType::NoEE,
                    inf_resources,
                    &from,
                    AccountDataRequest::empty()
                        .with_ee_version()
                        .with_nonce()
                        .with_artifacts_len()
                        .with_bytecode_len(),
                )
            })?;

            (
                account_data.is_contract(),
                ExecutionEnvironmentType::parse_ee_version_byte(account_data.ee_version.0)?,
                account_data.nonce.0,
            )
        };

        let account_model =
            AA::account_model_for_account(&transaction, caller_is_code, Config::AA_ENABLED);

        account_model.charge_additional_intrinsic_gas(&mut resources, &transaction)?;

        system.set_tx_context(from, &gas_price);

        let chain_id = system.get_chain_id();

        // Process access list
        // Note: this operation should be performed before the hashing of the
        // transaction, as the latter assumes the transaction structure has
        // already been validated.
        transaction.parse_and_warm_up_access_list(system, &mut resources)?;

        let tx_hash: Bytes32 = transaction
            .calculate_hash(chain_id, &mut resources)
            .map_err(TxError::oon_as_validation)?
            .into();
        let suggested_signed_hash: Bytes32 = transaction
            .calculate_signed_hash(chain_id, &mut resources)
            .map_err(TxError::oon_as_validation)?
            .into();

        let ValidationResult { validation_pubdata } = if !Config::ONLY_SIMULATE {
            Self::transaction_validation::<Config>(
                system,
                system_functions,
                memories.reborrow(),
                &tx_hash,
                &suggested_signed_hash,
                &mut transaction,
                &account_model,
                from,
                &gas_price,
                &gas_per_pubdata,
                &native_per_pubdata,
                caller_ee_type,
                caller_is_code,
                caller_nonce,
                &mut resources,
            )?
        } else {
            ValidationResult::default()
        };

        // Take a snapshot in case we need to revert due to out of native.
        let rollback_handle = system.start_global_frame()?;

        let execution_result = match Self::transaction_execution(
            system,
            system_functions,
            memories,
            &tx_hash,
            &suggested_signed_hash,
            &mut transaction,
            &account_model,
            &native_per_pubdata,
            validation_pubdata,
            caller_nonce,
            &mut resources,
        ) {
            Ok(r) => {
                match r {
                    ExecutionResult::Success { .. } => system.finish_global_frame(None)?,
                    ExecutionResult::Revert { .. } => {
                        system.finish_global_frame(Some(&rollback_handle))?
                    }
                }
                r
            }
            // Out of native is converted to a top-level revert and
            // gas is exhausted.
            Err(FatalError::OutOfNativeResources) => {
                let _ = system
                    .get_logger()
                    .write_fmt(format_args!("Transaction ran out of native resource\n"));
                resources.exhaust_ergs();
                system.finish_global_frame(Some(&rollback_handle))?;
                ExecutionResult::Revert { output: &[] }
            }
            Err(FatalError::Internal(e)) => return Err(e.into()),
        };

        let resources_before_refund = resources.clone();
        // After the transaction is executed, we reclaim the withheld resources.
        // This is needed to ensure correct "gas_used" calculation, also these
        // resources could be spent for pubdata.
        resources.reclaim_withheld(withheld_resources);

        let gas_used = if !Config::ONLY_SIMULATE {
            Self::refund_transaction::<Config>(
                system,
                system_functions,
                &tx_hash,
                &suggested_signed_hash,
                &mut transaction,
                from,
                &execution_result,
                &gas_price,
                &native_per_gas,
                &native_per_pubdata,
                validation_pubdata,
                caller_ee_type,
                &mut resources,
            )?
        } else {
            0
        };

        #[cfg(not(target_arch = "riscv32"))]
        cycle_marker::log_marker(
            format!(
                "Spent ergs for [process_transaction]: {}",
                gas_used * ERGS_PER_GAS
            )
            .as_str(),
        );
        #[cfg(not(target_arch = "riscv32"))]
        cycle_marker::log_marker(
            format!(
                "Spent native for [process_transaction]: {}",
                resources_before_refund
                    .clone()
                    .diff(initial_resources.clone())
                    .native()
                    .as_u64()
            )
            .as_str(),
        );

        Ok(TxProcessingResult {
            result: execution_result,
            tx_hash,
            is_l1_tx: false,
            is_upgrade_tx: false,
            gas_used,
            gas_refunded: 0,
            #[cfg(feature = "report_native")]
            native_used: resources_before_refund
                .diff(initial_resources)
                .native()
                .as_u64(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn transaction_validation<Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        mut memories: RunnerMemoryBuffers,
        tx_hash: &Bytes32,
        suggested_signed_hash: &Bytes32,
        transaction: &mut ZkSyncTransaction,
        account_model: &AA<S>,
        from: B160,
        gas_price: &U256,
        gas_per_pubdata: &U256,
        native_per_pubdata: &U256,
        caller_ee_type: ExecutionEnvironmentType,
        caller_is_code: bool,
        caller_nonce: u64,
        resources: &mut S::Resources,
    ) -> Result<ValidationResult, TxError> {
        let _ = system
            .get_logger()
            .write_fmt(format_args!("Start of validation\n"));

        let user_gas_per_pubdata_limit = transaction.get_user_gas_per_pubdata_limit();
        // Validate the user provided gas per pubdata
        require!(
            &user_gas_per_pubdata_limit >= gas_per_pubdata,
            InvalidTransaction::GasPerPubdataTooHigh,
            system
        )?;

        // Nonce validation
        let tx_nonce = u256_try_to_u64(&transaction.nonce.read()).ok_or(TxError::from(
            InvalidTransaction::NonceOverflowInTransaction,
        ))?;

        account_model.check_nonce_is_not_used(caller_nonce, tx_nonce)?;

        // AA validation
        account_model.validate(
            system,
            system_functions,
            memories.reborrow(),
            tx_hash,
            suggested_signed_hash,
            transaction,
            caller_ee_type,
            caller_is_code,
            caller_nonce,
            resources,
        )?;

        // Check nonce has been marked
        account_model.check_nonce_is_used_after_validation(
            system,
            caller_ee_type,
            resources,
            tx_nonce,
            from,
        )?;

        let _ = system.get_logger().write_fmt(format_args!(
            "Transaction was validated, can collect fees\n"
        ));

        // Charge fees
        Self::ensure_payment::<Config>(
            system,
            system_functions,
            memories,
            tx_hash,
            suggested_signed_hash,
            transaction,
            account_model,
            from,
            gas_price,
            caller_ee_type,
            resources,
        )?;

        // Charge for validation pubdata
        let (validation_pubdata, to_charge_for_pubdata) =
            get_resources_to_charge_for_pubdata(system, native_per_pubdata, None)?;
        resources.charge(&to_charge_for_pubdata)?;

        let _ = system
            .get_logger()
            .write_fmt(format_args!("Validation completed\n"));

        Ok(ValidationResult { validation_pubdata })
    }

    #[allow(clippy::too_many_arguments)]
    fn transaction_execution<'a>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        memories: RunnerMemoryBuffers<'a>,
        tx_hash: &Bytes32,
        suggested_signed_hash: &Bytes32,
        transaction: &mut ZkSyncTransaction,
        account_model: &AA<S>,
        native_per_pubdata: &U256,
        validation_pubdata: u64,
        current_tx_nonce: u64,
        resources: &mut S::Resources,
    ) -> Result<ExecutionResult<'a>, FatalError> {
        let _ = system
            .get_logger()
            .write_fmt(format_args!("Start of execution\n"));

        // TODO: factory deps? Probably fine to ignore for now

        // AA execution
        let execution_result = account_model.execute(
            system,
            system_functions,
            memories,
            tx_hash,
            suggested_signed_hash,
            transaction,
            current_tx_nonce,
            resources,
        )?;

        let _ = system
            .get_logger()
            .write_fmt(format_args!("Transaction execution completed\n"));

        if !check_enough_resources_for_pubdata(
            system,
            native_per_pubdata,
            resources,
            Some(validation_pubdata),
        )? {
            let _ = system
                .get_logger()
                .write_fmt(format_args!("Not enough gas for pubdata after execution\n"));
            Ok(execution_result.reverted())
        } else {
            Ok(execution_result)
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn ensure_payment<Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        mut memories: RunnerMemoryBuffers,
        tx_hash: &Bytes32,
        suggested_signed_hash: &Bytes32,
        transaction: &mut ZkSyncTransaction,
        account_model: &AA<S>,
        from: B160,
        gas_price: &U256,
        caller_ee_type: ExecutionEnvironmentType,
        resources: &mut S::Resources,
    ) -> Result<(), TxError> {
        let paymaster = transaction.paymaster.read();

        // Bootloader balance before fee payment
        let bootloader_balance_before = resources.with_infinite_ergs(|inf_resources| {
            system.io.get_nominal_token_balance(
                ExecutionEnvironmentType::NoEE,
                inf_resources,
                &BOOTLOADER_FORMAL_ADDRESS,
            )
        })?;
        let required_funds = gas_price
            .checked_mul(&U256::from(transaction.gas_limit.read()))
            .ok_or(InternalError("gp*gl"))?;
        // First we charge the fees, then we verify the bootloader got
        // the funds.
        // Paymaster flow is only allowed when AA is enabled.
        let payer = if Config::AA_ENABLED && paymaster != B160::ZERO {
            // Paymaster flow
            // First, the `prepareForPaymaster` method of the user's account is called.
            account_model.pre_paymaster(
                system,
                system_functions,
                memories.reborrow(),
                tx_hash,
                suggested_signed_hash,
                transaction,
                from,
                paymaster,
                caller_ee_type,
                resources,
            )?;

            let return_values = Self::validate_and_pay_for_paymaster_transaction(
                system,
                system_functions,
                memories.reborrow(),
                transaction,
                tx_hash,
                suggested_signed_hash,
                paymaster,
                caller_ee_type,
                resources,
            )?;
            let pre_tx_buffer = transaction.pre_tx_buffer();
            Self::store_paymaster_context_and_check_magic(system, pre_tx_buffer, &return_values)?;

            paymaster
        } else {
            // No paymaster
            account_model.pay_for_transaction(
                system,
                system_functions,
                memories,
                tx_hash,
                suggested_signed_hash,
                transaction,
                from,
                caller_ee_type,
                resources,
            )?;

            from
        };
        // Check bootloader got the funds and maybe return excessive funds
        let bootloader_balance_after = resources.with_infinite_ergs(|inf_resources| {
            system.io.get_nominal_token_balance(
                ExecutionEnvironmentType::NoEE,
                inf_resources,
                &BOOTLOADER_FORMAL_ADDRESS,
            )
        })?;
        let bootloader_received_funds = bootloader_balance_after
            .checked_sub(&bootloader_balance_before)
            .ok_or(InternalError("bba-bbb"))?;
        // If the amount of funds provided to the bootloader is less than the minimum required one
        // then this transaction should be rejected.
        require!(
            bootloader_received_funds.ge(&required_funds),
            InvalidTransaction::ReceivedInsufficientFees {
                received: bootloader_received_funds.clone(),
                required: required_funds.clone()
            },
            system
        )?;
        let excessive_funds = bootloader_received_funds
            .checked_sub(&required_funds)
            .ok_or(InternalError("brf-rf"))?;
        if excessive_funds.is_zero() == false {
            resources
                .with_infinite_ergs(|inf_resources| {
                    system.io.transfer_nominal_token_value(
                        caller_ee_type,
                        inf_resources,
                        &BOOTLOADER_FORMAL_ADDRESS,
                        &payer,
                        &excessive_funds,
                    )
                })
                .map_err(|e| match e {
                    UpdateQueryError::NumericBoundsError => SystemError::Internal(InternalError(
                        "Bootloader cannot return excessive funds",
                    )),
                    UpdateQueryError::System(e) => e,
                })?;
        }
        Ok(())
    }

    fn get_gas_price(
        system: &mut System<S>,
        max_fee_per_gas: u128,
        max_priority_fee_per_gas: u128,
    ) -> Result<U256, TxError> {
        let max_fee_per_gas = U256::from(max_fee_per_gas);
        let max_priority_fee_per_gas = U256::from(max_priority_fee_per_gas);
        let base_fee = system.get_eip1559_basefee();
        require!(
            max_priority_fee_per_gas <= max_fee_per_gas,
            TxError::Validation(InvalidTransaction::PriorityFeeGreaterThanMaxFee,),
            system
        )?;
        require!(
            base_fee <= max_fee_per_gas,
            TxError::Validation(InvalidTransaction::BaseFeeGreaterThanMaxFee,),
            system
        )?;
        let priority_fee_per_gas = if cfg!(feature = "charge_priority_fee") {
            let mut t = max_fee_per_gas;
            let _ = t.overflowing_sub_assign(&base_fee);
            core::cmp::min(max_priority_fee_per_gas, t)
        } else {
            U256::ZERO
        };
        let mut t = base_fee;
        let _ = t.overflowing_add_assign(&priority_fee_per_gas);

        Ok(t)
    }

    #[allow(clippy::too_many_arguments)]
    fn refund_transaction<Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        _system_functions: &mut HooksStorage<S, S::Allocator>,
        _tx_hash: &Bytes32,
        _suggested_signed_hash: &Bytes32,
        transaction: &mut ZkSyncTransaction,
        from: B160,
        execution_result: &ExecutionResult,
        gas_price: &U256,
        native_per_gas: &U256,
        native_per_pubdata: &U256,
        validation_pubdata: u64,
        caller_ee_type: ExecutionEnvironmentType,
        resources: &mut S::Resources,
    ) -> Result<u64, InternalError> {
        let paymaster = transaction.paymaster.read();
        let _ = system
            .get_logger()
            .write_fmt(format_args!("Start of refund\n"));
        let _success = matches!(execution_result, ExecutionResult::Success { .. });
        let _max_refunded_gas = resources.ergs().0.div_floor(ERGS_PER_GAS);
        let refund_recipient = if Config::AA_ENABLED && paymaster != B160::ZERO {
            // TODO: can paymaster post op run out of native?
            // let _succeeded = Self::paymaster_post_op::<_>(
            //     system,
            //     system_functions,
            //     callstack,
            //     transaction,
            //     tx_hash,
            //     suggested_signed_hash,
            //     success,
            //     max_refunded_gas,
            //     paymaster,
            //     gas_per_pubdata,
            //     validation_pubdata,
            //     resources,
            // )?;
            // TODO: what should we do if postOp reverts
            paymaster
        } else {
            // No paymaster
            from
        };

        // TODO: consider operator refund

        // Pubdata for validation has been charged already,
        // we charge for the rest now.
        let (_pubdata_spent, to_charge_for_pubdata) = get_resources_to_charge_for_pubdata(
            system,
            &native_per_pubdata,
            Some(validation_pubdata),
        )?;
        let (total_gas_refund, gas_used) = Self::compute_gas_refund(
            system,
            to_charge_for_pubdata,
            transaction.gas_limit.read(),
            native_per_gas,
            resources,
        )?;
        let token_to_refund = total_gas_refund
            .checked_mul(gas_price)
            .ok_or(InternalError("tgf*gp"))?;
        let mut inf_resources = S::Resources::FORMAL_INFINITE;
        system
            .io
            .transfer_nominal_token_value(
                caller_ee_type,
                &mut inf_resources,
                &BOOTLOADER_FORMAL_ADDRESS,
                &refund_recipient,
                &token_to_refund,
            )
            .map_err(|e| match e {
                UpdateQueryError::NumericBoundsError => {
                    InternalError("Bootloader cannot pay for refund")
                }
                UpdateQueryError::System(SystemError::OutOfErgs) => {
                    InternalError("should transfer refund")
                }
                UpdateQueryError::System(SystemError::OutOfNativeResources) => {
                    InternalError("should transfer refund")
                }
                UpdateQueryError::System(SystemError::Internal(e)) => e,
            })?;
        Ok(gas_used)
    }

    // Returns (gas_refund, gas_used)
    fn compute_gas_refund(
        system: &mut System<S>,
        to_charge_for_pubdata: S::Resources,
        gas_limit: u64,
        native_per_gas: &U256,
        resources: &mut S::Resources,
    ) -> Result<(U256, u64), InternalError> {
        // Already checked
        resources.charge_unchecked(&to_charge_for_pubdata);

        let mut gas_used = gas_limit - resources.ergs().0.div_floor(ERGS_PER_GAS);
        resources.exhaust_ergs();

        #[cfg(not(feature = "unlimited_native"))]
        {
            // Adjust gas_used with difference with used native
            let native_per_gas = u256_to_u64_saturated(&native_per_gas);
            let full_native_limit = gas_limit.saturating_mul(native_per_gas);
            let native_used = full_native_limit - resources.native().remaining().as_u64();

            let delta_gas = if native_per_gas == 0 {
                0
            } else {
                (native_used / native_per_gas) as i64 - (gas_used as i64)
            };

            if delta_gas > 0 {
                // In this case, the native resource consumption is more than the
                // gas consumption accounted for. Consume extra gas.
                gas_used += delta_gas as u64;
            }
            // TODO: return delta_gas to gas_used?
        }

        let total_gas_refund = gas_limit - gas_used;
        let _ = system
            .get_logger()
            .write_fmt(format_args!("Gas refund: {}\n", total_gas_refund));
        require_internal!(
            total_gas_refund.le(&gas_limit),
            "Gas refund greater than gas limit",
            system
        )?;
        let total_gas_refund = U256::from(total_gas_refund);
        Ok((total_gas_refund, gas_used))
    }
}
