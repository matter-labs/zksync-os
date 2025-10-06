use super::gas_helpers::get_resources_for_tx;
use super::transaction::ZkSyncTransaction;
use super::*;
use crate::bootloader::config::BasicBootloaderExecutionConfig;
use crate::bootloader::constants::UPGRADE_TX_NATIVE_PER_GAS;
use crate::bootloader::errors::BootloaderInterfaceError;
use crate::bootloader::errors::TxError::Validation;
use crate::bootloader::errors::{InvalidTransaction, TxError};
use crate::bootloader::runner::RunnerMemoryBuffers;
use crate::bootloader::transaction_flow::ExecutionResult;
use crate::{require, require_internal};
use constants::L1_TX_INTRINSIC_NATIVE_COST;
use constants::L1_TX_NATIVE_PRICE;
use constants::L2_TX_INTRINSIC_NATIVE_COST;
use constants::SIMULATION_NATIVE_PER_GAS;
use constants::{
    L1_TX_INTRINSIC_L2_GAS, L1_TX_INTRINSIC_PUBDATA, L2_TX_INTRINSIC_GAS, L2_TX_INTRINSIC_PUBDATA,
    MAX_BLOCK_GAS_LIMIT,
};
use errors::BootloaderSubsystemError;
use evm_interpreter::ERGS_PER_GAS;
use gas_helpers::check_enough_resources_for_pubdata;
use gas_helpers::get_resources_to_charge_for_pubdata;
use gas_helpers::ResourcesForTx;
use system_hooks::addresses_constants::BOOTLOADER_FORMAL_ADDRESS;
use system_hooks::HooksStorage;
use zk_ee::interface_error;
use zk_ee::internal_error;
use zk_ee::system::errors::cascade::CascadedError;
use zk_ee::system::errors::interface::InterfaceError;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::system::errors::root_cause::GetRootCause;
use zk_ee::system::errors::root_cause::RootCause;
use zk_ee::system::errors::runtime::RuntimeError;
use zk_ee::system::errors::subsystem::SubsystemError;
use zk_ee::system::{EthereumLikeTypes, Resources};
use zk_ee::wrap_error;

/// Return value of validation step
#[derive(Default)]
struct ValidationResult {
    validation_pubdata: u64,
}

