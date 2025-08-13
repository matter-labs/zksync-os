use crate::bootloader::account_models::{AccountModel, ExecutionOutput, ExecutionResult};
use crate::bootloader::constants::ERC20_APPROVE_SELECTOR;
use crate::bootloader::constants::PAYMASTER_APPROVAL_BASED_SELECTOR;
use crate::bootloader::constants::PAYMASTER_GENERAL_SELECTOR;
use crate::bootloader::constants::{DEPLOYMENT_TX_EXTRA_INTRINSIC_GAS, ERC20_ALLOWANCE_SELECTOR};
use crate::bootloader::constants::{SPECIAL_ADDRESS_TO_WASM_DEPLOY, TX_OFFSET};
use crate::bootloader::errors::InvalidTransaction::CreateInitCodeSizeLimit;
use crate::bootloader::errors::{AAMethod, BootloaderSubsystemError};
use crate::bootloader::errors::{InvalidTransaction, TxError};
use crate::bootloader::runner::{run_till_completion, RunnerMemoryBuffers};
use crate::bootloader::supported_ees::SystemBoundEVMInterpreter;
use crate::bootloader::transaction::ZkSyncTransaction;
use crate::bootloader::BasicBootloaderExecutionConfig;
use crate::bootloader::{BasicBootloader, Bytes32};
use core::fmt::Write;
use crypto::secp256k1::SECP256K1N_HALF;
use evm_interpreter::{ERGS_PER_GAS, MAX_INITCODE_SIZE};
use ruint::aliases::{B160, U256};
use system_hooks::addresses_constants::BOOTLOADER_FORMAL_ADDRESS;
use system_hooks::HooksStorage;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::memory::ArrayBuilder;
use zk_ee::system::errors::interface::InterfaceError;
use zk_ee::system::errors::subsystem::SubsystemError;
use zk_ee::system::tracer::Tracer;
use zk_ee::system::{
    errors::{runtime::RuntimeError, system::SystemError},
    logger::Logger,
    EthereumLikeTypes, System, SystemTypes, *,
};
use zk_ee::utils::{b160_to_u256, u256_to_b160_checked};
use zk_ee::{internal_error, out_of_native_resources, wrap_error};

macro_rules! require_or_revert {
    ($b:expr, $m:expr, $s:expr, $system:expr) => {
        if $b {
            Ok(())
        } else {
            let _ = $system
                .get_logger()
                .write_fmt(format_args!("Reverted: {}\n", $s));
            Err(TxError::Validation(InvalidTransaction::Revert {
                method: $m,
                output: None,
            }))
        }
    };
}

pub struct EOA;

