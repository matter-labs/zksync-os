use super::*;
use crate::bootloader::block_flow::ethereum::metadata_op::EthereumBlockMetadata;
use crate::bootloader::errors::BootloaderInterfaceError;
use crate::bootloader::errors::InvalidTransaction;
use crate::bootloader::supported_ees::errors::EESubsystemError;
use crate::bootloader::transaction_flow::BasicTransactionFlow;
use core::fmt::Write;
use core::ptr::addr_of_mut;
use crypto::MiniDigest;
use errors::cascade::CascadedError;
use errors::interface::InterfaceError;
use evm_interpreter::ERGS_PER_GAS;
use refund_calculation::compute_gas_refund;
use ruint::aliases::U256;
use tx_level_metadata::EthereumTransactionMetadata;
use zk_ee::common_structs::GenericEventContentRef;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::interface_error;
use zk_ee::internal_error;
use zk_ee::out_of_native_resources;
use zk_ee::storage_types::MAX_EVENT_TOPICS;
use zk_ee::system::errors::root_cause::GetRootCause;
use zk_ee::system::errors::root_cause::RootCause;
use zk_ee::system::errors::runtime::RuntimeError;
use zk_ee::system::errors::subsystem::SubsystemError;
use zk_ee::system::logger::Logger;
use zk_ee::system::*;
use zk_ee::system_log;
use zk_ee::types_config::EthereumIOTypesConfig;
use zk_ee::utils::Bytes32;
use zk_ee::wrap_error;

pub mod tx_level_metadata;

pub struct EthereumTransactionFlow<S: EthereumLikeTypes> {
    _marker: core::marker::PhantomData<S>,
}

#[derive(Debug)]
pub struct EthereumTxResult<'a> {
    pub result: ExecutionResult<'a, EthereumIOTypesConfig>,
    // pub tx_hash: Bytes32,
    pub gas_used: u64,
}

impl<'a> MinimalTransactionOutput<'a> for EthereumTxResult<'a> {
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
        unimplemented!("transaction hash is not computed for Ethereum STF");
        // self.tx_hash
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

mod validation_impl;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LogsBloom {
    inner: [u64; 32], // blindly the capacity for 2048 bits, treated as BE integer all together
}

impl LogsBloom {
    pub fn from_bytes(input: &[u8; 256]) -> Self {
        unsafe {
            let mut result = core::mem::MaybeUninit::<Self>::uninit();
            core::ptr::write(
                addr_of_mut!((*result.as_mut_ptr()).inner).cast::<[u8; 256]>(),
                *input,
            );

            result.assume_init()
        }
    }

    pub fn as_bytes(&self) -> &[u8; 256] {
        // We are overaligned and continuous
        unsafe { core::mem::transmute(self) }
    }
    fn as_bytes_mut(&mut self) -> &mut [u8; 256] {
        // We are overaligned and continuous
        unsafe { core::mem::transmute(self) }
    }
    pub fn mark_events<'a>(
        &mut self,
        hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
        events: impl Iterator<
            Item = GenericEventContentRef<'a, MAX_EVENT_TOPICS, EthereumIOTypesConfig>,
        >,
    ) {
        for event in events {
            self.mark_event(hasher, event);
        }
    }

    pub fn mark_event<'a>(
        &mut self,
        hasher: &mut impl MiniDigest<HashOutput = [u8; 32]>,
        event: GenericEventContentRef<'a, MAX_EVENT_TOPICS, EthereumIOTypesConfig>,
    ) {
        hasher.update(&event.address.to_be_bytes::<20>());
        let address_hash = hasher.finalize_reset();
        self.mark(&address_hash);
        for topic in event.topics.iter() {
            hasher.update(topic.as_u8_ref());
            let topic_hash = hasher.finalize_reset();
            self.mark(&topic_hash);
        }
    }

    fn mark(&mut self, hash: &[u8; 32]) {
        // take lowest 11 bits integer of each of 2-byte words BE words
        for i in [0, 2, 4] {
            let word = [hash[i], hash[i + 1]];
            let word = (u16::from_be_bytes(word) & 0x7ff) as usize; // equal to mod 2048
            let byte_idx = word / 8;
            let bit_idx = word % 8;
            self.as_bytes_mut()[255 - byte_idx] |= 1 << bit_idx;

            // let u64_idx = 31 - word / 64; // BE
            // let bit_idx = 63 - word % 64; // BE
            // self.inner[u64_idx] |= 1 << bit_idx;
        }
    }

    pub fn merge(&mut self, other: &Self) {
        for (dst, src) in self.inner.iter_mut().zip(other.inner.iter()) {
            *dst |= *src;
        }
    }
}

