//! The transaction flow of a Glamsterdam block (EIP-7773): the Ethereum flow with
//!
//! - the decomposed intrinsic gas of EIP-2780, the calldata floor of EIP-7976 and the
//!   access list surcharge of EIP-7981 (`validation_impl`);
//! - the two gas dimensions of EIP-8037: the gas limit above the execution cap of EIP-7825
//!   goes to the state gas reservoir, the transaction settles both dimensions and the block
//!   accounts for them separately (`settle_gas`); the block counts gas before refunds
//!   (EIP-7778);
//! - the EIP-7702 delegations applied with their state-dependent costs after the fee is
//!   charged, so that running out of gas in them is a failed transaction, not an invalid one
//!   (`delegations`), followed by the runtime costs of the dispatch;
//! - the receipts carrying the gas after refunds and floor, as before.
//!
//! The value transfer log of EIP-7708 and the self-destruct rules of EIP-8246 are the
//! Glamsterdam IO's.

use super::*;
use crate::bootloader::block_flow::ethereum_glamsterdam::tx_range::metadata_op::GlamsterdamBlockMetadata;
use crate::bootloader::errors::BootloaderInterfaceError;
use crate::bootloader::errors::InvalidTransaction;
use crate::bootloader::supported_ees::errors::EESubsystemError;
use crate::bootloader::supported_ees::EthereumLikeSystem;
use crate::bootloader::supported_ees::EvmInterpreterExt;
use crate::bootloader::transaction_flow::ethereum::tx_level_metadata::EthereumTransactionMetadata;
use crate::bootloader::transaction_flow::logs_bloom::LogsBloom;
use crate::bootloader::transaction_flow::BasicTransactionFlow;
use core::fmt::Write;
use crypto::MiniDigest;
use errors::cascade::CascadedError;
use errors::interface::InterfaceError;
use evm_interpreter::v3::gas::{NEW_ACCOUNT, TX_MAX_GAS_LIMIT};
use ruint::aliases::{B160, U256};
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::interface_error;
use zk_ee::internal_error;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::system::errors::root_cause::GetRootCause;
use zk_ee::system::errors::root_cause::RootCause;
use zk_ee::system::errors::runtime::RuntimeError;
use zk_ee::system::errors::subsystem::SubsystemError;
use zk_ee::system::errors::system::SystemError;
use zk_ee::system::logger::Logger;
use zk_ee::system::MAX_BLOBS_PER_TX;
use zk_ee::system::*;
use zk_ee::system_log;
use zk_ee::types_config::EthereumIOTypesConfig;
use zk_ee::utils::Bytes32;
use zk_ee::wrap_error;

mod delegations;
mod validation_impl;

pub use validation_impl::IntrinsicGas;

pub struct GlamsterdamTransactionFlow<S: EthereumLikeSystem> {
    _marker: core::marker::PhantomData<S>,
}

#[derive(Debug)]
pub struct GlamsterdamTxResult<'a> {
    pub result: ExecutionResult<'a, EthereumIOTypesConfig>,
    /// The gas the sender paid for: after refunds and the calldata floor
    pub gas_used: u64,
}

impl<'a> MinimalTransactionOutput<'a> for GlamsterdamTxResult<'a> {
    fn is_success(&self) -> bool {
        matches!(self.result, ExecutionResult::Success { .. })
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
        unimplemented!("transaction hash is not computed for Ethereum STF");
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
            gas_refunded: 0,
            computational_native_used: 0,
            pubdata_used: 0,
            native_used: 0,
        }
    }
}

/// The settled gas of a transaction (EIP-8037, EIP-7778)
#[derive(Clone, Copy, Debug, Default)]
pub struct GasSettlement {
    /// What the sender pays for: after refunds, at least the calldata floor
    pub gas_used: u64,
    /// What the sender gets back, in gas
    pub gas_left: u64,
    /// The execution gas the block accounts for: before refunds, at least the floor
    pub execution_gas_used: u64,
    /// The state gas the block accounts for: the net state created
    pub state_gas_used: u64,
}