impl<S: EthereumLikeTypes> AccountModel<S> for EOA
where
    S::IO: IOSubsystemExt,
{
    fn validate<Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        _system_functions: &mut HooksStorage<S, S::Allocator>,
        _memories: RunnerMemoryBuffers,
        _tx_hash: Bytes32,
        suggested_signed_hash: Bytes32,
        transaction: &mut ZkSyncTransaction,
        caller_ee_type: ExecutionEnvironmentType,
        caller_is_code: bool,
        caller_nonce: u64,
        resources: &mut S::Resources,
        _tracer: &mut impl Tracer<S>,
    ) -> Result<(), TxError> {
        // safe to panic, validated by the structure
        let from = transaction.from.read();

        // EIP-3607: Reject transactions from senders with deployed code
        if caller_is_code {
            return Err(InvalidTransaction::RejectCallerWithCode.into());
        }

        // Balance check
        let total_required_balance = transaction.required_balance()?;

        match system
            .io
            .get_nominal_token_balance(caller_ee_type, resources, &from)
        {
            Ok(balance) => {
                if total_required_balance > balance {
                    return Err(TxError::Validation(
                        InvalidTransaction::LackOfFundForMaxFee {
                            fee: total_required_balance,
                            balance,
                        },
                    ));
                }
            }
            Err(SystemError::LeafRuntime(RuntimeError::OutOfErgs(_))) => {
                return Err(TxError::Validation(
                    InvalidTransaction::OutOfGasDuringValidation,
                ))
            }
            Err(SystemError::LeafRuntime(RuntimeError::OutOfNativeResources(_))) => {
                return Err(TxError::Validation(
                    InvalidTransaction::OutOfNativeResourcesDuringValidation,
                ))
            }
            Err(SystemError::LeafDefect(e)) => return Err(TxError::Internal(e.into())),
        }

        let signature = transaction.signature();
        let r = &signature[..32];
        let s = &signature[32..64];
        let v = &signature[64];
        if !Config::ONLY_SIMULATE && U256::from_be_slice(s) > U256::from_be_bytes(SECP256K1N_HALF) {
            return Err(InvalidTransaction::MalleableSignature.into());
        }

        let mut ecrecover_input = [0u8; 128];
        ecrecover_input[0..32].copy_from_slice(suggested_signed_hash.as_u8_array_ref());
        ecrecover_input[63] = *v;
        ecrecover_input[64..96].copy_from_slice(r);
        ecrecover_input[96..128].copy_from_slice(s);

        let mut ecrecover_output = ArrayBuilder::default();
        S::SystemFunctions::secp256k1_ec_recover(
            ecrecover_input.as_slice(),
            &mut ecrecover_output,
            resources,
            system.get_allocator(),
        )
        .map_err(SystemError::from)?;

        if !Config::ONLY_SIMULATE {
            if ecrecover_output.is_empty() {
                return Err(InvalidTransaction::IncorrectFrom {
                    recovered: B160::ZERO,
                    tx: from,
                }
                .into());
            }

            let recovered_from = B160::try_from_be_slice(&ecrecover_output.build()[12..])
                .ok_or(internal_error!("Invalid ecrecover return value"))?;

            if recovered_from != from {
                return Err(InvalidTransaction::IncorrectFrom {
                    recovered: recovered_from,
                    tx: from,
                }
                .into());
            }
        }

        let old_nonce = match system
            .io
            .increment_nonce(caller_ee_type, resources, &from, 1u64)
        {
            Ok(x) => Ok(x),
            Err(SubsystemError::LeafUsage(InterfaceError(NonceError::NonceOverflow, _))) => {
                return Err(TxError::Validation(
                    InvalidTransaction::NonceOverflowInTransaction,
                ))
            }
            Err(e) => Err(wrap_error!(e)),
        }?;

        assert_eq!(caller_nonce, old_nonce);

        Ok(())
    }

    fn execute<'a>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        memories: RunnerMemoryBuffers<'a>,
        _tx_hash: Bytes32,
        _suggested_signed_hash: Bytes32,
        transaction: &mut ZkSyncTransaction,
        // This data is read before bumping nonce
        current_tx_nonce: u64,
        resources: &mut S::Resources,
        tracer: &mut impl Tracer<S>,
    ) -> Result<ExecutionResult<'a>, BootloaderSubsystemError> {
        // panic is not reachable, validated by the structure
        let from = transaction.from.read();

        let main_calldata = transaction.calldata();

        // panic is not reachable, to is validated
        let to = transaction.to.read();

        let nominal_token_value = transaction.value.read();

        let to_ee_type = if !transaction.reserved[1].read().is_zero() {
            Some(ExecutionEnvironmentType::EVM)
        } else {
            None
        };

        let TxExecutionResult {
            return_values,
            resources_returned,
            reverted,
            deployed_address,
        } = match to_ee_type {
            Some(to_ee_type) => process_deployment(
                system,
                system_functions,
                memories,
                resources,
                to_ee_type,
                main_calldata,
                from,
                nominal_token_value,
                current_tx_nonce,
                tracer,
            )?,
            None => {
                let final_state = BasicBootloader::run_single_interaction(
                    system,
                    system_functions,
                    memories,
                    main_calldata,
                    &from,
                    &to,
                    resources.clone(),
                    &nominal_token_value,
                    true,
                    tracer,
                )?;

                let CompletedExecution {
                    return_values,
                    resources_returned,
                    reverted,
                    ..
                } = final_state;

                TxExecutionResult {
                    return_values,
                    resources_returned,
                    reverted,
                    deployed_address: DeployedAddress::CallNoAddress,
                }
            }
        };

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

        let result = match reverted {
            true => ExecutionResult::Revert {
                output: returndata_region,
            },
            false => {
                // Safe to do so by construction.
                match deployed_address {
                    DeployedAddress::Address(at) => ExecutionResult::Success {
                        output: ExecutionOutput::Create(returndata_region, at),
                    },
                    _ => ExecutionResult::Success {
                        output: ExecutionOutput::Call(returndata_region),
                    },
                }
            }
        };
        Ok(result)
    }

    ///
    /// EOA requires tx_nonce == account nonce
    ///
    fn check_nonce_is_not_used(account_data_nonce: u64, tx_nonce: u64) -> Result<(), TxError> {
        if tx_nonce > account_data_nonce {
            return Err(InvalidTransaction::NonceTooHigh {
                tx: tx_nonce,
                state: account_data_nonce,
            }
            .into());
        }
        if tx_nonce < account_data_nonce {
            return Err(InvalidTransaction::NonceTooLow {
                tx: tx_nonce,
                state: account_data_nonce,
            }
            .into());
        }
        Ok(())
    }

    fn check_nonce_is_used_after_validation(
        _system: &mut System<S>,
        _caller_ee_type: ExecutionEnvironmentType,
        _resources: &mut S::Resources,
        _tx_nonce: u64,
        _from: B160,
    ) -> Result<(), TxError> {
        // The bootloader increments the account for EOA, no check
        // is needed
        Ok(())
    }

    fn pay_for_transaction(
        system: &mut System<S>,
        _system_functions: &mut HooksStorage<S, S::Allocator>,
        _memories: RunnerMemoryBuffers,
        _tx_hash: Bytes32,
        _suggested_signed_hash: Bytes32,
        transaction: &mut ZkSyncTransaction,
        from: B160,
        caller_ee_type: ExecutionEnvironmentType,
        resources: &mut S::Resources,
        _tracer: &mut impl Tracer<S>,
    ) -> Result<(), TxError> {
        let amount = transaction
            .max_fee_per_gas
            .read()
            .checked_mul(transaction.gas_limit.read() as u128)
            .ok_or(internal_error!("mfpg*gl"))?;
        let amount = U256::from(amount);
        system
            .io
            .transfer_nominal_token_value(
                caller_ee_type,
                resources,
                &from,
                &BOOTLOADER_FORMAL_ADDRESS,
                &amount,
            )
            .map_err(|e| match e {
                SubsystemError::LeafUsage(interface_error) => {
                    let _ = system
                        .get_logger()
                        .write_fmt(format_args!("{interface_error:?}"));
                    match system
                        .io
                        .get_nominal_token_balance(caller_ee_type, resources, &from)
                    {
                        Ok(balance) => {
                            TxError::Validation(InvalidTransaction::LackOfFundForMaxFee {
                                fee: amount,
                                balance,
                            })
                        }
                        Err(e) => e.into(),
                    }
                }
                SubsystemError::LeafDefect(internal_error) => internal_error.into(),
                SubsystemError::LeafRuntime(runtime_error) => match runtime_error {
                    RuntimeError::OutOfNativeResources(_) => {
                        TxError::oon_as_validation(out_of_native_resources!().into())
                    }
                    RuntimeError::OutOfErgs(_) => {
                        TxError::Validation(InvalidTransaction::OutOfGasDuringValidation)
                    }
                },
                SubsystemError::Cascaded(cascaded_error) => match cascaded_error {},
            })?;
        Ok(())
    }

    fn pre_paymaster(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        mut memories: RunnerMemoryBuffers,
        _tx_hash: Bytes32,
        _suggested_signed_hash: Bytes32,
        transaction: &mut ZkSyncTransaction,
        from: B160,
        paymaster: B160,
        _caller_ee_type: ExecutionEnvironmentType,
        resources: &mut S::Resources,
        tracer: &mut impl Tracer<S>,
    ) -> Result<(), TxError> {
        let paymaster_input = transaction.paymaster_input();
        require_or_revert!(
            paymaster_input.len() >= 4,
            AAMethod::AccountPrePaymaster,
            "The standard paymaster input must be at least 4 bytes long",
            system
        )?;
        if paymaster_input.starts_with(PAYMASTER_APPROVAL_BASED_SELECTOR) {
            require_or_revert!(
                paymaster_input.len() >= 68,
                AAMethod::AccountPrePaymaster,
                "The approvalBased paymaster input must be at least 68 bytes long",
                system
            )?;
            let token_end = 4 + U256::BYTES;
            let token = U256::try_from_be_slice(&paymaster_input[4..token_end]);
            require_or_revert!(
                token.is_some(),
                AAMethod::AccountPrePaymaster,
                "invalid paymaster input",
                system
            )?;
            // Safe to unrwrap thanks to the previous check.
            let token = token.unwrap();
            let token = u256_to_b160_checked(token);
            let min_allowance_end = token_end + U256::BYTES;
            let min_allowance =
                U256::try_from_be_slice(&paymaster_input[token_end..min_allowance_end]);
            require_or_revert!(
                min_allowance.is_some(),
                AAMethod::AccountPrePaymaster,
                "invalid paymaster input",
                system
            )?;
            // Safe to unrwrap thanks to the previous check.
            let min_allowance = min_allowance.unwrap();

            let pre_tx_buffer = transaction.pre_tx_buffer();
            let current_allowance = erc20_allowance(
                system,
                system_functions,
                memories.reborrow(),
                pre_tx_buffer,
                from,
                paymaster,
                token,
                resources,
                tracer,
            )?;
            if current_allowance < min_allowance {
                // Some tokens, e.g. USDT require that the allowance is
                // firstly set to zero and only then updated to the new value.
                let success = erc20_approve(
                    system,
                    system_functions,
                    memories.reborrow(),
                    pre_tx_buffer,
                    from,
                    paymaster,
                    token,
                    U256::ZERO,
                    resources,
                    tracer,
                )?;
                require_or_revert!(
                    success == U256::from(1),
                    AAMethod::AccountPrePaymaster,
                    "ERC20 0 approve failed",
                    system
                )?;
                let success = erc20_approve(
                    system,
                    system_functions,
                    memories,
                    pre_tx_buffer,
                    from,
                    paymaster,
                    token,
                    min_allowance,
                    resources,
                    tracer,
                )?;
                require_or_revert!(
                    success == U256::from(1),
                    AAMethod::AccountPrePaymaster,
                    "ERC20 min_allowance approve failed",
                    system
                )
            } else {
                Ok(())
            }
        } else if paymaster_input.starts_with(PAYMASTER_GENERAL_SELECTOR) {
            // Do nothing. general(bytes) paymaster flow means that the paymaster must interpret these bytes on his own.
            Ok(())
        } else {
            require_or_revert!(
                false,
                AAMethod::AccountPrePaymaster,
                "Unsupported paymaster flow",
                system
            )
        }
    }

    fn charge_additional_intrinsic_gas(
        resources: &mut S::Resources,
        transaction: &ZkSyncTransaction,
    ) -> Result<(), TxError> {
        let to = transaction.to.read();
        let is_deployment =
            !transaction.reserved[1].read().is_zero() || to == SPECIAL_ADDRESS_TO_WASM_DEPLOY;
        if is_deployment {
            let calldata_len = transaction.calldata().len() as u64;
            if calldata_len > MAX_INITCODE_SIZE as u64 {
                return Err(TxError::Validation(CreateInitCodeSizeLimit));
            }
            let initcode_gas_cost = evm_interpreter::gas_constants::INITCODE_WORD_COST
                * (calldata_len.next_multiple_of(32) / 32)
                + DEPLOYMENT_TX_EXTRA_INTRINSIC_GAS;
            let ergs_to_spend = Ergs(initcode_gas_cost.saturating_mul(ERGS_PER_GAS));
            match resources.charge(&S::Resources::from_ergs(ergs_to_spend)) {
                Ok(_) => (),
                Err(SystemError::LeafRuntime(RuntimeError::OutOfErgs(_))) => {
                    return Err(TxError::Validation(
                        InvalidTransaction::OutOfGasDuringValidation,
                    ))
                }
                Err(e @ SystemError::LeafRuntime(RuntimeError::OutOfNativeResources(_))) => {
                    return Err(TxError::oon_as_validation(e.into()))
                }
                Err(SystemError::LeafDefect(e)) => return Err(TxError::Internal(e.into())),
            };
        }
        #[cfg(feature = "pectra")]
        {
            let authorization_list_length = transaction.parse_authorization_list_length()?;
            let authorization_list_gas_cost = authorization_list_length
                .saturating_mul(evm_interpreter::gas_constants::NEWACCOUNT);
            let ergs_to_spend = Ergs(authorization_list_gas_cost.saturating_mul(ERGS_PER_GAS));
            match resources.charge(&S::Resources::from_ergs(ergs_to_spend)) {
                Ok(_) => (),
                Err(SystemError::LeafRuntime(RuntimeError::OutOfErgs(_))) => {
                    return Err(TxError::Validation(
                        InvalidTransaction::OutOfGasDuringValidation,
                    ))
                }
                Err(e @ SystemError::LeafRuntime(RuntimeError::OutOfNativeResources(_))) => {
                    return Err(TxError::oon_as_validation(e.into()))
                }
                Err(SystemError::LeafDefect(e)) => return Err(TxError::Internal(e.into())),
            };
        }

        Ok(())
    }
}