pub struct ResourcesForEthereumTx<S: EthereumLikeTypes> {
    pub main_resources: S::Resources,
}

impl<S: EthereumLikeTypes> core::fmt::Debug for ResourcesForEthereumTx<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ResourcesForEthereumTx")
            .field("gas", &(self.main_resources.ergs().0 / ERGS_PER_GAS))
            .field("main_resources", &self.main_resources)
            .finish()
    }
}

pub struct EthereumTxContext<S: EthereumLikeTypes> {
    pub resources: ResourcesForEthereumTx<S>,
    // pub tx_hash: Bytes32,
    pub fee_to_prepay: U256,
    pub priority_fee_per_gas: U256,
    pub minimal_gas_to_charge: u64,
    pub originator_nonce_to_use: u64,
    pub tx_gas_limit: u64,
    pub gas_used: u64,
    pub blob_gas_used: u64,
    pub tx_level_metadata: EthereumTransactionMetadata<{ MAX_BLOBS_PER_BLOCK }>,
}

impl<S: EthereumLikeTypes> core::fmt::Debug for EthereumTxContext<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TxContextForPreAndPostProcessing")
            .field("resources", &self.resources)
            // .field("tx_hash", &self.tx_hash)
            .field("fee_to_prepay", &self.fee_to_prepay)
            .field("priority_fee_per_gas", &self.priority_fee_per_gas)
            .field("minimal_gas_to_charge", &self.minimal_gas_to_charge)
            .field("originator_nonce_to_use", &self.originator_nonce_to_use)
            .field("tx_gas_limit", &self.tx_gas_limit)
            .field("gas_used", &self.gas_used)
            .field("blob_gas_used", &self.blob_gas_used)
            .field("tx_level_metadata", &self.tx_level_metadata)
            .finish()
    }
}

impl<S: EthereumLikeTypes<Metadata = EthereumBlockMetadata>> BasicTransactionFlow<S>
    for EthereumTransactionFlow<S>
