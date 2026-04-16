use crate::alloc::string::ToString;
use crate::bootloader::block_flow::BlockTransactionsDataKeeper;
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
use alloc::format;
use core::fmt::Write;
use errors::cascade::CascadedError;
use errors::root_cause::RootCause;
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
use zk_ee::system::validator::TxValidator;
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
    pub is_priority_tx: bool,
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
    /// Resources used to pay for system operations that are precharged by the
    /// intrinsic computational native formula (e.g. ecrecover, keccak of the
    /// signed/tx hash, account read, nonce write, fee prepayment, refund and
    /// coinbase payment). Always initialized to `FORMAL_INFINITE`. Under the
    /// `verify_intrinsic_native` feature the actual native consumption is
    /// recovered by subtracting the residual from `FORMAL_INFINITE` and
    /// compared against the formula as an upper bound.
    pub intrinsic_resources: S::Resources,
    /// Number of EIP-7702 authorization list entries in the transaction.
    /// Used by `verify_intrinsic_native` to skip the overcharging check when
    /// authorizations are present (failed auths consume much less native than
    /// the worst-case formula budgets).
    pub authorization_list_num: u64,
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
            .field("gas_refunded", &self.gas_refunded)
            .field("validation_pubdata", &self.validation_pubdata)
            .field("total_pubdata", &self.total_pubdata)
            .field("native_used", &self.native_used)
            .field("intrinsic_resources", &self.intrinsic_resources)
            .field("authorization_list_num", &self.authorization_list_num)
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
            .intrinsic_resources
            .with_infinite_ergs(|resources| {
                system.io.update_account_nominal_token_balance(
                    ExecutionEnvironmentType::NoEE,
                    resources,
                    &from,
                    &fee,
                    true,
                    Config::SIMULATION,
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
                // shouldn't be reachable as we are using infinite resources
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
        // we are saving amount of pubdata spent during validation,
        // it's already covered by intrinsic cost, so it will be excluded
        // from pubdata payment after execution.
        // In the future we may consider using intrinsic pubdata here,
        // so worst case validation pubdata will be "refunded" if not used
        context.validation_pubdata = system.net_pubdata_used()?;

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
        validator: &mut impl TxValidator<S>,
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
            validator,
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
    ) -> Result<(), BootloaderSubsystemError> {
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

            // First refund the sender. Routed through `intrinsic_resources` so
            // the native charge (precharged by the intrinsic formula) can be
            // verified under `verify_intrinsic_native`.
            context
                .intrinsic_resources
                .with_infinite_ergs(|resources| {
                    system.io.update_account_nominal_token_balance(
                        ExecutionEnvironmentType::NoEE,
                        resources,
                        &refund_recipient,
                        &token_to_refund,
                        false,
                        Config::SIMULATION,
                    )
                })
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
            // We use saturating arithmetic to allow the caller of this method to
            // allow gas_price < base_fee. This can be used, for example, for
            // transaction simulation
            context.gas_price.saturating_sub(base_fee)
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
        // Operator payment native is precharged by the intrinsic formula too.
        context
            .intrinsic_resources
            .with_infinite_ergs(|resources| {
                system.io.update_account_nominal_token_balance(
                    ExecutionEnvironmentType::NoEE,
                    resources,
                    &coinbase,
                    &token_to_pay_operator,
                    false,
                    Config::SIMULATION,
                )
            })
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
        transaction: Transaction<<S as SystemTypes>::Allocator>,
        context: Self::TransactionContext,
        result: ExecutionResult<'a, <S as SystemTypes>::IOTypes>,
        _transaction_data_keeper: &mut impl BlockTransactionsDataKeeper<S, Self>,
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

        use crate::bootloader::transaction_flow::gas_helpers::calculate_l2_tx_intrinsic_pubdata;

        let num_blobs = system.metadata.num_blobs();
        let blob_gas_used = num_blobs as u64 * GAS_PER_BLOB;

        #[cfg(feature = "verify_intrinsic_native")]
        Self::verify_intrinsic_native(system, &context);

        let intrinsic_pubdata = calculate_l2_tx_intrinsic_pubdata(
            context.authorization_list_num,
            transaction.is_service(),
        );

        ZkTxResult {
            result,
            tx_hash: context.tx_hash,
            is_priority_tx: false,
            is_upgrade_tx: false,
            is_service_tx: transaction.is_service(),
            gas_used: context.gas_used,
            gas_refunded: context.gas_refunded,
            native_used: context.native_used,
            computational_native_used,
            pubdata_used: context.total_pubdata + intrinsic_pubdata,
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
        validator: &mut impl TxValidator<S>,
    ) -> Result<Self::ExecutionResult<'a>, BootloaderSubsystemError>
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
            validator,
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
        validator: &mut impl TxValidator<S>,
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
            validator,
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
        validator: &mut impl TxValidator<S>,
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
            validator,
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
        validator: &mut impl TxValidator<S>,
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
                validator,
            )?,
            None => Self::execute_call(
                system,
                system_functions,
                memories,
                transaction,
                context,
                tracer,
                validator,
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
            execution_result = execution_result.to_reverted();
            system_log!(system, "Not enough gas for pubdata after execution\n");
            // Burn all remaining ergs.
            context.resources.main_resources.exhaust_ergs();
            Ok((
                execution_result.to_reverted(),
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

    /// Compare the native that was actually consumed for operations covered by the
    /// intrinsic computational native formula (everything charged through
    /// `intrinsic_resources`, plus the per-site deltas accumulated during validation
    /// for the access list and authorization list) against the formula value. The
    /// formula must be an upper bound; otherwise a transaction could consume more
    /// native than was precharged.
    #[cfg(feature = "verify_intrinsic_native")]
    fn verify_intrinsic_native(
        system: &mut System<S>,
        context: &TxContextForPreAndPostProcessing<S>,
    ) {
        let initial = S::Resources::FORMAL_INFINITE.native().as_u64();
        let remaining = context.intrinsic_resources.native().as_u64();
        let actual_used = initial.saturating_sub(remaining);
        let formula = context.resources.intrinsic_computational_native_charged;
        system_log!(
            system,
            "intrinsic native verification: formula={}, actually_used={}\n",
            formula,
            actual_used
        );
        assert!(
            actual_used <= formula,
            "intrinsic computational native formula ({}) is not an upper bound on actual consumption ({})",
            formula,
            actual_used
        );
        // Skip the overcharging check when authorization-list entries are
        // present: failed auths (bad sig, wrong chain id, nonce overflow)
        // consume only PER_AUTH_NATIVE_COMPUTATIONAL_OVERHEAD while the
        // formula budgets worst-case success cost per entry.
        if context.authorization_list_num == 0 {
            assert!(
                formula <= actual_used * 2,
                "intrinsic computational native formula ({}) is overcharging more than twice comparing to actual consumption ({})",
                formula,
                actual_used
            );
        }
    }
}