// Address deployed, or reason for the lack thereof.
enum DeployedAddress {
    CallNoAddress,
    RevertedNoAddress,
    Address(B160),
}

struct TxExecutionResult<'a, S: SystemTypes> {
    return_values: ReturnValues<'a, S>,
    resources_returned: S::Resources,
    reverted: bool,
    deployed_address: DeployedAddress,
}

/// Run the deployment part of a contract creation tx
/// The boolean in the return
fn process_deployment<'a, S: EthereumLikeTypes>(
    system: &mut System<S>,
    system_functions: &mut HooksStorage<S, S::Allocator>,
    memories: RunnerMemoryBuffers<'a>,
    resources: &mut S::Resources,
    to_ee_type: ExecutionEnvironmentType,
    main_calldata: &[u8],
    from: B160,
    nominal_token_value: U256,
    existing_nonce: u64,
    tracer: &mut impl Tracer<S>,
) -> Result<TxExecutionResult<'a, S>, BootloaderSubsystemError>
where
    S::IO: IOSubsystemExt,
{
    // Next check max initcode size
    if main_calldata.len() > MAX_INITCODE_SIZE {
        return Ok(TxExecutionResult {
            return_values: ReturnValues::empty(),
            resources_returned: resources.clone(),
            reverted: true,
            deployed_address: DeployedAddress::RevertedNoAddress,
        });
    }
    let ee_specific_deployment_processing_data = match to_ee_type {
        ExecutionEnvironmentType::NoEE => {
            return Err(internal_error!("Deployment cannot target NoEE").into())
        }
        ExecutionEnvironmentType::EVM => {
            SystemBoundEVMInterpreter::<S>::default_ee_deployment_options(system)
        }
    };

    let deployment_parameters = DeploymentPreparationParameters {
        address_of_deployer: from,
        call_scratch_space: None,
        constructor_parameters: &[],
        nominal_token_value,
        deployment_code: main_calldata,
        ee_specific_deployment_processing_data,
        deployer_full_resources: resources.clone(),
        deployer_nonce: Some(existing_nonce),
    };
    let rollback_handle = system.start_global_frame()?;

    let final_state = run_till_completion(
        memories,
        system,
        system_functions,
        to_ee_type,
        ExecutionEnvironmentSpawnRequest::RequestedDeployment(deployment_parameters),
        tracer,
    )?;
    let TransactionEndPoint::CompletedDeployment(CompletedDeployment {
        resources_returned,
        deployment_result,
    }) = final_state
    else {
        return Err(internal_error!("attempt to deploy ended up in invalid state").into());
    };

    let (deployment_success, reverted, return_values, at) = match deployment_result {
        DeploymentResult::Successful {
            return_values,
            deployed_at,
            ..
        } => (true, false, return_values, Some(deployed_at)),
        DeploymentResult::Failed { return_values, .. } => (false, true, return_values, None),
    };
    // Do not forget to reassign it back after potential copy when finishing frame
    system.finish_global_frame(reverted.then_some(&rollback_handle))?;

    // TODO: debug implementation for Bits uses global alloc, which panics in ZKsync OS
    #[cfg(not(target_arch = "riscv32"))]
    let _ = system.get_logger().write_fmt(format_args!(
        "Deployment at {at:?} ended with success = {deployment_success}\n"
    ));
    let returndata_iter = return_values.returndata.iter().copied();
    let _ = system.get_logger().write_fmt(format_args!("Returndata = "));
    let _ = system.get_logger().log_data(returndata_iter);
    let deployed_address = at
        .map(DeployedAddress::Address)
        .unwrap_or(DeployedAddress::RevertedNoAddress);
    Ok(TxExecutionResult {
        return_values,
        resources_returned,
        reverted: !deployment_success,
        deployed_address,
    })
}

