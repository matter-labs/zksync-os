use crate::alloc::string::ToString;
use crate::bootloader::errors::{BootloaderInterfaceError, BootloaderSubsystemError};
use crate::bootloader::errors::{InvalidTransaction, TxError};
use crate::bootloader::runner::RunnerMemoryBuffers;
use crate::bootloader::supported_ees::errors::EESubsystemError;
use crate::bootloader::transaction::Transaction;
use crate::bootloader::transaction_flow::gas_helpers::{
    get_resources_to_charge_for_pubdata, ResourcesForTx,
};
use crate::bootloader::transaction_flow::refund_calculation::compute_gas_refund;
use crate::bootloader::transaction_flow::BasicTransactionFlow;
use crate::bootloader::transaction_flow::DeployedAddress;
use crate::bootloader::transaction_flow::MinimalTransactionOutput;
use crate::bootloader::transaction_flow::TxExecutionResult;
use crate::bootloader::transaction_flow::{ExecutionOutput, ExecutionResult};
use crate::bootloader::BasicBootloaderExecutionConfig;
use crate::bootloader::TxProcessingOutput;
use core::fmt::Write;
use errors::cascade::CascadedError;
use errors::internal::InternalError;
use errors::root_cause::RootCause;
use errors::system::SystemError;
use metadata::basic_metadata::{BasicMetadata, ZkSpecificPricingMetadata};
use metadata::zk_metadata::TxLevelMetadata;
use ruint::aliases::U256;
use zk_ee::common_structs::system_hooks::HooksStorage;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::system::errors::interface::InterfaceError;
use zk_ee::system::errors::root_cause::GetRootCause;
use zk_ee::system::errors::subsystem::SubsystemError;
use zk_ee::system::metadata::basic_metadata::BasicTransactionMetadata;
use zk_ee::system::tracer::Tracer;
use zk_ee::system::{
    errors::runtime::RuntimeError, logger::Logger, EthereumLikeTypes, System, SystemTypes, *,
};
use zk_ee::system_log;
use zk_ee::types_config::EthereumIOTypesConfig;
use zk_ee::utils::Bytes32;
use zk_ee::{interface_error, internal_error, out_of_native_resources, wrap_error};

use super::gas_helpers::check_enough_resources_for_pubdata;

pub mod process_l1_transaction;
mod validation_impl;

pub struct ZkTransactionFlowOnlyEOA<S: EthereumLikeTypes> {
    _marker: core::marker::PhantomData<S>,
}

#[derive(Debug)]
pub struct ZkTxResult<'a> {
    pub result: ExecutionResult<'a, EthereumIOTypesConfig>,
    pub tx_hash: Bytes32,
    pub is_l1_tx: bool,
    pub is_upgrade_tx: bool,
    pub is_service_tx: bool,
    pub gas_refunded: u64,
    pub gas_used: u64,
    pub computational_native_used: u64,
    pub native_used: u64,
    pub pubdata_used: u64,
    pub blob_gas_used: u64,
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
            native_used: self.native_used,
        }
    }
}

pub struct TxContextForPreAndPostProcessing<S: EthereumLikeTypes> {
    pub resources: ResourcesForTx<S>,
    pub tx_hash: Bytes32,
    pub fee_to_prepay: U256,
    pub gas_price: U256,
    pub minimal_ergs_to_charge: Ergs,
    pub originator_nonce_to_use: u64,
    pub native_per_pubdata: u64,
    pub native_per_gas: u64,
    pub tx_gas_limit: u64,
    pub gas_used: u64,
    pub gas_refunded: u64,
    pub validation_pubdata: u64,
    pub total_pubdata: u64,
    pub native_used: u64,
    pub initial_resources: S::Resources,
    pub resources_before_refund: S::Resources,
}

impl<S: EthereumLikeTypes> core::fmt::Debug for TxContextForPreAndPostProcessing<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TxContextForPreAndPostProcessing")
            .field("resources", &self.resources)
            .field("tx_hash", &self.tx_hash)
            .field("fee_to_prepay", &self.fee_to_prepay)
            .field("gas_price", &self.gas_price)
            .field("minimal_ergs_to_charge", &self.minimal_ergs_to_charge)
            .field("originator_nonce_to_use", &self.originator_nonce_to_use)
            .field("native_per_pubdata", &self.native_per_pubdata)
            .field("native_per_gas", &self.native_per_gas)
            .field("tx_gas_limit", &self.tx_gas_limit)
            .field("gas_used", &self.gas_used)
            .field("gas_refunded", &self.gas_used)
            .field("validation_pubdata", &self.validation_pubdata)
            .field("total_pubdata", &self.total_pubdata)
            .field("native_used", &self.native_used)
            .finish()
    }
}