pub struct GlamsterdamTxContext<S: EthereumLikeSystem> {
    /// The execution gas (ergs), the state gas reservoir and its spill
    pub resources: S::Resources,
    pub fee_to_prepay: U256,
    pub priority_fee_per_gas: U256,
    pub originator_nonce_to_use: u64,
    pub tx_gas_limit: u64,
    pub intrinsic: IntrinsicGas,
    /// The state gas the transaction started with
    pub state_gas_reservoir: u32,
    /// The spill the delegations committed: never refilled
    pub committed_spill: u32,
    /// The state gas after the delegations and before the creation charge of the
    /// target: what a reverted or halted main body is restored to (EIP-8037), so that
    /// the creation, which does not happen then, is refilled
    pub state_gas_baseline: u32,
    pub blob_gas_used: u64,
    pub tx_level_metadata: EthereumTransactionMetadata<{ MAX_BLOBS_PER_TX }>,
    pub settlement: GasSettlement,
}

impl<S: EthereumLikeSystem> core::fmt::Debug for GlamsterdamTxContext<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("GlamsterdamTxContext")
            .field("resources", &self.resources)
            .field("fee_to_prepay", &self.fee_to_prepay)
            .field("priority_fee_per_gas", &self.priority_fee_per_gas)
            .field("originator_nonce_to_use", &self.originator_nonce_to_use)
            .field("tx_gas_limit", &self.tx_gas_limit)
            .field("intrinsic", &self.intrinsic)
            .field("state_gas_reservoir", &self.state_gas_reservoir)
            .field("committed_spill", &self.committed_spill)
            .field("blob_gas_used", &self.blob_gas_used)
            .field("settlement", &self.settlement)
            .finish()
    }
}

impl<S: EthereumLikeSystem<Metadata = GlamsterdamBlockMetadata>> BasicTransactionFlow<S>
    for GlamsterdamTransactionFlow<S>
