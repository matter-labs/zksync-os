use super::*;
use crate::bootloader::errors::InvalidTransaction;
use crate::bootloader::{transaction::ZkSyncTransaction, transaction_flow::BasicTransactionFlow};
use crate::bootloader::{BasicBootloader, TxDataBuffer};
use core::fmt::Write;
use ruint::aliases::B160;
use ruint::aliases::U256;
use zk_ee::internal_error;
use zk_ee::metadata_markers::basic_metadata::BasicMetadata;
use zk_ee::metadata_markers::basic_metadata::ZkSpecificPricingMetadata;
use zk_ee::out_of_native_resources;
use zk_ee::system::errors::root_cause::GetRootCause;
use zk_ee::system::errors::root_cause::RootCause;
use zk_ee::system::errors::runtime::RuntimeError;
use zk_ee::system::errors::subsystem::SubsystemError;
use zk_ee::system::logger::Logger;
use zk_ee::system::EthereumLikeTypes;
use zk_ee::system::*;
use zk_ee::types_config::EthereumIOTypesConfig;
use zk_ee::utils::Bytes32;

pub struct ZkTransactionFlowOnlyEOA<S: EthereumLikeTypes> {
    _marker: core::marker::PhantomData<S>,
}

#[derive(Debug)]
pub struct ZkTxResult<'a> {
    pub result: ExecutionResult<'a, EthereumIOTypesConfig>,
    pub tx_hash: Bytes32,
    pub is_l1_tx: bool,
    pub is_upgrade_tx: bool,
    pub gas_refunded: u64,
    pub gas_used: u64,
    pub computational_native_used: u64,
    pub pubdata_used: u64,
}

impl<'a> MinimalTransactionOutput<'a> for ZkTxResult<'a> {
    fn is_success(&self) -> bool {
        match &self.result {
            ExecutionResult::Success { .. } => true,
            ExecutionResult::Revert { .. } => false,
        }
    }
    fn returndata(&self) -> &[u8] {
        match &self.result {
            ExecutionResult::Success { output } => match output {
                ExecutionOutput::Call(returndata) => returndata,
                ExecutionOutput::Create(..) => &[],
            },
            ExecutionResult::Revert { output } => output,
        }
    }
    fn transaction_hash(&self) -> Bytes32 {
        self.tx_hash
    }
    fn into_bookkeeper_output(self) -> TxProcessingOutput<'a> {
        let (success, returndata, created_address) = match self.result {
            ExecutionResult::Success { output } => match output {
                ExecutionOutput::Call(returndata) => (true, returndata, None),
                ExecutionOutput::Create(returndata, address) => (true, returndata, Some(address)),
            },
            ExecutionResult::Revert { output } => (false, output, None),
        };

        TxProcessingOutput {
            status: success,
            output: returndata,
            contract_address: created_address,
            gas_used: self.gas_used,
            gas_refunded: self.gas_refunded,
            computational_native_used: self.computational_native_used,
            pubdata_used: self.pubdata_used,
        }
    }
}

use crate::bootloader::gas_helpers::ResourcesForTx;

mod validation_impl;

pub struct TxContextForPreAndPostProcessing<S: EthereumLikeTypes> {
    pub resources: ResourcesForTx<S>,
    pub tx_hash: Bytes32,
    pub fee_to_prepay: U256,
    pub gas_price_for_metadata: U256,
    pub gas_price_for_fee_commitment: U256,
    pub minimal_ergs_to_charge: Ergs,
    pub originator_nonce_to_use: u64,
    pub native_per_pubdata: u64,
    pub native_per_gas: u64,
    pub tx_gas_limit: u64,
    pub gas_used: u64,
    pub gas_refunded: u64,
    pub validation_pubdata: u64,
    pub total_pubdata: u64,
    pub initial_resources: S::Resources,
    pub resources_before_refund: S::Resources,
}