impl<S: EthereumLikeTypes, F: BasicTransactionFlow<S>> BasicBootloader<S, F>
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
        tracer: &mut impl Tracer<S>,
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
                    Self::process_l1_transaction::<Config>(
                        system,
                        system_functions,
                        memories,
                        transaction,
                        false,
                        tracer,
                    )
                }
            }
            ZkSyncTransaction::L1_L2_TX_TYPE => Self::process_l1_transaction::<Config>(
                system,
                system_functions,
                memories,
                transaction,
                true,
                tracer,
            ),
            _ => Self::process_l2_transaction::<Config>(
                system,
                system_functions,
                memories,
                transaction,
                tracer,
            ),
        }
    }

    fn process_l1_transaction<'a, Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        memories: RunnerMemoryBuffers<'a>,
        transaction: ZkSyncTransaction,
        is_priority_op: bool,
        tracer: &mut impl Tracer<S>,
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
            if Config::SIMULATION {
                SIMULATION_NATIVE_PER_GAS
            } else {
                U256::from(gas_price).div_ceil(native_price)
            }
        } else {
            UPGRADE_TX_NATIVE_PER_GAS
        };
        let native_per_pubdata = U256::from(gas_per_pubdata)
            .checked_mul(native_per_gas)
            .ok_or(internal_error!("gpp*npg"))?;

        let ResourcesForTx {
            main_resources: mut resources,
            withheld: withheld_resources,
            intrinsic_computational_native_charged,
        } = get_resources_for_tx::<S>(
            gas_limit,
            native_per_pubdata,
            native_per_gas,
            transaction.calldata(),
            L1_TX_INTRINSIC_L2_GAS,
            L1_TX_INTRINSIC_PUBDATA,
            L1_TX_INTRINSIC_NATIVE_COST,
            true,
        )?;
        // Just used for computing native used
        let initial_resources = resources.clone();

        let tx_internal_cost = gas_price
            .checked_mul(gas_limit as u128)
            .ok_or(internal_error!("gp*gl"))?;
        let value = transaction.value.read();
        let total_deposited = transaction.reserved[0].read();
        let needed_amount = value
            .checked_add(U256::from(tx_internal_cost))
            .ok_or(internal_error!("v+tic"))?;
        require_internal!(
            total_deposited >= needed_amount,
            "Deposited amount too low",
            system
        )?;

        // TODO: l1 transaction preparation (marking factory deps)
        let chain_id = system.get_chain_id();

        let (tx_hash, preparation_out_of_resources): (Bytes32, bool) = match transaction
            .calculate_hash(chain_id, &mut resources)
        {
            Ok(h) => (h.into(), false),
            Err(e) => {
                match e {
                    TxError::Internal(e) if !matches!(e.root_cause(), RootCause::Runtime(_)) => {
                        return Err(e.into());
                    }
                    // Only way hashing of L1 tx can fail due to Validation or Runtime is
                    // due to running out of native.
                    _ => {
                        let _ = system.get_logger().write_fmt(format_args!(
                            "Transaction preparation exhausted native resources: {e:?}\n"
                        ));

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
                }
            }
        };

        // pubdata_info = (pubdata_used, to_charge_for_pubdata) can be cached
        // to used in the refund step only if the execution succeeded.
        // Otherwise, this value needs to be recomputed after reverting
        // state changes.
        let (result, pubdata_info, resources_before_refund) = if !preparation_out_of_resources {
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
                value,
                native_per_pubdata,
                &mut resources,
                withheld_resources,
                tracer,
            ) {
                Ok((r, pubdata_used, to_charge_for_pubdata, resources_before_refund)) => {
                    let pubdata_info = match r {
                        ExecutionResult::Success { .. } => {
                            system.finish_global_frame(None)?;
                            Some((pubdata_used, to_charge_for_pubdata))
                        }
                        ExecutionResult::Revert { .. } => {
                            system.finish_global_frame(Some(&rollback_handle))?;
                            None
                        }
                    };
                    (r, pubdata_info, resources_before_refund)
                }
                Err(e) => {
                    match e.root_cause() {
                        // Out of native is converted to a top-level revert and
                        // gas is exhausted.
                        RootCause::Runtime(e @ RuntimeError::FatalRuntimeError(_)) => {
                            let _ = system.get_logger().write_fmt(format_args!(
                                "L1 transaction ran out of native resources or memory {e:?}\n"
                            ));
                            resources.exhaust_ergs();
                            system.finish_global_frame(Some(&rollback_handle))?;
                            (
                                ExecutionResult::Revert { output: &[] },
                                None,
                                S::Resources::empty(),
                            )
                        }
                        _ => return Err(e.into()),
                    }
                }
            }
        } else {
            (
                ExecutionResult::Revert { output: &[] },
                None,
                S::Resources::empty(),
            )
        };

        // Compute gas to refund
        // TODO: consider operator refund
        #[allow(unused_variables)]
        let (pubdata_used, to_charge_for_pubdata) = match pubdata_info {
            Some(r) => r,
            None => get_resources_to_charge_for_pubdata(system, native_per_pubdata, None)?,
        };

        #[allow(unused_variables)]
        let RefundInfo {
            gas_refund: _,
            gas_used,
            evm_refund,
            native_used,
        } = Self::compute_gas_refund(
            system,
            to_charge_for_pubdata,
            gas_limit,
            native_per_gas,
            &mut resources,
        )?;

        // Mint fee to bootloader
        // We already checked that total_gas_refund <= gas_limit
        let pay_to_operator = U256::from(gas_used)
            .checked_mul(U256::from(gas_price))
            .ok_or(internal_error!("gu*gp"))?;
        let mut inf_resources = S::Resources::FORMAL_INFINITE;

        Self::mint_token(
            system,
            &pay_to_operator,
            &BOOTLOADER_FORMAL_ADDRESS,
            &mut inf_resources,
        )
        .map_err(|e| match e.root_cause() {
            RootCause::Runtime(RuntimeError::OutOfErgs(_)) => {
                internal_error!("Out of ergs on infinite ergs").into()
            }
            RootCause::Runtime(RuntimeError::FatalRuntimeError(_)) => {
                internal_error!("Out of native on infinite").into()
            }
            _ => e,
        })?;

        // Refund
        let to_refund_recipient = match result {
            ExecutionResult::Revert { .. } => {
                // Upgrade transactions must always succeed
                if !is_priority_op {
                    return Err(internal_error!("Upgrade transaction must succeed").into());
                }
                // If the transaction reverts, then minting the msg.value to the
                // user has been reverted as well, so we can simply mint everything
                // that the user has deposited to the refund recipient
                total_deposited
                    .checked_sub(pay_to_operator)
                    .ok_or(internal_error!("td-pto"))
            }
            ExecutionResult::Success { .. } => {
                // If the transaction succeeds, then it is assumed that msg.value
                // was transferred correctly.
                // However, the remaining value deposited will be given to
                // the refund recipient.
                let value_plus_fee = value
                    .checked_add(pay_to_operator)
                    .ok_or(internal_error!("v+pto"))?;
                total_deposited
                    .checked_sub(value_plus_fee)
                    .ok_or(internal_error!("td-vpf"))
            }
        }?;
        if to_refund_recipient > U256::ZERO {
            let refund_recipient = u256_to_b160_checked(transaction.reserved[1].read());
            Self::mint_token(
                system,
                &to_refund_recipient,
                &refund_recipient,
                &mut inf_resources,
            )
            .map_err(|e| -> BootloaderSubsystemError {
                match e.root_cause() {
                    RootCause::Runtime(RuntimeError::OutOfErgs(_)) => {
                        internal_error!("Out of ergs on infinite ergs").into()
                    }
                    RootCause::Runtime(RuntimeError::FatalRuntimeError(_)) => {
                        internal_error!("Out of native on infinite").into()
                    }
                    _ => e,
                }
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
            is_priority_op,
        )?;

        // Add back the intrinsic native charged in get_resources_for_tx,
        // as initial_resources doesn't include them.
        let computational_native_used = resources_before_refund
            .diff(initial_resources)
            .native()
            .as_u64()
            + intrinsic_computational_native_charged;

        Ok(TxProcessingResult {
            result,
            tx_hash,
            is_l1_tx: is_priority_op,
            is_upgrade_tx: !is_priority_op,
            gas_used,
            gas_refunded: evm_refund,
            computational_native_used,
            native_used,
            pubdata_used: pubdata_used + L1_TX_INTRINSIC_PUBDATA,
        })
    }

    // Returns (execution_result, pubdata_used, to_charge_for_pubdata, resources_before_refund)
    fn execute_l1_transaction_and_notify_result<'a>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        memories: RunnerMemoryBuffers<'a>,
        transaction: &ZkSyncTransaction,
        from: B160,
        to: B160,
        value: U256,
        native_per_pubdata: U256,
        resources: &mut S::Resources,
        withheld_resources: S::Resources,
        tracer: &mut impl Tracer<S>,
    ) -> Result<(ExecutionResult<'a>, u64, S::Resources, S::Resources), BootloaderSubsystemError>
    {
        let _ = system
            .get_logger()
            .write_fmt(format_args!("Executing L1 transaction\n"));

        let gas_price = U256::from(transaction.max_fee_per_gas.read());
        system.set_tx_context(from, gas_price);

        // Start a frame, to revert minting of value if execution fails
        let rollback_handle = system.start_global_frame()?;

        // First we mint value
        if value > U256::ZERO {
            resources
                .with_infinite_ergs(|inf_resources| {
                    Self::mint_token(system, &value, &from, inf_resources)
                })
                .map_err(|e| match e.root_cause() {
                    RootCause::Runtime(RuntimeError::OutOfErgs(_)) => {
                        let _ = system.get_logger().write_fmt(format_args!(
                            "Out of ergs on infinite ergs: inner error was {e:?}"
                        ));
                        BootloaderSubsystemError::LeafDefect(internal_error!(
                            "Out of ergs on infinite ergs"
                        ))
                    }
                    _ => e,
                })?;
        }

        let resources_for_tx = resources.clone();

        // transaction is in managed region, so we can recast it back
        let calldata = transaction.calldata();

        // TODO: add support for deployment transactions,
        // probably unify with execution logic for EOA

        let CompletedExecution {
            resources_returned,
            result,
        } = Self::run_single_interaction(
            system,
            system_functions,
            memories,
            calldata,
            &from,
            &to,
            resources_for_tx,
            &value,
            false,
            tracer,
        )?;
        let reverted = result.failed();
        let return_values = result.return_values();

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

        // Just used for computing native used
        // Needs to use the resources before we reclaim withheld
        let resources_before_refund = resources.clone();

        // After the transaction is executed, we reclaim the withheld resources.
        // This is needed to ensure correct "gas_used" calculation, also these
        // resources could be spent for pubdata.
        resources.reclaim_withheld(withheld_resources);

        let (enough, to_charge_for_pubdata, pubdata_used) =
            check_enough_resources_for_pubdata(system, native_per_pubdata, resources, None)?;
        let execution_result = if !enough {
            let _ = system
                .get_logger()
                .write_fmt(format_args!("Not enough gas for pubdata after execution\n"));
            execution_result.reverted()
        } else {
            execution_result
        };

        Ok((
            execution_result,
            pubdata_used,
            to_charge_for_pubdata,
            resources_before_refund,
        ))
    }

    fn process_l2_transaction<'a, Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        mut memories: RunnerMemoryBuffers<'a>,
        mut transaction: ZkSyncTransaction,
        tracer: &mut impl Tracer<S>,
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

        let pubdata_price = system.get_pubdata_price();
        let native_price = system.get_native_price();
        let gas_price = Self::get_gas_price(
            system,
            transaction.max_fee_per_gas.read(),
            transaction.max_priority_fee_per_gas.read(),
        )?;
        if native_price.is_zero() {
            return Err(internal_error!("Native price cannot be 0").into());
        };
        let native_per_gas = if cfg!(feature = "resources_for_tester") {
            U256::from(crate::bootloader::constants::TESTER_NATIVE_PER_GAS)
        } else if Config::SIMULATION {
            SIMULATION_NATIVE_PER_GAS
        } else {
            U256::from(gas_price).div_ceil(native_price)
        };
        // We checked native_price != 0 above
        let native_per_pubdata = pubdata_price.wrapping_div(native_price);

        let ResourcesForTx {
            main_resources: mut resources,
            withheld: withheld_resources,
            intrinsic_computational_native_charged,
        } = get_resources_for_tx::<S>(
            gas_limit,
            native_per_pubdata,
            native_per_gas,
            calldata,
            L2_TX_INTRINSIC_GAS,
            L2_TX_INTRINSIC_PUBDATA,
            L2_TX_INTRINSIC_NATIVE_COST,
            false,
        )?;
        // Just used for computing native used
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
                        .with_unpadded_code_len()
                        .with_is_delegated(),
                )
            })?;

            (
                account_data.is_contract(),
                ExecutionEnvironmentType::parse_ee_version_byte(account_data.ee_version.0)?,
                account_data.nonce.0,
            )
        };

        F::charge_additional_intrinsic_gas(&mut resources, &transaction)?;

        system.set_tx_context(from, gas_price);

        let chain_id = system.get_chain_id();

        // Process access list
        transaction.parse_and_warm_up_access_list(system, &mut resources)?;

        let tx_hash: Bytes32 = transaction.calculate_hash(chain_id, &mut resources)?.into();
        let suggested_signed_hash: Bytes32 = transaction
            .calculate_signed_hash(chain_id, &mut resources)?
            .into();

        let ValidationResult { validation_pubdata } = Self::transaction_validation::<Config>(
            system,
            system_functions,
            memories.reborrow(),
            tx_hash,
            suggested_signed_hash,
            &mut transaction,
            from,
            gas_price,
            native_per_pubdata,
            caller_ee_type,
            caller_is_code,
            caller_nonce,
            &mut resources,
            tracer,
        )?;

        // Parse, validate and apply authorization list, following EIP-7702
        #[cfg(feature = "pectra")]
        transaction.parse_authorization_list_and_apply_delegations(system, &mut resources)?;

        // Take a snapshot in case we need to revert due to out of native.
        let rollback_handle = system.start_global_frame()?;

        // pubdata_info = (pubdata_used, to_charge_for_pubdata) can be cached
        // to used in the refund step only if the execution succeeded.
        // Otherwise, this value needs to be recomputed after reverting
        // state changes.
        let (execution_result, pubdata_info) = match Self::transaction_execution(
            system,
            system_functions,
            memories,
            tx_hash,
            suggested_signed_hash,
            &mut transaction,
            native_per_pubdata,
            validation_pubdata,
            caller_nonce,
            &mut resources,
            tracer,
            withheld_resources.clone(),
        ) {
            Ok((r, pubdata_used, to_charge_for_pubdata)) => {
                let pubdata_info = match r {
                    ExecutionResult::Success { .. } => {
                        system.finish_global_frame(None)?;
                        Some((pubdata_used, to_charge_for_pubdata))
                    }
                    ExecutionResult::Revert { .. } => {
                        system.finish_global_frame(Some(&rollback_handle))?;
                        None
                    }
                };
                (r, pubdata_info)
            }
            // Out of native is converted to a top-level revert and
            // gas is exhausted.
            Err(e) => match e.root_cause() {
                RootCause::Runtime(e @ RuntimeError::FatalRuntimeError(_)) => {
                    let _ = system.get_logger().write_fmt(format_args!(
                        "Transaction ran out of native resources or memory: {e:?}\n"
                    ));
                    resources.exhaust_ergs();
                    system.finish_global_frame(Some(&rollback_handle))?;
                    (ExecutionResult::Revert { output: &[] }, None)
                }
                _ => return Err(e.into()),
            },
        };

        // Just used for computing native used
        let resources_before_refund = resources.clone();
        // Now we can actually reclaim resources withheld for pubdata
        resources.reclaim_withheld(withheld_resources);

        let (
            RefundInfo {
                gas_refund: _,
                gas_used,
                evm_refund,
                native_used,
            },
            pubdata_used,
        ) = Self::refund_transaction(
            system,
            system_functions,
            tx_hash,
            suggested_signed_hash,
            &mut transaction,
            from,
            &execution_result,
            gas_price,
            native_per_gas,
            native_per_pubdata,
            validation_pubdata,
            caller_ee_type,
            &mut resources,
            pubdata_info,
        )?;

        // Add back the intrinsic native charged in get_resources_for_tx,
        // as initial_resources doesn't include them.
        let computational_native_used = resources_before_refund
            .diff(initial_resources)
            .native()
            .as_u64()
            + intrinsic_computational_native_charged;

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
            format!("Spent native for [process_transaction]: {computational_native_used}").as_str(),
        );

        Ok(TxProcessingResult {
            result: execution_result,
            tx_hash,
            is_l1_tx: false,
            is_upgrade_tx: false,
            gas_used,
            gas_refunded: evm_refund,
            computational_native_used,
            native_used,
            pubdata_used: pubdata_used + L2_TX_INTRINSIC_PUBDATA,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn transaction_validation<Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        mut memories: RunnerMemoryBuffers,
        tx_hash: Bytes32,
        suggested_signed_hash: Bytes32,
        transaction: &mut ZkSyncTransaction,
        from: B160,
        gas_price: U256,
        native_per_pubdata: U256,
        caller_ee_type: ExecutionEnvironmentType,
        caller_is_code: bool,
        caller_nonce: u64,
        resources: &mut S::Resources,
        tracer: &mut impl Tracer<S>,
    ) -> Result<ValidationResult, TxError> {
        let _ = system
            .get_logger()
            .write_fmt(format_args!("Start of validation\n"));

        // Nonce validation
        let tx_nonce = u256_try_to_u64(&transaction.nonce.read()).ok_or(TxError::from(
            InvalidTransaction::NonceOverflowInTransaction,
        ))?;

        if !Config::SIMULATION {
            F::check_nonce_is_not_used(caller_nonce, tx_nonce)?;
        }

        // validation
        F::validate::<Config>(
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
            tracer,
        )?;

        // Check nonce has been marked
        if !Config::SIMULATION {
            F::check_nonce_is_used_after_validation(
                system,
                caller_ee_type,
                resources,
                tx_nonce,
                from,
            )?;
        }

        let _ = system.get_logger().write_fmt(format_args!(
            "Transaction was validated, can collect fees\n"
        ));

        // Charge fees
        Self::ensure_payment(
            system,
            system_functions,
            tx_hash,
            suggested_signed_hash,
            transaction,
            from,
            gas_price,
            caller_ee_type,
            resources,
            tracer,
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

    // Returns (execution_result, pubdata_used, to_charge_for_pubdata)
    #[allow(clippy::too_many_arguments)]
    fn transaction_execution<'a>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        memories: RunnerMemoryBuffers<'a>,
        tx_hash: Bytes32,
        suggested_signed_hash: Bytes32,
        transaction: &mut ZkSyncTransaction,
        native_per_pubdata: U256,
        validation_pubdata: u64,
        current_tx_nonce: u64,
        resources: &mut S::Resources,
        tracer: &mut impl Tracer<S>,
        withheld_resources: S::Resources,
    ) -> Result<(ExecutionResult<'a>, u64, S::Resources), BootloaderSubsystemError> {
        let _ = system
            .get_logger()
            .write_fmt(format_args!("Start of execution\n"));

        // TODO: factory deps? Probably fine to ignore for now

        // execution
        let execution_result = F::execute(
            system,
            system_functions,
            memories,
            tx_hash,
            suggested_signed_hash,
            transaction,
            current_tx_nonce,
            resources,
            tracer,
        )?;

        let _ = system
            .get_logger()
            .write_fmt(format_args!("Transaction execution completed\n"));

        // After the transaction is executed, we reclaim the withheld resources.
        // This is needed to ensure correct "gas_used" calculation, also these
        // resources could be spent for pubdata.
        // We do not reclaim it to the actual `resources` yet, as that would make
        // the calculation of computational native used more complicated.
        let mut resources_for_check = resources.clone();
        resources_for_check.reclaim_withheld(withheld_resources);

        let (has_enough, to_charge_for_pubdata, pubdata_used) = check_enough_resources_for_pubdata(
            system,
            native_per_pubdata,
            &resources_for_check,
            Some(validation_pubdata),
        )?;
        if !has_enough {
            let _ = system
                .get_logger()
                .write_fmt(format_args!("Not enough gas for pubdata after execution\n"));
            Ok((
                execution_result.reverted(),
                pubdata_used,
                to_charge_for_pubdata,
            ))
        } else {
            Ok((execution_result, pubdata_used, to_charge_for_pubdata))
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn ensure_payment(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        tx_hash: Bytes32,
        suggested_signed_hash: Bytes32,
        transaction: &mut ZkSyncTransaction,
        from: B160,
        gas_price: U256,
        caller_ee_type: ExecutionEnvironmentType,
        resources: &mut S::Resources,
        tracer: &mut impl Tracer<S>,
    ) -> Result<(), TxError> {
        // Bootloader balance before fee payment
        let bootloader_balance_before = resources.with_infinite_ergs(|inf_resources| {
            system.io.get_nominal_token_balance(
                ExecutionEnvironmentType::NoEE,
                inf_resources,
                &BOOTLOADER_FORMAL_ADDRESS,
            )
        })?;
        let required_funds = gas_price
            .checked_mul(U256::from(transaction.gas_limit.read()))
            .ok_or(internal_error!("gp*gl"))?;
        // First we charge the fees, then we verify the bootloader got
        // the funds.
        let payer = {
            F::pay_for_transaction(
                system,
                system_functions,
                tx_hash,
                suggested_signed_hash,
                transaction,
                from,
                caller_ee_type,
                resources,
                tracer,
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
            .checked_sub(bootloader_balance_before)
            .ok_or(internal_error!("bba-bbb"))?;
        // If the amount of funds provided to the bootloader is less than the minimum required one
        // then this transaction should be rejected.
        require!(
            bootloader_received_funds >= required_funds,
            InvalidTransaction::ReceivedInsufficientFees {
                received: bootloader_received_funds,
                required: required_funds
            },
            system
        )?;
        let excessive_funds = bootloader_received_funds
            .checked_sub(required_funds)
            .ok_or(internal_error!("brf-rf"))?;
        if excessive_funds > U256::ZERO {
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
                .map_err(|e| TxError::Internal(wrap_error!(e)))?;
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
            core::cmp::min(max_priority_fee_per_gas, max_fee_per_gas - base_fee)
        } else {
            U256::ZERO
        };
        Ok(base_fee + priority_fee_per_gas)
    }

    // Returns (refund_info, total_pubdata_used)
    #[allow(clippy::too_many_arguments)]
    fn refund_transaction(
        system: &mut System<S>,
        _system_functions: &mut HooksStorage<S, S::Allocator>,
        _tx_hash: Bytes32,
        _suggested_signed_hash: Bytes32,
        transaction: &mut ZkSyncTransaction,
        from: B160,
        execution_result: &ExecutionResult,
        gas_price: U256,
        native_per_gas: U256,
        native_per_pubdata: U256,
        validation_pubdata: u64,
        caller_ee_type: ExecutionEnvironmentType,
        resources: &mut S::Resources,
        pubdata_info: Option<(u64, S::Resources)>,
    ) -> Result<(RefundInfo, u64), BootloaderSubsystemError> {
        let _ = system
            .get_logger()
            .write_fmt(format_args!("Start of refund\n"));
        let _success = matches!(execution_result, ExecutionResult::Success { .. });
        let _max_refunded_gas = resources.ergs().0.div_floor(ERGS_PER_GAS);
        let refund_recipient = from;

        // TODO: consider operator refund

        // Pubdata for validation has been charged already,
        // we charge for the rest now.
        let (total_pubdata_used, to_charge_for_pubdata) = match pubdata_info {
            Some((net_execution_pubdata, to_charge)) => {
                (net_execution_pubdata + validation_pubdata, to_charge)
            }
            None => {
                let (execution_pubdata_spent, to_charge_for_pubdata) =
                    get_resources_to_charge_for_pubdata(
                        system,
                        native_per_pubdata,
                        Some(validation_pubdata),
                    )?;
                (
                    execution_pubdata_spent + validation_pubdata,
                    to_charge_for_pubdata,
                )
            }
        };
        let refund_info = Self::compute_gas_refund(
            system,
            to_charge_for_pubdata,
            transaction.gas_limit.read(),
            native_per_gas,
            resources,
        )?;
        let token_to_refund = refund_info
            .gas_refund
            .checked_mul(gas_price)
            .ok_or(internal_error!("tgf*gp"))?;
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
                // Balance errors can not be cascaded
                SubsystemError::Cascaded(CascadedError(inner, _)) => match inner {},
                SubsystemError::LeafUsage(InterfaceError(ie, _)) => match ie {
                    BalanceError::InsufficientBalance => {
                        interface_error!(BootloaderInterfaceError::CantPayRefundInsufficientBalance)
                    }
                    BalanceError::Overflow => {
                        interface_error!(BootloaderInterfaceError::CantPayRefundOverflow)
                    }
                },
                other => wrap_error!(other),
            })?;
        Ok((refund_info, total_pubdata_used))
    }

    fn compute_gas_refund(
        system: &mut System<S>,
        to_charge_for_pubdata: S::Resources,
        gas_limit: u64,
        native_per_gas: U256,
        resources: &mut S::Resources,
    ) -> Result<RefundInfo, InternalError> {
        // Already checked
        resources.charge_unchecked(&to_charge_for_pubdata);

        let mut gas_used = gas_limit - resources.ergs().0.div_floor(ERGS_PER_GAS);
        resources.exhaust_ergs();

        // Following EIP-3529, refunds are capped to 1/5 of the gas used
        #[cfg(feature = "evm_refunds")]
        let evm_refund = {
            let full_refund = system.io.get_refund_counter() as u64;
            let max_refund = gas_used / 5;
            core::cmp::min(full_refund, max_refund)
        };

        #[cfg(not(feature = "evm_refunds"))]
        let evm_refund = 0;

        gas_used -= evm_refund;

        let full_native_limit = if cfg!(feature = "unlimited_native") {
            u64::MAX
        } else {
            gas_limit.saturating_mul(u256_to_u64_saturated(&native_per_gas))
        };
        let native_used = full_native_limit.saturating_sub(resources.native().remaining().as_u64());

        #[cfg(not(feature = "unlimited_native"))]
        {
            // Adjust gas_used with difference with used native
            let native_per_gas = u256_to_u64_saturated(&native_per_gas);

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
            .write_fmt(format_args!("Gas refund: {total_gas_refund}\n"));
        require_internal!(
            total_gas_refund <= gas_limit,
            "Gas refund greater than gas limit",
            system
        )?;
        let total_gas_refund = U256::from(total_gas_refund);
        let refund_info = RefundInfo {
            gas_refund: total_gas_refund,
            gas_used,
            evm_refund,
            native_used,
        };
        Ok(refund_info)
    }
}

struct RefundInfo {
    // Amount of gas to be returned to user
    gas_refund: U256,
    // EVM gas used by the transaction
    gas_used: u64,
    // EVM-specific refund
    evm_refund: u64,
    // Total native resource used by the transaction (includes pubdata)
    native_used: u64,
}