/// Call the ERC20 [allowance] method for [token]
#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
fn erc20_allowance<S: EthereumLikeTypes>(
    system: &mut System<S>,
    system_functions: &mut HooksStorage<S, S::Allocator>,
    memories: RunnerMemoryBuffers,
    pre_tx_buffer: &mut [u8],
    from: B160,
    paymaster: B160,
    token: B160,
    resources: &mut S::Resources,
    tracer: &mut impl Tracer<S>,
) -> Result<U256, TxError>
where
    S::IO: IOSubsystemExt,
{
    // Calldata:
    // selector (4)
    // owner (32)
    // spender (32)
    let calldata_length = 4 + U256::BYTES * 2;
    let calldata_start = TX_OFFSET - calldata_length;

    // Write selector
    pre_tx_buffer[calldata_start..(calldata_start + 4)].copy_from_slice(ERC20_ALLOWANCE_SELECTOR);
    // Write owner
    let owner_start = calldata_start + 4;
    pre_tx_buffer[owner_start..(owner_start + U256::BYTES)]
        .copy_from_slice(&b160_to_u256(from).to_be_bytes::<{ U256::BYTES }>());
    // Write spender
    let spender_start = owner_start + U256::BYTES;
    pre_tx_buffer[spender_start..(spender_start + U256::BYTES)]
        .copy_from_slice(&b160_to_u256(paymaster).to_be_bytes::<{ U256::BYTES }>());

    // we are static relative to everything that happens later
    let calldata = &pre_tx_buffer[calldata_start..(calldata_start + calldata_length)];

    let _ = system
        .get_logger()
        .write_fmt(format_args!("Calling ERC20 allowance\n"));

    let CompletedExecution {
        resources_returned,
        return_values,
        reverted,
        ..
    } = BasicBootloader::run_single_interaction(
        system,
        system_functions,
        memories,
        calldata,
        &from,
        &token,
        resources.clone(),
        &U256::ZERO,
        true,
        tracer,
    )
    .map_err(TxError::oon_as_validation)?;

    let returndata_region = return_values.returndata;
    let returndata_slice = &returndata_region;

    *resources = resources_returned;

    let res: Result<U256, TxError> = if reverted {
        Err(TxError::Validation(InvalidTransaction::Revert {
            method: AAMethod::AccountPrePaymaster,
            output: None, // TODO
        }))
    } else if returndata_slice.len() != 32 {
        Err(TxError::Validation(
            InvalidTransaction::InvalidReturndataLength,
        ))
    } else {
        Ok(U256::from_be_slice(returndata_slice))
    };

    res
}