impl<S: EthereumLikeTypes> core::fmt::Debug for TxContextForPreAndPostProcessing<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TxContextForPreAndPostProcessing")
            .field("resources", &self.resources)
            .field("tx_hash", &self.tx_hash)
            .field("fee_to_prepay", &self.fee_to_prepay)
            .field("gas_price_for_metadata", &self.gas_price_for_metadata)
            .field(
                "gas_price_for_fee_commitment",
                &self.gas_price_for_fee_commitment,
            )
            .field("minimal_ergs_to_charge", &self.minimal_ergs_to_charge)
            .field("originator_nonce_to_use", &self.originator_nonce_to_use)
            .field("native_per_pubdata", &self.native_per_pubdata)
            .field("native_per_gas", &self.native_per_gas)
            .field("tx_gas_limit", &self.tx_gas_limit)
            .field("gas_used", &self.gas_used)
            .field("gas_refunded", &self.gas_used)
            .field("validation_pubdata", &self.validation_pubdata)
            .field("total_pubdata", &self.total_pubdata)
            .finish()
    }
}

impl<S: EthereumLikeTypes> BasicTransactionFlow<S> for ZkTransactionFlowOnlyEOA<S>
where
    S::IO: IOSubsystemExt,
    S::Metadata: ZkSpecificPricingMetadata,
    <S::Metadata as BasicMetadata<S::IOTypes>>::TransactionMetadata: From<(B160, U256)>,
{
    type Transaction<'a> = ZkSyncTransaction<'a>;
    type TransactionContext = TxContextForPreAndPostProcessing<S>;
    type ExecutionBodyExtraData = Option<(u64, S::Resources)>;

    // We identity few steps that are somewhat universal (it's named "basic"),
    // and will try to adhere to them to easier compose the execution flow for transactions that are "intrinsic" and not "enforced upon".

    // We also keep initial transaction parsing/obtaining out of scope

    type ScratchSpace = TxDataBuffer<S::Allocator>;
    fn create_tx_loop_scratch_space(system: &mut System<S>) -> Self::ScratchSpace {
        TxDataBuffer::new(system.get_allocator())
    }

    type TransactionBuffer<'a> = &'a mut [u8];
    fn try_begin_next_tx<'a>(
        system: &'_ mut System<S>,
        scratch_space: &'a mut Self::ScratchSpace,
    ) -> Option<Self::TransactionBuffer<'a>> {
        let tx_length_in_bytes = system
            .try_begin_next_tx(&mut scratch_space.into_writable())
            .expect("TX start call must always succeed")?;
        let initial_calldata_buffer = scratch_space.as_tx_buffer(tx_length_in_bytes);

        Some(initial_calldata_buffer)
    }

    fn parse_transaction<'a>(
        _system: &System<S>,
        source: Self::TransactionBuffer<'a>,
        _tracer: &mut impl Tracer<S>,
    ) -> Result<Self::Transaction<'a>, TxError> {
        ZkSyncTransaction::try_from_slice(source)
            .map_err(|_| TxError::Validation(InvalidTransaction::InvalidEncoding))
    }

    #[inline(always)]
    fn before_validation<'a>(
        system: &System<S>,
        transaction: &Self::Transaction<'a>,
        _tracer: &mut impl Tracer<S>,
    ) -> Result<(), TxError> {
        let _ = system.get_logger().write_fmt(
            format_args!(
                "Will process transaction from 0x{:040x} to 0x{:040x} with gas limit of {} and value of {:?} and {} bytes of calldata\n",
                transaction.from.read().as_uint(),
                transaction.to.read().as_uint(),
                transaction.gas_limit.read(),
                transaction.value.read(),
                transaction.calldata().len(),
            )
        );

        Ok(())
    }

    fn validate_and_prepare_context<'a, Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        transaction: Self::Transaction<'a>,
        tracer: &mut impl Tracer<S>,
    ) -> Result<(Self::TransactionContext, Self::Transaction<'a>), TxError> {
        let context = self::validation_impl::validate_and_compute_fee_for_transaction::<S, Config>(
            system,
            &transaction,
            tracer,
        )?;

        Ok((context, transaction))
    }

    fn precharge_fee<Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        transaction: &Self::Transaction<'_>,
        context: &mut Self::TransactionContext,
        _tracer: &mut impl Tracer<S>,
    ) -> Result<(), TxError> {
        let from = transaction.from.read();
        let value = context.fee_to_prepay;

        let _ = system.get_logger().write_fmt(format_args!(
            "Will precharge {:?} native tokens for transaction\n",
            &value
        ));

        context
            .resources
            .main_resources
            .with_infinite_ergs(|resources| {
                system.io.update_account_nominal_token_balance(
                    ExecutionEnvironmentType::NoEE, // out of scope of other interactions
                    resources,
                    &from,
                    &value,
                    true,
                )
            })
            .map_err(|e| match e {
                SubsystemError::LeafUsage(interface_error) => {
                    unreachable!(
                        "balance should be pre-verified, but received error {:?}",
                        interface_error
                    );
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

    fn before_execute_transaction_payload<'a>(
        system: &mut System<S>,
        transaction: &Self::Transaction<'a>,
        context: &mut Self::TransactionContext,
        _tracer: &mut impl Tracer<S>,
    ) -> Result<(), TxError> {
        let validation_pubdata = system.net_pubdata_used()?;
        context.validation_pubdata = validation_pubdata;

        // Save resources to be able to compute native consumption after everything
        let initial_resources = context.resources.main_resources.clone();
        context.initial_resources = initial_resources;

        system.set_tx_context((transaction.from.read(), context.gas_price_for_metadata).into());

        Ok(())
    }

    fn create_frame_and_execute_transaction_payload<'a, Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        memories: RunnerMemoryBuffers<'a>,
        transaction: &Self::Transaction<'_>,
        context: &mut Self::TransactionContext,
        tracer: &mut impl Tracer<S>,
    ) -> Result<
        (
            ExecutionResult<'a, S::IOTypes>,
            Self::ExecutionBodyExtraData,
        ),
        BootloaderSubsystemError,
    >
    where
        S: 'a,
    {
        // Take a snapshot in case we need to revert due to out of native.
        let main_body_rollback_handle = system.start_global_frame()?;

        let (execution_result, pubdata_info) = match Self::execute_or_deploy_inner::<Config>(
            system,
            system_functions,
            memories,
            &transaction,
            context,
            tracer,
        ) {
            Ok((r, (pubdata_used, to_charge_for_pubdata))) => {
                let pubdata_info = match r {
                    ExecutionResult::Success { .. } => {
                        system.finish_global_frame(None)?;
                        let _ = system
                            .get_logger()
                            .write_fmt(format_args!("Transaction main payload was processed\n"));

                        Some((pubdata_used, to_charge_for_pubdata))
                    }
                    ExecutionResult::Revert { .. } => {
                        system.finish_global_frame(Some(&main_body_rollback_handle))?;
                        let _ = system
                            .get_logger()
                            .write_fmt(format_args!("Transaction main payload was reverted\n"));
                        None
                    }
                };
                (r, pubdata_info)
            }
            // Out of native is converted to a top-level revert and
            // gas is exhausted.
            Err(e) => match e.root_cause() {
                RootCause::Runtime(e @ RuntimeError::OutOfNativeResources(_)) => {
                    let _ = system.get_logger().write_fmt(format_args!(
                        "Transaction ran out of native resources: {e:?}\n"
                    ));
                    context.resources.main_resources.exhaust_ergs();
                    system.finish_global_frame(Some(&main_body_rollback_handle))?;
                    (ExecutionResult::Revert { output: &[] }, None)
                }
                _ => return Err(e),
            },
        };
        drop(main_body_rollback_handle);

        Ok((execution_result, pubdata_info))
    }

    fn before_refund<'a, Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        transaction: &Self::Transaction<'a>,
        context: &mut Self::TransactionContext,
        _result: &ExecutionResult<'a, S::IOTypes>,
        extra_data: Self::ExecutionBodyExtraData,
        _tracer: &mut impl Tracer<S>,
    ) -> Result<(), InternalError> {
        use crate::bootloader::gas_helpers::get_resources_to_charge_for_pubdata;
        use evm_interpreter::ERGS_PER_GAS;

        // Just used for computing native used
        context.resources_before_refund = context.resources.main_resources.clone();

        // After the transaction is executed, we reclaim the withheld resources.
        // This is needed to ensure correct "gas_used" calculation, also these
        // resources could be spent for pubdata.
        context
            .resources
            .main_resources
            .reclaim_withheld(context.resources.withheld.take());

        if Config::ONLY_SIMULATE {
            let min_gas_used = context.minimal_ergs_to_charge.0 / ERGS_PER_GAS;
            // Compute gas used following the same logic as in normal execution
            // TODO: remove when simulation flow runs validation
            let (pubdata_spent, to_charge_for_pubdata) = get_resources_to_charge_for_pubdata(
                system,
                U256::from(context.native_per_pubdata),
                None,
            )?;
            let (_gas_refund, gas_used, evm_refund) = BasicBootloader::<S>::compute_gas_refund(
                system,
                to_charge_for_pubdata,
                transaction.gas_limit.read(),
                min_gas_used,
                U256::from(context.native_per_gas),
                &mut context.resources.main_resources,
            )?;
            context.gas_used = gas_used;
            context.gas_refunded = evm_refund;
            context.total_pubdata = pubdata_spent;

            return Ok(());
        }

        let pubdata_info = extra_data;

        let _ = system.get_logger().write_fmt(format_args!(
            "Have {:?} resources available before refund, and need to cover {:?} pubdata\n",
            &context.resources.main_resources, &pubdata_info
        ));

        let validation_pubdata = context.validation_pubdata;

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
                        U256::from(context.native_per_pubdata),
                        Some(validation_pubdata),
                    )?;
                (
                    execution_pubdata_spent + validation_pubdata,
                    to_charge_for_pubdata,
                )
            }
        };
        let min_gas_used = context.minimal_ergs_to_charge.0 / ERGS_PER_GAS;
        let (_total_gas_refund, gas_used, evm_refund) = BasicBootloader::<S>::compute_gas_refund(
            system,
            to_charge_for_pubdata,
            transaction.gas_limit.read(),
            min_gas_used,
            U256::from(context.native_per_gas),
            &mut context.resources.main_resources,
        )?;
        debug_assert_eq!(context.gas_used, 0);
        context.gas_used = gas_used;
        context.gas_refunded = evm_refund;
        context.total_pubdata = total_pubdata_used;

        Ok(())
    }

    fn refund_and_commit_fee<'a, Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        transaction: &Self::Transaction<'_>,
        context: &mut Self::TransactionContext,
        _tracer: &mut impl Tracer<S>,
    ) -> Result<(), BalanceSubsystemError> {
        // here we refund the user, then we will transfer fee to the operator

        // use would be refunded based on potentially one gas price, and operator will be paid using different one. But those
        // changes are not "transfers" in nature

        let mut inf_resources = S::Resources::FORMAL_INFINITE;

        assert!(
            context.gas_used <= context.tx_gas_limit,
            "gas limit is {}, but {} gas is reported as used",
            context.tx_gas_limit,
            context.gas_used
        );

        if context.tx_gas_limit > context.gas_used {
            let _ = system.get_logger().write_fmt(format_args!(
                "Gas price for refund is {:?}\n",
                &context.gas_price_for_metadata
            ));

            // refund
            let receiver = transaction.from.read();
            let refund = context.gas_price_for_metadata
                * U256::from(context.tx_gas_limit - context.gas_used); // can not overflow

            inf_resources.with_infinite_ergs(|resources| {
                system.io.update_account_nominal_token_balance(
                    ExecutionEnvironmentType::NoEE, // out of scope of other interactions
                    resources,
                    &receiver,
                    &refund,
                    false,
                )
            })?;
        }

        assert!(context.gas_used > 0);

        let _ = system.get_logger().write_fmt(format_args!(
            "Gas price for coinbase fee is {:?}\n",
            &context.gas_price_for_fee_commitment
        ));

        let fee = context.gas_price_for_fee_commitment * U256::from(context.gas_used); // can not overflow
        let coinbase = system.get_coinbase();

        inf_resources.with_infinite_ergs(|resources| {
            system.io.update_account_nominal_token_balance(
                ExecutionEnvironmentType::NoEE, // out of scope of other interactions
                resources,
                &coinbase,
                &fee,
                false,
            )
        })?;

        Ok(())
    }

    type ExecutionResult<'a> = ZkTxResult<'a>;

    fn after_execution<'a, Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        transaction: Self::Transaction<'_>,
        context: Self::TransactionContext,
        result: ExecutionResult<'a, S::IOTypes>,
        transaction_data_collector: &mut impl BlockTransactionsDataCollector<S, Self>,
        _tracer: &mut impl Tracer<S>,
    ) -> Self::ExecutionResult<'a> {
        // Add back the intrinsic native charged in get_resources_for_tx,
        // as initial_resources doesn't include them.
        let computational_native_used = context
            .resources_before_refund
            .clone()
            .diff(context.initial_resources.clone())
            .native()
            .as_u64()
            .saturating_add(context.resources.intrinsic_computational_native_charged);

        #[cfg(not(target_arch = "riscv32"))]
        cycle_marker::log_marker(
            format!(
                "Spent ergs for [process_transaction]: {}",
                context.gas_used * evm_interpreter::ERGS_PER_GAS
            )
            .as_str(),
        );
        #[cfg(not(target_arch = "riscv32"))]
        cycle_marker::log_marker(
            format!("Spent native for [process_transaction]: {computational_native_used}").as_str(),
        );

        transaction_data_collector.record_transaction_results(
            &*system,
            transaction,
            &context,
            &result,
        );

        use crate::bootloader::constants::L2_TX_INTRINSIC_PUBDATA;

        ZkTxResult {
            result,
            tx_hash: context.tx_hash,
            is_l1_tx: false,
            is_upgrade_tx: false,
            gas_used: context.gas_used,
            gas_refunded: context.gas_refunded,
            computational_native_used,
            pubdata_used: context.total_pubdata + L2_TX_INTRINSIC_PUBDATA as u64,
        }
    }
}