///
/// Pubdata info collected after execution can be cached
/// to used in the refund step only if the execution succeeded.
/// Otherwise, these values needs to be recomputed after reverting
/// state changes.
///
pub struct CachedPubdataInfo<S: EthereumLikeTypes> {
    pubdata_used: u64,
    to_charge_for_pubdata: S::Resources,
}

impl<S: EthereumLikeTypes> core::fmt::Debug for CachedPubdataInfo<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CachedPubdataInfo")
            .field("pubdata_used", &self.pubdata_used)
            .field("to_charge_for_pubdata", &self.to_charge_for_pubdata)
            .finish()
    }
}

impl<S: EthereumLikeTypes> BasicTransactionFlow<S> for ZkTransactionFlowOnlyEOA<S>
where
    S::IO: IOSubsystemExt,
    S::Metadata: ZkSpecificPricingMetadata
        + BasicMetadata<S::IOTypes, TransactionMetadata = TxLevelMetadata<S::IOTypes>>,
{
    type TransactionContext = TxContextForPreAndPostProcessing<S>;
    type ExecutionBodyExtraData = Option<CachedPubdataInfo<S>>;
    type ExecutionResult<'a> = ZkTxResult<'a>;

    #[inline(always)]
    fn before_validation<'a>(
        system: &mut System<S>,
        transaction: &Transaction<S::Allocator>,
        _tracer: &mut impl Tracer<S>,
    ) -> Result<(), TxError> {
        system_log!(system, "Will process transaction from 0x{:040x} to {} with gas limit of {} and value of {:?} and {} bytes of calldata\n",
                transaction.from().as_uint(),
                // Inline match to avoid allocation
                match transaction.to() {
                    Some(to) => alloc::format!("0x{:040x}", to.as_uint()),
                    None => "null".to_string(),
                },
                transaction.gas_limit(),
                transaction.value(),
                transaction.calldata().len(),);
        Ok(())
    }

    fn validate_and_prepare_context<Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        transaction: &mut Transaction<<S as SystemTypes>::Allocator>,
        tracer: &mut impl Tracer<S>,
    ) -> Result<Self::TransactionContext, TxError> {
        let context = self::validation_impl::validate_and_compute_fee_for_transaction::<S, Config>(
            system,
            transaction,
            tracer,
        )?;
        Ok(context)
    }

    fn before_fee_collection(
        _system: &mut System<S>,
        _transaction: &Transaction<<S as SystemTypes>::Allocator>,
        _context: &Self::TransactionContext,
        _tracer: &mut impl Tracer<S>,
    ) -> Result<(), TxError> {
        Ok(())
    }

    fn precharge_fee<Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        transaction: &Transaction<<S as SystemTypes>::Allocator>,
        context: &mut Self::TransactionContext,
        _tracer: &mut impl Tracer<S>,
    ) -> Result<(), TxError> {
        let from = transaction.from();
        let fee = context.fee_to_prepay;

        system_log!(
            system,
            "Will precharge {:?} native tokens for transaction\n",
            &fee
        );

        // ARCHITECTURE NOTE: Fee payment is split into two phases:
        // 1. Deduct full fee from sender at transaction start (here)
        // 2. Transfer actual payment to operator after execution (in refund_transaction_and_pay_operator)
        // This ensures sender has sufficient funds before execution begins
        context
            .resources
            .main_resources
            .with_infinite_ergs(|resources| {
                system.io.update_account_nominal_token_balance(
                    ExecutionEnvironmentType::NoEE,
                    resources,
                    &from,
                    &fee,
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
                    RuntimeError::FatalRuntimeError(_) => {
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

    fn before_execute_transaction_payload(
        system: &mut System<S>,
        _transaction: &Transaction<<S as SystemTypes>::Allocator>,
        context: &mut Self::TransactionContext,
        _tracer: &mut impl Tracer<S>,
    ) -> Result<(), TxError> {
        // Charge for validation pubdata
        let (validation_pubdata, to_charge_for_pubdata) =
            get_resources_to_charge_for_pubdata(system, context.native_per_pubdata, None)?;
        context.validation_pubdata = validation_pubdata;
        Self::charge_for_validation_pubdata_using_withheld(
            &mut context.resources,
            &to_charge_for_pubdata,
        )?;

        // Save resources to be able to calculate computational native consumption after everything
        let initial_resources = context.resources.main_resources.clone();
        context.initial_resources = initial_resources;

        Ok(())
    }

    fn create_frame_and_execute_transaction_payload<'a, Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, <S as SystemTypes>::Allocator>,
        memories: RunnerMemoryBuffers<'a>,
        transaction: &Transaction<<S as SystemTypes>::Allocator>,
        context: &mut Self::TransactionContext,
        tracer: &mut impl Tracer<S>,
    ) -> Result<
        (
            ExecutionResult<'a, <S as SystemTypes>::IOTypes>,
            Self::ExecutionBodyExtraData,
        ),
        BootloaderSubsystemError,
    >
    where
        S: 'a,
    {
        // Take a snapshot in case we need to revert due to out of native.
        let main_body_rollback_handle = system.start_global_frame()?;

        // pubdata_info = (pubdata_used, to_charge_for_pubdata) can be cached
        // to used in the refund step only if the execution succeeded.
        // Otherwise, this value needs to be recomputed after reverting
        // state changes.
        let (execution_result, pubdata_info) = match Self::execute_or_deploy_inner(
            system,
            system_functions,
            memories,
            &transaction,
            context,
            tracer,
        ) {
            Ok((r, cached_pubdata_info)) => {
                let pubdata_info = match r {
                    ExecutionResult::Success { .. } => {
                        system.finish_global_frame(None)?;
                        system_log!(system, "Transaction main payload was processed\n");
                        Some(cached_pubdata_info)
                    }
                    ExecutionResult::Revert { .. } => {
                        system.finish_global_frame(Some(&main_body_rollback_handle))?;
                        system_log!(system, "Transaction main payload was reverted\n");
                        None
                    }
                };
                (r, pubdata_info)
            }
            // Out of native is converted to a top-level revert and
            // gas is exhausted.
            Err(e) => match e.root_cause() {
                RootCause::Runtime(e @ RuntimeError::FatalRuntimeError(_)) => {
                    system_log!(
                        system,
                        "Transaction ran out of native resources or memory: {e:?}\n"
                    );
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
        transaction: &Transaction<<S as SystemTypes>::Allocator>,
        context: &mut Self::TransactionContext,
        _result: &ExecutionResult<'a, <S as SystemTypes>::IOTypes>,
        pubdata_info: Self::ExecutionBodyExtraData,
        _tracer: &mut impl Tracer<S>,
    ) -> Result<(), InternalError> {
        use evm_interpreter::ERGS_PER_GAS;

        // Just used for computing native used
        context.resources_before_refund = context.resources.main_resources.clone();

        // Now we can actually reclaim resources withheld for pubdata
        context
            .resources
            .main_resources
            .reclaim_withheld(context.resources.withheld.take());

        system_log!(
            system,
            "Have {:?} resources available before refund, and need to cover {:?} pubdata\n",
            &context.resources.main_resources,
            &pubdata_info
        );

        let validation_pubdata = context.validation_pubdata;

        // Pubdata for validation has been charged already,
        // we charge for the rest now.
        let (total_pubdata_used, to_charge_for_pubdata) = match pubdata_info {
            Some(CachedPubdataInfo {
                pubdata_used,
                to_charge_for_pubdata,
            }) => (pubdata_used + validation_pubdata, to_charge_for_pubdata),
            None => {
                let (execution_pubdata_spent, to_charge_for_pubdata) =
                    get_resources_to_charge_for_pubdata(
                        system,
                        context.native_per_pubdata,
                        Some(validation_pubdata),
                    )?;
                (
                    execution_pubdata_spent + validation_pubdata,
                    to_charge_for_pubdata,
                )
            }
        };
        let min_gas_used = context.minimal_ergs_to_charge.0 / ERGS_PER_GAS;
        let refund_info = compute_gas_refund(
            system,
            to_charge_for_pubdata,
            transaction.gas_limit(),
            min_gas_used,
            context.native_per_gas,
            &mut context.resources.main_resources,
        )?;
        debug_assert_eq!(context.gas_used, 0);
        context.gas_used = refund_info.gas_used;
        context.gas_refunded = refund_info.evm_refund;
        context.total_pubdata = total_pubdata_used;
        context.native_used = refund_info.native_used;

        Ok(())
    }

    fn refund_and_commit_fee<Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        transaction: &Transaction<<S as SystemTypes>::Allocator>,
        context: &mut Self::TransactionContext,
        _tracer: &mut impl Tracer<S>,
    ) -> Result<(), BootloaderSubsystemError> {
        // here we refund the user, then we will transfer fee to the operator

        if context.tx_gas_limit > context.gas_used {
            system_log!(system, "Gas price for refund is {:?}\n", &context.gas_price);

            // refund
            let refund_recipient = transaction.from();
            let token_to_refund =
                context.gas_price * U256::from(context.tx_gas_limit - context.gas_used); // can not overflow

            let mut inf_resources = S::Resources::FORMAL_INFINITE;
            // First refund the sender
            system
                .io
                .update_account_nominal_token_balance(
                    ExecutionEnvironmentType::NoEE,
                    &mut inf_resources,
                    &refund_recipient,
                    &token_to_refund,
                    false,
                )
                .map_err(|e| match e {
                    // Balance errors can not be cascaded
                    SubsystemError::Cascaded(CascadedError(inner, _)) => match inner {},
                    SubsystemError::LeafUsage(InterfaceError(ie, _)) => match ie {
                        BalanceError::InsufficientBalance => {
                            unreachable!("Cannot be insufficient when incrementing balance")
                        }
                        BalanceError::Overflow => {
                            interface_error!(BootloaderInterfaceError::CantPayRefundOverflow)
                        }
                    },
                    other => wrap_error!(other),
                })?;
        }

        // Next we pay the operator
        // ARCHITECTURE NOTE: Fee payment is split into two phases:
        // 1. Deduct full fee from sender at transaction start (in pay_for_transaction)
        // 2. Transfer actual payment to operator after execution (here)
        // This ensures sender has sufficient funds before execution begins

        // EIP-1559 compatibility: When burn_base_fee is enabled, only priority fees
        // go to the operator. Base fees are effectively "burned" (not transferred anywhere).
        let gas_price_for_operator = if cfg!(feature = "burn_base_fee") {
            let base_fee = system.get_eip1559_basefee();
            context
                .gas_price
                .checked_sub(base_fee)
                .ok_or(internal_error!("Gas_price - base_fee underflow"))?
        } else {
            context.gas_price
        };

        system_log!(
            system,
            "Gas price for coinbase fee is {:?}\n",
            &gas_price_for_operator
        );

        let token_to_pay_operator = U256::from(context.gas_used)
            .checked_mul(gas_price_for_operator)
            .ok_or(internal_error!("gu*gpfo"))?;

        let coinbase = system.get_coinbase();
        let mut inf_resources = S::Resources::FORMAL_INFINITE;
        system
            .io
            .update_account_nominal_token_balance(
                ExecutionEnvironmentType::NoEE,
                &mut inf_resources,
                &coinbase,
                &token_to_pay_operator,
                false,
            )
            .map_err(|e| match e {
                // Balance errors can not be cascaded
                SubsystemError::Cascaded(CascadedError(inner, _)) => match inner {},
                SubsystemError::LeafUsage(InterfaceError(ie, _)) => match ie {
                    BalanceError::InsufficientBalance => {
                        unreachable!("Cannot be insufficient when incrementing balance")
                    }
                    BalanceError::Overflow => {
                        interface_error!(BootloaderInterfaceError::CantPayOperatorOverflow)
                    }
                },
                other => wrap_error!(other),
            })?;

        Ok(())
    }

    fn after_execution<'a, Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        transaction: &Transaction<<S as SystemTypes>::Allocator>,
        context: Self::TransactionContext,
        result: ExecutionResult<'a, <S as SystemTypes>::IOTypes>,
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

        use crate::bootloader::constants::L2_TX_INTRINSIC_PUBDATA;

        let num_blobs = system.metadata.num_blobs();
        let blob_gas_used = num_blobs as u64 * GAS_PER_BLOB;

        ZkTxResult {
            result,
            tx_hash: context.tx_hash,
            is_l1_tx: false,
            is_upgrade_tx: false,
            is_service_tx: transaction.is_service(),
            gas_used: context.gas_used,
            gas_refunded: context.gas_refunded,
            native_used: context.native_used,
            computational_native_used,
            pubdata_used: context.total_pubdata + L2_TX_INTRINSIC_PUBDATA,
            blob_gas_used,
        }
    }

    fn process_l1_transaction<'a, Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, <S as SystemTypes>::Allocator>,
        memories: RunnerMemoryBuffers<'a>,
        transaction: &crate::bootloader::transaction::abi_encoded::AbiEncodedTransaction<
            <S as SystemTypes>::Allocator,
        >,
        is_priority_op: bool,
        tracer: &mut impl Tracer<S>,
    ) -> Result<Self::ExecutionResult<'a>, TxError>
    where
        S: 'a,
    {
        self::process_l1_transaction::process_l1_transaction::<S, Config>(
            system,
            system_functions,
            memories,
            transaction,
            is_priority_op,
            tracer,
        )
    }
}

impl<S: EthereumLikeTypes> ZkTransactionFlowOnlyEOA<S>
where
    S::IO: IOSubsystemExt,
    S::Metadata: ZkSpecificPricingMetadata
        + BasicMetadata<S::IOTypes, TransactionMetadata = TxLevelMetadata<S::IOTypes>>,
{
    fn execute_call<'a>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        memories: RunnerMemoryBuffers<'a>,
        transaction: &Transaction<S::Allocator>,
        context: &mut <Self as BasicTransactionFlow<S>>::TransactionContext,
        tracer: &mut impl Tracer<S>,
    ) -> Result<TxExecutionResult<'a, S>, BootloaderSubsystemError>
    where
        S: 'a,
    {
        let from = transaction.from();
        let main_calldata = transaction.calldata();
        // panic is not reachable, to is validated
        let to = transaction.to().unwrap_or_default();
        let nominal_token_value = transaction.value();

        let resources = context.resources.main_resources.take();

        let final_state = crate::bootloader::BasicBootloader::<S, Self>::run_single_interaction(
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
            resources_returned,
            result,
        } = final_state;

        system_log!(system, "Resources to refund = {resources_returned:?}\n",);
        context.resources.main_resources.reclaim(resources_returned);

        let reverted = result.failed();
        let return_values = result.return_values();

        Ok(TxExecutionResult {
            return_values,
            reverted,
            deployed_address: DeployedAddress::CallNoAddress,
        })
    }

    fn perform_deployment<'a>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        memories: RunnerMemoryBuffers<'a>,
        transaction: &Transaction<S::Allocator>,
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

        let mut resources = context.resources.main_resources.take();
        let from = transaction.from();
        let main_calldata = transaction.calldata();
        let nominal_token_value = transaction.value();

        let deployed_address = match to_ee_type {
            ExecutionEnvironmentType::NoEE => {
                return Err(internal_error!("Deployment cannot target NoEE").into())
            }
            ExecutionEnvironmentType::EVM => {
                SystemBoundEVMInterpreter::<S>::derive_address_for_deployment_create(
                    &mut resources,
                    &from,
                    context.originator_nonce_to_use,
                )
                .map_err(|e| {
                    let ee_error: EESubsystemError = wrap_error!(e);
                    wrap_error!(ee_error)
                })?
            }
        };

        let ergs_to_pass = resources.ergs();

        let deployment_request = ExternalCallRequest {
            available_resources: resources,
            ergs_to_pass,
            caller: *from,
            callee: deployed_address,
            callers_caller: Default::default(), // Fine to use placeholder, should not be used
            modifier: CallModifier::Constructor,
            input: main_calldata,
            nominal_token_value: *nominal_token_value,
            call_scratch_space: None,
        };

        let rollback_handle = system.start_global_frame()?;

        let final_state = run_till_completion(
            memories,
            system,
            system_functions,
            to_ee_type,
            deployment_request,
            tracer,
        )?;

        let CompletedExecution {
            resources_returned,
            result: deployment_result,
        } = final_state;

        system_log!(system, "Resources to refund = {resources_returned:?}\n",);
        context.resources.main_resources.reclaim(resources_returned);

        let (deployment_success, reverted, return_values, at) = match deployment_result {
            CallResult::Successful { mut return_values } => {
                // In commonly used Ethereum clients it is expected that top-level deployment returns deployed bytecode as the returndata
                let deployed_bytecode =
                    context
                        .resources
                        .main_resources
                        .with_infinite_ergs(|inf_resources| {
                            system.io.get_observable_bytecode(
                                to_ee_type,
                                inf_resources,
                                &deployed_address,
                            )
                        })?;
                return_values.returndata = deployed_bytecode;

                (true, false, return_values, Some(deployed_address))
            }
            CallResult::Failed { return_values, .. } => (false, true, return_values, None),
            CallResult::PreparationStepFailed => {
                return Err(internal_error!("Preparation step failed in root call").into())
            } // Should not happen
        };
        system.finish_global_frame(reverted.then_some(&rollback_handle))?;

        system_log!(
            system,
            "Deployment at {at:?} ended with success = {deployment_success}\n"
        );
        let returndata_iter = return_values.returndata.iter().copied();
        system_log!(system, "Returndata = ");
        let _ = system.get_logger().log_data(returndata_iter);
        system_log!(system, "\n");
        let deployed_address = at
            .map(DeployedAddress::Address)
            .unwrap_or(DeployedAddress::RevertedNoAddress);
        Ok(TxExecutionResult {
            return_values,
            reverted: !deployment_success,
            deployed_address,
        })
    }

    fn execute_or_deploy_inner<'a>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        memories: RunnerMemoryBuffers<'a>,
        transaction: &Transaction<S::Allocator>,
        context: &mut <Self as BasicTransactionFlow<S>>::TransactionContext,
        tracer: &mut impl Tracer<S>,
    ) -> Result<(ExecutionResult<'a, S::IOTypes>, CachedPubdataInfo<S>), BootloaderSubsystemError>
    where
        S: 'a,
    {
        system_log!(system, "Start of execution\n");

        let to_ee_type = transaction.is_deployment();

        let TxExecutionResult {
            return_values,
            reverted,
            deployed_address,
        } = match to_ee_type {
            Some(to_ee_type) => Self::perform_deployment(
                system,
                system_functions,
                memories,
                transaction,
                context,
                to_ee_type,
                tracer,
            )?,
            None => Self::execute_call(
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

        system_log!(system, "Main TX body successful = {}\n", !reverted);

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

        system_log!(system, "Transaction execution completed\n");

        // After the transaction is executed, we reclaim the withheld resources.
        // This is needed to ensure correct "gas_used" calculation, also these
        // resources could be spent for pubdata.
        // We do not reclaim it to the actual `resources` yet, as that would make
        // the calculation of computational native used more complicated.
        let mut resources_for_check = context.resources.main_resources.clone();
        resources_for_check.reclaim_withheld(context.resources.withheld.clone());

        let (has_enough, to_charge_for_pubdata, pubdata_used) = check_enough_resources_for_pubdata(
            system,
            context.native_per_pubdata,
            &resources_for_check,
            Some(context.validation_pubdata),
        )?;
        if !has_enough {
            execution_result = execution_result.reverted();
            system_log!(system, "Not enough gas for pubdata after execution\n");
            Ok((
                execution_result.reverted(),
                CachedPubdataInfo {
                    pubdata_used,
                    to_charge_for_pubdata,
                },
            ))
        } else {
            Ok((
                execution_result,
                CachedPubdataInfo {
                    pubdata_used,
                    to_charge_for_pubdata,
                },
            ))
        }
    }

    ///
    /// Charge validation pubdata using both main and withheld resources.
    /// First try to use withheld.
    ///
    fn charge_for_validation_pubdata_using_withheld(
        resources: &mut ResourcesForTx<S>,
        to_charge_for_pubdata: &S::Resources,
    ) -> Result<(), SystemError> {
        if resources.withheld.has_enough(to_charge_for_pubdata) {
            // Simple case, just spend directly from withheld
            resources.withheld.charge(to_charge_for_pubdata)?;
            return Ok(());
        }

        if resources.withheld.is_empty() {
            // Simple case, just spend directly from main resources
            resources.main_resources.charge(to_charge_for_pubdata)?;
            return Ok(());
        }

        // General case: first compute the part that should be charged from
        // withheld.
        let to_charge_from_main = to_charge_for_pubdata.diff(resources.withheld.clone());
        // Then charge from withheld, this will return an Err with OON and zero it out.
        // We ignore the error and continue charging from the main resources.
        if resources.withheld.charge(to_charge_for_pubdata).is_ok() {
            return Err(internal_error!("Withheld should be insufficient, checked above").into());
        }
        resources.main_resources.charge(&to_charge_from_main)?;
        Ok(())
    }
}