where
    S::IO: IOSubsystemExt,
    S::Resources: StateGasResources,
{
    type TransactionContext = GlamsterdamTxContext<S>;
    type ExecutionBodyExtraData = ();
    type ExecutionResult<'a> = GlamsterdamTxResult<'a>;

    fn process_l1_transaction<'a, Config: BasicBootloaderExecutionConfig>(
        _system: &mut System<S>,
        _system_functions: &mut HooksStorage<S, <S as SystemTypes>::Allocator>,
        _memories: RunnerMemoryBuffers<'a>,
        _transaction: &AbiEncodedTransaction<<S as SystemTypes>::Allocator>,
        _is_priority_op: bool,
        _tracer: &mut impl Tracer<S>,
        _validator: &mut impl TxValidator<S>,
    ) -> Result<Self::ExecutionResult<'a>, BootloaderSubsystemError>
    where
        S: 'a,
    {
        Err(internal_error!("Ethereum STF doesn't support L1 txs").into())
    }

    fn before_validation<'a>(
        system: &mut System<S>,
        transaction: &Transaction<S::Allocator>,
        _tracer: &mut impl Tracer<S>,
    ) -> Result<(), TxError> {
        system_log!(
            system,
            "Will try to process transaction to {:?} with gas limit of {} and value of {:?} and {} bytes of calldata\n",
            transaction.to(),
            transaction.gas_limit(),
            transaction.value(),
            transaction.calldata().len(),
        );
        Ok(())
    }

    fn validate_and_prepare_context<Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        transaction: &mut Transaction<S::Allocator>,
        tracer: &mut impl Tracer<S>,
    ) -> Result<Self::TransactionContext, TxError> {
        validation_impl::validate_and_compute_fee_for_transaction::<S, Config>(
            system,
            transaction,
            tracer,
        )
    }

    fn before_fee_collection<'a>(
        system: &mut System<S>,
        transaction: &Transaction<S::Allocator>,
        context: &Self::TransactionContext,
        _tracer: &mut impl Tracer<S>,
    ) -> Result<(), TxError> {
        system_log!(
            system,
            "Will process transaction from 0x{:040x} (nonce {}) to {:?}: {:?}\n",
            transaction.from().as_uint(),
            context.originator_nonce_to_use,
            transaction.to(),
            context,
        );
        Ok(())
    }

    fn precharge_fee<Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        transaction: &Transaction<S::Allocator>,
        context: &mut Self::TransactionContext,
        _tracer: &mut impl Tracer<S>,
    ) -> Result<(), TxError> {
        let from = transaction.from();
        let value = context.fee_to_prepay;
        system_log!(
            system,
            "Will precharge 0x{:040x} with {:?} native tokens for transaction\n",
            from.as_uint(),
            &value
        );
        context
            .resources
            .with_infinite_ergs(|resources| {
                system.io.update_account_nominal_token_balance(
                    ExecutionEnvironmentType::NoEE,
                    resources,
                    from,
                    &value,
                    true,
                    false,
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
                        TxError::oon_as_validation(zk_ee::out_of_native_resources!().into())
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
        _transaction: &Transaction<S::Allocator>,
        context: &mut Self::TransactionContext,
        _tracer: &mut impl Tracer<S>,
    ) -> Result<(), TxError> {
        system.set_tx_context(context.tx_level_metadata.clone());
        Ok(())
    }

    /// The delegations and the runtime costs of the dispatch (EIP-2780, EIP-8037) in a
    /// frame of their own: a failure there rolls them back and forfeits the gas, the
    /// transaction is included as failed. Then the code, in its own frame.
    fn create_frame_and_execute_transaction_payload<'a, Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, <S as SystemTypes>::Allocator>,
        memories: RunnerMemoryBuffers<'a>,
        transaction: &Transaction<S::Allocator>,
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
        let preparation_rollback_handle = system.start_global_frame()?;
        match Self::prepare_dispatch(system, transaction, context) {
            Ok(()) => {
                system.finish_global_frame(None)?;
            }
            Err(PreparationError::OutOfGas) => {
                system_log!(system, "Transaction preparation ran out of gas\n");
                system.finish_global_frame(Some(&preparation_rollback_handle))?;
                // every state charge is refilled and all the gas is forfeited (the
                // refilled spill included)
                context
                    .resources
                    .restore_state_gas(context.state_gas_reservoir);
                context.committed_spill = 0;
                context.resources.exhaust_ergs();
                return Ok((ExecutionResult::Revert { output: &[] }, ()));
            }
            Err(PreparationError::Internal(e)) => return Err(e),
        }
        drop(preparation_rollback_handle);

        // Take a snapshot in case we need to revert due to out of native.
        let main_body_rollback_handle = system.start_global_frame()?;
        let baseline = context.state_gas_baseline;
        let execution_result = match Self::execute_or_deploy_inner(
            system,
            system_functions,
            memories,
            transaction,
            context,
            tracer,
            validator,
        ) {
            Ok(r) => {
                match r {
                    ExecutionResult::Success { .. } => {
                        system.finish_global_frame(None)?;
                        system_log!(system, "Transaction main payload was processed\n");
                    }
                    ExecutionResult::Revert { .. } => {
                        system.finish_global_frame(Some(&main_body_rollback_handle))?;
                        // the state the body created is gone, its creation charge
                        // included: the entry frame refilled its own charges, the
                        // target's creation is the flow's (`prepare_dispatch`)
                        context.resources.restore_state_gas(baseline);
                        system_log!(system, "Transaction main payload was reverted\n");
                    }
                };
                r
            }
            // Out of native is converted to a top-level revert and
            // gas is exhausted.
            Err(e) => match e.root_cause() {
                RootCause::Runtime(e @ RuntimeError::FatalRuntimeError(_)) => {
                    system_log!(
                        system,
                        "Transaction ran out of native resources or memory: {e:?}\n"
                    );
                    context.resources.restore_state_gas(baseline);
                    context.resources.exhaust_ergs();
                    system.finish_global_frame(Some(&main_body_rollback_handle))?;
                    ExecutionResult::Revert { output: &[] }
                }
                _ => return Err(e),
            },
        };
        drop(main_body_rollback_handle);

        Ok((execution_result, ()))
    }

    fn before_refund<'a, Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        _transaction: &Transaction<S::Allocator>,
        context: &mut Self::TransactionContext,
        _result: &ExecutionResult<'a, <S as SystemTypes>::IOTypes>,
        _extra_data: Self::ExecutionBodyExtraData,
        _tracer: &mut impl Tracer<S>,
    ) -> Result<(), BootloaderSubsystemError> {
        system_log!(
            system,
            "Have {:?} resources available before refund\n",
            &context.resources,
        );
        let refund_counter = system.io.get_refund_counter().legacy_gas();
        context.settlement = settle_gas(
            context.tx_gas_limit,
            context.intrinsic.calldata_floor,
            context.state_gas_reservoir,
            context.committed_spill,
            &context.resources,
            refund_counter,
        )?;
        system_log!(system, "Gas settlement: {:?}\n", &context.settlement);
        context.resources.exhaust_ergs();
        Ok(())
    }

    fn refund_and_commit_fee<'a, Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        transaction: &Transaction<S::Allocator>,
        context: &mut Self::TransactionContext,
        _tracer: &mut impl Tracer<S>,
    ) -> Result<(), BootloaderSubsystemError> {
        let GasSettlement {
            gas_used, gas_left, ..
        } = context.settlement;
        if gas_left > 0 {
            let receiver = transaction.from();
            let refund = context.tx_level_metadata.tx_gas_price * U256::from(gas_left);
            system_log!(
                system,
                "Will refund 0x{:040x} with {:?} native tokens\n",
                receiver.as_uint(),
                &refund
            );
            let mut inf_resources = S::Resources::FORMAL_INFINITE;
            system
                .io
                .update_account_nominal_token_balance(
                    ExecutionEnvironmentType::NoEE,
                    &mut inf_resources,
                    receiver,
                    &refund,
                    false,
                    Config::SIMULATION,
                )
                .map_err(|e| match e {
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

        if !context.priority_fee_per_gas.is_zero() {
            let fee = context.priority_fee_per_gas * U256::from(gas_used);
            let coinbase = system.get_coinbase();
            system_log!(system, "Coinbase's share of fee is {:?}\n", &fee);
            let mut inf_resources = S::Resources::FORMAL_INFINITE;
            system
                .io
                .update_account_nominal_token_balance(
                    ExecutionEnvironmentType::NoEE,
                    &mut inf_resources,
                    &coinbase,
                    &fee,
                    false,
                    Config::SIMULATION,
                )
                .map_err(|e| match e {
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
        }
        Ok(())
    }

    fn after_execution<'a, Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        transaction: Transaction<S::Allocator>,
        context: Self::TransactionContext,
        result: ExecutionResult<'a, <S as SystemTypes>::IOTypes>,
        transaction_data_keeper: &mut impl BlockTransactionsDataKeeper<S, Self>,
        _tracer: &mut impl Tracer<S>,
    ) -> Self::ExecutionResult<'a> {
        transaction_data_keeper.record_transaction_results(system, transaction, &context, &result);
        GlamsterdamTxResult {
            result,
            gas_used: context.settlement.gas_used,
        }
    }
}

/// Why the preparation of the dispatch failed
enum PreparationError {
    /// The transaction is included as failed
    OutOfGas,
    Internal(BootloaderSubsystemError),
}

impl From<BootloaderSubsystemError> for PreparationError {
    fn from(e: BootloaderSubsystemError) -> Self {
        Self::Internal(e)
    }
}

impl From<InternalError> for PreparationError {
    fn from(e: InternalError) -> Self {
        Self::Internal(e.into())
    }
}

/// Settles the gas of a finished transaction (EIP-8037 `settle_transaction_gas`): the gas
/// used before refunds is the limit less what came back in both dimensions; the refund is
/// capped at a fifth of it; the sender pays at least the calldata floor; the block counts
/// the state gas created net of refills and the execution gas before refunds, with the floor
/// on the execution side (EIP-7778).
pub fn settle_gas<R: StateGasResources>(
    gas_limit: u64,
    calldata_floor: u64,
    state_gas_reservoir: u32,
    committed_spill: u32,
    resources: &R,
    refund_counter: u64,
) -> Result<GasSettlement, InternalError> {
    let gas_left = resources.legacy_gas();
    let state_gas_left = resources.state_gas() as u64;
    let gas_used_before_refund = gas_limit
        .checked_sub(gas_left)
        .and_then(|g| g.checked_sub(state_gas_left))
        .ok_or(internal_error!("gas remaining > gas limit"))?;
    let gas_refund = core::cmp::min(gas_used_before_refund / 5, refund_counter);
    let gas_used_after_refund = gas_used_before_refund - gas_refund;
    let gas_used = core::cmp::max(gas_used_after_refund, calldata_floor);
    // the net state gas: refills can exceed the charges of the transaction
    let state_gas_used = (state_gas_reservoir as i64 - state_gas_left as i64
        + resources.state_gas_spilled() as i64
        + committed_spill as i64)
        .max(0) as u64;
    let execution_gas_used = core::cmp::max(
        gas_used_before_refund.saturating_sub(state_gas_used),
        calldata_floor,
    );
    if gas_used > gas_limit {
        return Err(internal_error!("gas used above the gas limit"));
    }
    Ok(GasSettlement {
        gas_used,
        gas_left: gas_limit - gas_used,
        execution_gas_used,
        state_gas_used,
    })
}

/// Whether an account is alive: it exists and is not empty
fn is_alive(nonce: u64, balance: &U256, has_bytecode: bool) -> bool {
    nonce != 0 || !balance.is_zero() || has_bytecode
}

impl<S: EthereumLikeSystem<Metadata = GlamsterdamBlockMetadata>> GlamsterdamTransactionFlow<S>
where
    S::IO: IOSubsystemExt,
    S::Resources: StateGasResources,
{
    /// The delegations (EIP-7702) with their state-dependent costs, committed, then the
    /// runtime costs of the dispatch: the creation of the deployed account or of a dead
    /// recipient of value (state gas), and the access of a delegate's code
    fn prepare_dispatch(
        system: &mut System<S>,
        transaction: &Transaction<S::Allocator>,
        context: &mut GlamsterdamTxContext<S>,
    ) -> Result<(), PreparationError> {
        let from = *transaction.from();
        let value = *transaction.value();
        let to = transaction.to();

        if let Some(auth_list) = transaction.authorization_list() {
            delegations::apply_delegations(
                system,
                &mut context.resources,
                auth_list,
                &from,
                to.filter(|_| !value.is_zero()),
            )?;
        }
        // The delegations stay if the code fails: their state gas is not refillable. The
        // creation charge of the target below is: EIP-8037 takes the baseline after the
        // authorization charges and before it, so a revert or halt of the code refills it
        // (and a halt burns the refill with the rest of the execution gas).
        context.committed_spill = context.resources.commit_state_gas();
        context.state_gas_baseline = context.resources.state_gas();

        let dispatch_target = match to {
            None => {
                use crate::bootloader::supported_ees::SystemBoundEVMInterpreter;
                SystemBoundEVMInterpreter::<S>::derive_address_for_deployment_create(
                    &mut context.resources,
                    &from,
                    context.originator_nonce_to_use,
                )
                .map_err(|e| {
                    let ee_error: EESubsystemError = wrap_error!(e);
                    PreparationError::Internal(wrap_error!(ee_error))
                })?
            }
            Some(to) => to,
        };
        // the recipient's access is in the intrinsic gas; the entry frame reads it free
        let target = context.resources.with_infinite_ergs(|inf_resources| {
            system.io.read_account_properties(
                ExecutionEnvironmentType::NoEE,
                inf_resources,
                &dispatch_target,
                AccountDataRequest::empty()
                    .with_nonce()
                    .with_nominal_token_balance()
                    .with_has_bytecode()
                    .with_is_delegated()
                    .with_bytecode()
                    .with_unpadded_code_len(),
            )
        })?;
        let target_alive = is_alive(
            target.nonce.0,
            &target.nominal_token_balance.0,
            target.has_bytecode(),
        );
        let creates_account = match to {
            None => !target_alive,
            Some(_) => !value.is_zero() && !target_alive,
        };
        if creates_account {
            context
                .resources
                .charge_state_gas(NEW_ACCOUNT)
                .map_err(out_of_gas)?;
        }
        if to.is_some() && target.is_delegated.0 {
            let delegation = &target.bytecode.0[..target.unpadded_code_len.0 as usize];
            let delegate = crate::bootloader::runner::parse_delegation(delegation)?;
            // the delegate's access, cold or warm (EIP-7702)
            system
                .io
                .touch_account(
                    ExecutionEnvironmentType::EVM,
                    &mut context.resources,
                    &delegate,
                )
                .map_err(out_of_gas)?;
        }
        Ok(())
    }

    fn execute_call<'a>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        memories: RunnerMemoryBuffers<'a>,
        transaction: &Transaction<S::Allocator>,
        context: &mut GlamsterdamTxContext<S>,
        tracer: &mut impl Tracer<S>,
        validator: &mut impl TxValidator<S>,
    ) -> Result<TxExecutionResult<'a, S>, BootloaderSubsystemError>
    where
        S: 'a,
    {
        let from = transaction.from();
        let main_calldata = transaction.calldata();
        let to = transaction.to().unwrap_or_default();
        let nominal_token_value = transaction.value();

        let resources = context.resources.take();

        let final_state = crate::bootloader::BasicBootloader::<S, Self>::run_single_interaction(
            system,
            system_functions,
            memories,
            main_calldata,
            from,
            &to,
            resources,
            nominal_token_value,
            true,
            tracer,
            validator,
        )?;

        let CompletedExecution {
            resources_returned,
            result,
        } = final_state;

        system_log!(system, "Resources to refund = {resources_returned:?}\n");
        context.resources.reclaim(resources_returned);

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
        context: &mut GlamsterdamTxContext<S>,
        to_ee_type: ExecutionEnvironmentType,
        tracer: &mut impl Tracer<S>,
        validator: &mut impl TxValidator<S>,
    ) -> Result<TxExecutionResult<'a, S>, BootloaderSubsystemError>
    where
        S: 'a,
    {
        use crate::bootloader::runner::run_till_completion;
        use crate::bootloader::supported_ees::SystemBoundEVMInterpreter;

        let mut resources = context.resources.take();
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
                    from,
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
            callers_caller: Default::default(),
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
        context.resources.reclaim(resources_returned);

        let (deployment_success, reverted, return_values, at) = match deployment_result {
            CallResult::Successful { mut return_values } => {
                // the deployed code is the return data of a deployment
                let deployed_bytecode = context.resources.with_infinite_ergs(|inf_resources| {
                    system
                        .io
                        .get_observable_bytecode(to_ee_type, inf_resources, &deployed_address)
                })?;
                return_values.returndata = deployed_bytecode;
                (true, false, return_values, Some(deployed_address))
            }
            CallResult::Failed { return_values, .. } => (false, true, return_values, None),
            CallResult::PreparationStepFailed => {
                return Err(internal_error!("Preparation step failed in root call").into())
            }
        };
        system.finish_global_frame(reverted.then_some(&rollback_handle))?;

        system_log!(
            system,
            "Deployment at {at:?} ended with success = {deployment_success}\n"
        );
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
        context: &mut GlamsterdamTxContext<S>,
        tracer: &mut impl Tracer<S>,
        validator: &mut impl TxValidator<S>,
    ) -> Result<ExecutionResult<'a, S::IOTypes>, BootloaderSubsystemError>
    where
        S: 'a,
    {
        system_log!(system, "Start of execution\n");

        let TxExecutionResult {
            return_values,
            reverted,
            deployed_address,
        } = if transaction.to().is_some() {
            Self::execute_call(
                system,
                system_functions,
                memories,
                transaction,
                context,
                tracer,
                validator,
            )?
        } else {
            Self::perform_deployment(
                system,
                system_functions,
                memories,
                transaction,
                context,
                ExecutionEnvironmentType::EVM,
                tracer,
                validator,
            )?
        };

        let returndata_region = return_values.returndata;
        let _ = system
            .get_logger()
            .log_data(returndata_region.iter().copied());
        system_log!(system, "Main TX body successful = {}\n", !reverted);

        let execution_result = match reverted {
            true => ExecutionResult::Revert {
                output: returndata_region,
            },
            false => match deployed_address {
                DeployedAddress::Address(at) => ExecutionResult::Success {
                    output: ExecutionOutput::Create(returndata_region, at),
                },
                _ => ExecutionResult::Success {
                    output: ExecutionOutput::Call(returndata_region),
                },
            },
        };
        Ok(execution_result)
    }
}

/// Maps a charge that failed to the preparation outcome
fn out_of_gas(e: SystemError) -> PreparationError {
    match e {
        SystemError::LeafRuntime(RuntimeError::OutOfErgs(_)) => PreparationError::OutOfGas,
        SystemError::LeafRuntime(RuntimeError::FatalRuntimeError(f)) => PreparationError::Internal(
            SystemError::LeafRuntime(RuntimeError::FatalRuntimeError(f)).into(),
        ),
        SystemError::LeafDefect(e) => PreparationError::Internal(e.into()),
    }
}

impl From<SystemError> for PreparationError {
    fn from(e: SystemError) -> Self {
        out_of_gas(e)
    }
}

/// The per-transaction bookkeeping of a transaction range: the running sums the receipts
/// and the header carry, and the last transaction's receipt data for the loop to check
/// against the witnessed receipt
#[derive(Debug)]
pub struct GlamsterdamTransactionDataKeeper {
    pub block_gas_limit: u64,
    /// Number of transactions recorded
    pub transaction_count: u32,
    /// The receipts' cumulative gas: after refunds and floor
    pub cumulative_gas_used: u64,
    /// The block's execution gas (EIP-8037 `block_execution_gas_used`)
    pub block_execution_gas_used: u64,
    /// The block's state gas
    pub block_state_gas_used: u64,
    pub block_blob_gas_used: u64,
    pub block_bloom: LogsBloom,
    /// Status, cumulative gas, number of logs and bloom of the last transaction
    pub last_receipt: Option<(bool, u64, usize, LogsBloom)>,
}

impl GlamsterdamTransactionDataKeeper {
    pub fn new(
        block_gas_limit: u64,
        cumulative_gas_used: u64,
        block_execution_gas_used: u64,
        block_state_gas_used: u64,
        block_blob_gas_used: u64,
        block_bloom: LogsBloom,
    ) -> Self {
        Self {
            block_gas_limit,
            transaction_count: 0,
            cumulative_gas_used,
            block_execution_gas_used,
            block_state_gas_used,
            block_blob_gas_used,
            block_bloom,
            last_receipt: None,
        }
    }

    /// The header's gas used so far: the fuller dimension (EIP-8037)
    pub fn block_gas_used(&self) -> u64 {
        core::cmp::max(self.block_execution_gas_used, self.block_state_gas_used)
    }
}

impl<S: EthereumLikeSystem<Metadata = GlamsterdamBlockMetadata>>
    BlockTransactionsDataKeeper<S, GlamsterdamTransactionFlow<S>>
    for GlamsterdamTransactionDataKeeper
where
    S::IO: IOSubsystemExt + IOTeardown<S::IOTypes>,
    S::Resources: StateGasResources,
{
    fn record_transaction_results(
        &mut self,
        system: &System<S>,
        _transaction: Transaction<<S as SystemTypes>::Allocator>,
        context: &GlamsterdamTxContext<S>,
        result: &ExecutionResult<'_, <S as SystemTypes>::IOTypes>,
    ) {
        let settlement = context.settlement;
        self.cumulative_gas_used += settlement.gas_used;
        self.block_execution_gas_used += settlement.execution_gas_used;
        self.block_state_gas_used += settlement.state_gas_used;
        self.block_blob_gas_used += context.blob_gas_used;
        let status = matches!(result, ExecutionResult::Success { .. });
        let mut bloom = LogsBloom::default();
        let events = system.io.events_in_this_tx_iterator();
        let num_events = events.len();
        let mut hasher = crypto::sha3::Keccak256::new();
        bloom.mark_events(&mut hasher, events);
        self.block_bloom.merge(&bloom);
        self.last_receipt = Some((status, self.cumulative_gas_used, num_events, bloom));
        self.transaction_count += 1;
    }

    /// EIP-8037 `check_block_gas_capacity`: each dimension against its remaining budget
    fn admit_transaction(&self, gas_limit: u64, blob_gas: u64) -> Result<(), InvalidTransaction> {
        let execution_available = self
            .block_gas_limit
            .saturating_sub(self.block_execution_gas_used);
        let state_available = self
            .block_gas_limit
            .saturating_sub(self.block_state_gas_used);
        if core::cmp::min(TX_MAX_GAS_LIMIT, gas_limit) > execution_available
            || gas_limit > state_available
        {
            return Err(InvalidTransaction::BlockGasLimitReached);
        }
        let blob_gas_available = system_blob_gas_limit().saturating_sub(self.block_blob_gas_used);
        if blob_gas > blob_gas_available {
            return Err(InvalidTransaction::BlockBlobGasLimitReached);
        }
        Ok(())
    }
}

/// The blob gas limit of a block under the range's blob schedule
fn system_blob_gas_limit() -> u64 {
    crate::bootloader::block_flow::ethereum_glamsterdam::tx_range::metadata_op::BLOB_SCHEDULE
        .max_blob_gas_per_block()
}

#[allow(dead_code)]
fn _address_type_check(_: B160) {}