impl<S: EthereumLikeTypes> ZkTransactionFlowOnlyEOA<S>
where
    S::IO: IOSubsystemExt,
    S::Metadata: ZkSpecificPricingMetadata,
    <S::Metadata as BasicMetadata<S::IOTypes>>::TransactionMetadata: From<(B160, U256)>,
{
    fn execute_call<'a, Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        memories: RunnerMemoryBuffers<'a>,
        transaction: &<Self as BasicTransactionFlow<S>>::Transaction<'_>,
        context: &mut <Self as BasicTransactionFlow<S>>::TransactionContext,
        tracer: &mut impl Tracer<S>,
    ) -> Result<TxExecutionResult<'a, S>, BootloaderSubsystemError>
    where
        S: 'a,
    {
        let from = transaction.from.read();
        let main_calldata = transaction.calldata();
        let to = transaction.to.read();
        let nominal_token_value = transaction.value.read();

        let resources = context.resources.main_resources.take();

        let final_state = crate::bootloader::BasicBootloader::<S>::run_single_interaction(
            system,
            system_functions,
            memories,
            main_calldata,
            &from,
            &to,
            resources,
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

        let _ = system.get_logger().write_fmt(format_args!(
            "Resources to refund = {resources_returned:?}\n",
        ));
        context.resources.main_resources.reclaim(resources_returned);

        Ok(TxExecutionResult {
            return_values,
            reverted,
            deployed_address: DeployedAddress::CallNoAddress,
        })
    }

    fn perform_deployment<'a, Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        memories: RunnerMemoryBuffers<'a>,
        transaction: &<Self as BasicTransactionFlow<S>>::Transaction<'_>,
        context: &mut <Self as BasicTransactionFlow<S>>::TransactionContext,
        to_ee_type: ExecutionEnvironmentType,
        tracer: &mut impl Tracer<S>,
    ) -> Result<TxExecutionResult<'a, S>, BootloaderSubsystemError>
    where
        S: 'a,
    {
        use crate::bootloader::runner::run_till_completion;
        use crate::bootloader::supported_ees::SystemBoundEVMInterpreter;

        // NOTE: in this transaction execution workflow (from this folder),
        // we did pre-charge for deployment being the entry-point for the transaction,
        // and validated input length. So we just need to move into EE

        let ee_specific_deployment_processing_data = match to_ee_type {
            ExecutionEnvironmentType::NoEE => {
                return Err(internal_error!("Deployment cannot target NoEE").into());
            }
            ExecutionEnvironmentType::EVM => {
                SystemBoundEVMInterpreter::<S>::default_ee_deployment_options(system)
            }
        };

        let from = transaction.from.read();
        let main_calldata = transaction.calldata();
        let nominal_token_value = transaction.value.read();

        let resources = context.resources.main_resources.take();

        let deployment_parameters = DeploymentPreparationParameters {
            address_of_deployer: from,
            call_scratch_space: None,
            constructor_parameters: &[],
            nominal_token_value,
            deployment_code: main_calldata,
            ee_specific_deployment_processing_data,
            deployer_full_resources: resources,
            deployer_nonce: Some(context.originator_nonce_to_use),
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

        let _ = system.get_logger().write_fmt(format_args!(
            "Resources to refund = {resources_returned:?}\n",
        ));
        context.resources.main_resources.reclaim(resources_returned);

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

        let _ = system.get_logger().write_fmt(format_args!(
            "Deployment at {at:?} ended with success = {deployment_success}\n"
        ));
        let returndata_iter = return_values.returndata.iter().copied();
        let _ = system.get_logger().write_fmt(format_args!("Returndata = "));
        let _ = system.get_logger().log_data(returndata_iter);
        let _ = system.get_logger().write_fmt(format_args!("\n"));
        let deployed_address = at
            .map(DeployedAddress::Address)
            .unwrap_or(DeployedAddress::RevertedNoAddress);
        Ok(TxExecutionResult {
            return_values,
            reverted: !deployment_success,
            deployed_address,
        })
    }

    fn execute_or_deploy_inner<'a, Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        memories: RunnerMemoryBuffers<'a>,
        transaction: &<Self as BasicTransactionFlow<S>>::Transaction<'_>,
        context: &mut <Self as BasicTransactionFlow<S>>::TransactionContext,
        tracer: &mut impl Tracer<S>,
    ) -> Result<(ExecutionResult<'a, S::IOTypes>, (u64, S::Resources)), BootloaderSubsystemError>
    where
        S: 'a,
    {
        let _ = system
            .get_logger()
            .write_fmt(format_args!("Start of execution\n"));

        let to_ee_type = if !transaction.reserved[1].read().is_zero() {
            Some(ExecutionEnvironmentType::EVM)
        } else {
            None
        };

        let TxExecutionResult {
            return_values,
            reverted,
            deployed_address,
        } = match to_ee_type {
            Some(to_ee_type) => Self::perform_deployment::<Config>(
                system,
                system_functions,
                memories,
                transaction,
                context,
                to_ee_type,
                tracer,
            )?,
            None => Self::execute_call::<Config>(
                system,
                system_functions,
                memories,
                transaction,
                context,
                tracer,
            )?,
        };

        let returndata_region = return_values.returndata;
        let _ = system
            .get_logger()
            .log_data(returndata_region.iter().copied());

        let _ = system
            .get_logger()
            .write_fmt(format_args!("Main TX body successful = {}\n", !reverted));

        let mut execution_result = match reverted {
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

        let _ = system
            .get_logger()
            .write_fmt(format_args!("Transaction execution completed\n"));

        use crate::bootloader::gas_helpers::check_enough_resources_for_pubdata;
        let (has_enough, to_charge_for_pubdata, pubdata_used) = check_enough_resources_for_pubdata(
            system,
            U256::from(context.native_per_pubdata),
            &mut context.resources.main_resources,
            Some(context.validation_pubdata),
        )?;
        if !has_enough {
            execution_result = execution_result.reverted();
            let _ = system
                .get_logger()
                .write_fmt(format_args!("Not enough gas for pubdata after execution\n"));
            Ok((
                execution_result.reverted(),
                (pubdata_used, to_charge_for_pubdata),
            ))
        } else {
            Ok((execution_result, (pubdata_used, to_charge_for_pubdata)))
        }
    }
}