/// Call the ERC20 [approve] method for [token]
#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
fn erc20_approve<S: EthereumLikeTypes>(
    system: &mut System<S>,
    system_functions: &mut HooksStorage<S, S::Allocator>,
    memories: RunnerMemoryBuffers,
    pre_tx_buffer: &mut [u8],
    from: B160,
    paymaster: B160,
    token: B160,
    amount: U256,
    resources: &mut S::Resources,
    tracer: &mut impl Tracer<S>,
) -> Result<U256, TxError>
where
    S::IO: IOSubsystemExt,
{
    // Calldata:
    // selector (4)
    // spender (32)
    // amount (32)
    let calldata_length = 4 + U256::BYTES * 2;
    let calldata_start = TX_OFFSET - calldata_length;

    // Write selector
    pre_tx_buffer[calldata_start..(calldata_start + 4)].copy_from_slice(ERC20_APPROVE_SELECTOR);
    // Write spender
    let spender_start = calldata_start + 4;
    pre_tx_buffer[spender_start..(spender_start + U256::BYTES)]
        .copy_from_slice(&b160_to_u256(paymaster).to_be_bytes::<{ U256::BYTES }>());
    // Write
    let amount_start = spender_start + U256::BYTES;
    pre_tx_buffer[amount_start..(amount_start + U256::BYTES)]
        .copy_from_slice(&amount.to_be_bytes::<{ U256::BYTES }>());

    // we are static relative to everything that happens later
    let calldata = &pre_tx_buffer[calldata_start..(calldata_start + calldata_length)];
    let _ = system
        .get_logger()
        .write_fmt(format_args!("Calling ERC20 approve\n"));

    let CompletedExecution {
        resources_returned,
        return_values,
        reverted,
        ..
    } = BasicBootloader::run_single_interaction(
        system,
        system_functions,
        memories,
        calldata,
        &from,
        &token,
        resources.clone(),
        &U256::ZERO,
        true,
        tracer,
    )
    .map_err(TxError::oon_as_validation)?;

    let returndata_region = return_values.returndata;
    let returndata_slice = &returndata_region;

    *resources = resources_returned;

    let res: Result<U256, TxError> = if reverted {
        Err(TxError::Validation(InvalidTransaction::Revert {
            method: AAMethod::AccountPrePaymaster,
            output: None, // TODO
        }))
    } else if returndata_slice.len() != 32 {
        Err(TxError::Validation(
            InvalidTransaction::InvalidReturndataLength,
        ))
    } else {
        Ok(U256::from_be_slice(returndata_slice))
    };

    res
}