where
    S::IO: IOSubsystemExt,
{
    type TransactionContext = EthereumTxContext<S>;

    type ExecutionBodyExtraData = (); // we can use context for everything

    fn process_l1_transaction<'a, Config: BasicBootloaderExecutionConfig>(
        _system: &mut System<S>,
        _system_functions: &mut HooksStorage<S, <S as SystemTypes>::Allocator>,
        _memories: RunnerMemoryBuffers<'a>,
        _transaction: &AbiEncodedTransaction<<S as SystemTypes>::Allocator>,
        _is_priority_op: bool,
        _tracer: &mut impl Tracer<S>,
        _validator: &mut impl TxValidator<S>,
    ) -> Result<Self::ExecutionResult<'a>, TxError>
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
        if let Some(to) = transaction.to() {
            system_log!(
                system,
                "Will try to process transaction to 0x{:040x} with gas limit of {} and value of {:?} and {} bytes of calldata\n",
                to.as_uint(),
                transaction.gas_limit(),
                transaction.value(),
                transaction.calldata().len(),
            );
        } else {
            system_log!(
                system,
                "Will try to process deployment with gas limit of {} and value of {:?} and {} bytes of calldata\n",
                transaction.gas_limit(),
                transaction.value(),
                transaction.calldata().len(),
            );
        }

        Ok(())
    }

    fn validate_and_prepare_context<Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        transaction: &mut Transaction<S::Allocator>,
        tracer: &mut impl Tracer<S>,
    ) -> Result<Self::TransactionContext, TxError> {
        self::validation_impl::validate_and_compute_fee_for_transaction::<S, Config>(
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
        if let Some(to) = transaction.to() {
            system_log!(
                system,
                "Will process transaction:\nCall from 0x{:040x} to 0x{:040x} with gas limit of {} and value of {:?} and {} bytes of calldata\n",
                transaction.from().as_uint(),
                to.as_uint(),
                transaction.gas_limit(),
                transaction.value(),
                transaction.calldata().len(),
            );
        } else {
            system_log!(
                system,
                "Will process transaction:\nDeployment from 0x{:040x} at nonce {} with gas limit of {} and value of {:?} and {} bytes of calldata\n",
                transaction.from().as_uint(),
                context.originator_nonce_to_use,
                transaction.gas_limit(),
                transaction.value(),
                transaction.calldata().len(),
            );
        }

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

        // let _ = system.get_logger().write_fmt(format_args!(
        //     "Balance of 0x{:040x} before transaction is {}\n",
        //     from.as_uint(),
        //     context
        //     .resources
        //     .main_resources
        //     .with_infinite_ergs(|resources| {
        //         system.io.get_nominal_token_balance(
        //             ExecutionEnvironmentType::NoEE, // out of scope of other interactions
        //             resources,
        //             from,
        //         ).unwrap()
        //     })
        // ));

        context
            .resources
            .main_resources
            .with_infinite_ergs(|resources| {
                system.io.update_account_nominal_token_balance(
                    ExecutionEnvironmentType::NoEE, // out of scope of other interactions
                    resources,
                    from,
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
                    RuntimeError::FatalRuntimeError(_) => {
                        TxError::oon_as_validation(out_of_native_resources!().into())
                    }
                    RuntimeError::OutOfErgs(_) => {
                        TxError::Validation(InvalidTransaction::OutOfGasDuringValidation)
                    }
                },
                SubsystemError::Cascaded(cascaded_error) => match cascaded_error {},
            })?;

        // let _ = system.get_logger().write_fmt(format_args!(
        //     "Balance of 0x{:040x} after precharge is {}\n",
        //     from.as_uint(),
        //     context
        //     .resources
        //     .main_resources
        //     .with_infinite_ergs(|resources| {
        //         system.io.get_nominal_token_balance(
        //             ExecutionEnvironmentType::NoEE, // out of scope of other interactions
        //             resources,
        //             from,
        //         ).unwrap()
        //     })
        // ));

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
        // Take a snapshot in case we need to revert due to out of native.
        let main_body_rollback_handle = system.start_global_frame()?;

        let execution_result = match Self::execute_or_deploy_inner(
            system,
            system_functions,
            memories,
            &transaction,
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
                    context.resources.main_resources.exhaust_ergs();
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
        transaction: &Transaction<S::Allocator>,
        context: &mut Self::TransactionContext,
        _result: &ExecutionResult<'a, <S as SystemTypes>::IOTypes>,
        _extra_data: Self::ExecutionBodyExtraData,
        _tracer: &mut impl Tracer<S>,
    ) -> Result<(), InternalError> {
        system_log!(
            system,
            "Have {:?} resources available before refund\n",
            &context.resources.main_resources,
        );

        let min_gas_used = context.minimal_gas_to_charge;
        // Compute gas used following the same logic as in normal execution

        let refund_info = compute_gas_refund(
            system,
            S::Resources::empty(),
            transaction.gas_limit(),
            min_gas_used,
            0u64,
            &mut context.resources.main_resources,
        )?;
        context.gas_used = refund_info.gas_used;

        Ok(())
    }

    fn refund_and_commit_fee<'a, Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        transaction: &Transaction<S::Allocator>,
        context: &mut Self::TransactionContext,
        _tracer: &mut impl Tracer<S>,
    ) -> Result<(), BootloaderSubsystemError> {
        // here we refund the user, then we will transfer fee to the operator

        // use would be refunded based on potentially one gas price, and operator will be paid using different one. But those
        // changes are not "transfers" in nature

        assert!(
            context.gas_used <= context.tx_gas_limit,
            "gas limit is {}, but {} gas is reported as used",
            context.tx_gas_limit,
            context.gas_used
        );

        if context.tx_gas_limit > context.gas_used {
            system_log!(
                system,
                "Gas price for refund is {:?}\n",
                &context.tx_level_metadata.tx_gas_price
            );

            // refund
            let receiver = transaction.from();
            let refund = context.tx_level_metadata.tx_gas_price
                * U256::from(context.tx_gas_limit - context.gas_used); // can not overflow

            system_log!(
                system,
                "Will refund 0x{:040x} with {:?} native tokens\n",
                receiver.as_uint(),
                &refund
            );

            // let _ = system.get_logger().write_fmt(format_args!(
            //     "Balance of 0x{:040x} before refund is {}\n",
            //     receiver.as_uint(),
            //     context
            //     .resources
            //     .main_resources
            //     .with_infinite_ergs(|resources| {
            //         system.io.get_nominal_token_balance(
            //             ExecutionEnvironmentType::NoEE, // out of scope of other interactions
            //             resources,
            //             receiver,
            //         ).unwrap()
            //     })
            // ));

            let mut inf_resources = S::Resources::FORMAL_INFINITE;
            // First refund the sender
            system
                .io
                .update_account_nominal_token_balance(
                    ExecutionEnvironmentType::NoEE,
                    &mut inf_resources,
                    &receiver,
                    &refund,
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

            // let _ = system.get_logger().write_fmt(format_args!(
            //     "Balance of 0x{:040x} after refund is {}\n",
            //     receiver.as_uint(),
            //     context
            //     .resources
            //     .main_resources
            //     .with_infinite_ergs(|resources| {
            //         system.io.get_nominal_token_balance(
            //             ExecutionEnvironmentType::NoEE, // out of scope of other interactions
            //             resources,
            //             receiver,
            //         ).unwrap()
            //     })
            // ));
        }

        assert!(context.gas_used > 0);

        if context.priority_fee_per_gas.is_zero() == false {
            system_log!(
                system,
                "Gas price for coinbase fee is {:?}\n",
                &context.priority_fee_per_gas
            );

            let fee = context.priority_fee_per_gas * U256::from(context.gas_used); // can not overflow
            let coinbase = system.get_coinbase();

            system_log!(system, "Coinbase's share of fee is {:?}\n", &fee);

            // let _ = system.get_logger().write_fmt(format_args!(
            //     "Balance of coinbase 0x{:040x} before fee collection is {}\n",
            //     coinbase.as_uint(),
            //     context
            //     .resources
            //     .main_resources
            //     .with_infinite_ergs(|resources| {
            //         system.io.get_nominal_token_balance(
            //             ExecutionEnvironmentType::NoEE, // out of scope of other interactions
            //             resources,
            //             &coinbase,
            //         ).unwrap()
            //     })
            // ));

            let mut inf_resources = S::Resources::FORMAL_INFINITE;
            system
                .io
                .update_account_nominal_token_balance(
                    ExecutionEnvironmentType::NoEE,
                    &mut inf_resources,
                    &coinbase,
                    &fee,
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

            // let _ = system.get_logger().write_fmt(format_args!(
            //     "Balance of coinbase 0x{:040x} after fee collection is {}\n",
            //     coinbase.as_uint(),
            //     context
            //     .resources
            //     .main_resources
            //     .with_infinite_ergs(|resources| {
            //         system.io.get_nominal_token_balance(
            //             ExecutionEnvironmentType::NoEE, // out of scope of other interactions
            //             resources,
            //             &coinbase,
            //         ).unwrap()
            //     })
            // ));
        }

        Ok(())
    }

    type ExecutionResult<'a> = EthereumTxResult<'a>;

    fn after_execution<'a, Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        transaction: Transaction<S::Allocator>,
        context: Self::TransactionContext,
        result: ExecutionResult<'a, <S as SystemTypes>::IOTypes>,
        transaction_data_keeper: &mut impl BlockTransactionsDataKeeper<S, Self>,
        _tracer: &mut impl Tracer<S>,
    ) -> Self::ExecutionResult<'a> {
        transaction_data_keeper.record_transaction_results(system, transaction, &context, &result);

        EthereumTxResult {
            result,
            // tx_hash: context.tx_hash,
            gas_used: context.gas_used,
        }
    }
}

impl<S: EthereumLikeTypes<Metadata = EthereumBlockMetadata>> EthereumTransactionFlow<S>
where
    S::IO: IOSubsystemExt,
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
        let to = transaction.to().unwrap_or_default();
        let nominal_token_value = transaction.value();

        let resources = context.resources.main_resources.take();

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
        context.resources.main_resources.reclaim(resources_returned);

        let reverted = result.failed();
        let return_values = result.return_values();
        Ok(TxExecutionResult {
            return_values,
            reverted,
            deployed_address: DeployedAddress::CallNoAddress,
        })
    }

    // Exact duplicate from zk stf
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
            // deployment
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

        Ok(execution_result)
    }
}
