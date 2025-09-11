use super::*;
use crate::bootloader::block_flow::ethereum_block_flow::EthereumBlockMetadata;
use crate::bootloader::block_flow::ethereum_block_flow::*;
use crate::bootloader::errors::InvalidTransaction;
use crate::bootloader::transaction::ethereum_tx_format::EthereumTransactionMetadata;
use crate::bootloader::transaction::ethereum_tx_format::EthereumTransactionWithBuffer;
use crate::bootloader::transaction_flow::BasicTransactionFlow;
use crate::bootloader::BasicBootloader;
use core::fmt::Write;
use core::ptr::addr_of_mut;
use crypto::MiniDigest;
use evm_interpreter::ERGS_PER_GAS;
use ruint::aliases::U256;
use zk_ee::common_structs::GenericEventContentRef;
use zk_ee::internal_error;
use zk_ee::kv_markers::MAX_EVENT_TOPICS;
use zk_ee::out_of_native_resources;
use zk_ee::system::errors::root_cause::GetRootCause;
use zk_ee::system::errors::root_cause::RootCause;
use zk_ee::system::errors::runtime::RuntimeError;
use zk_ee::system::errors::subsystem::SubsystemError;
use zk_ee::system::logger::Logger;
use zk_ee::system::*;
use zk_ee::types_config::EthereumIOTypesConfig;
use zk_ee::utils::Bytes32;
use zk_ee::utils::UsizeAlignedByteBox;

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
    type Transaction<'a> = EthereumTransactionWithBuffer<S::Allocator>;
    type TransactionContext = EthereumTxContext<S>;
    type ExecutionBodyExtraData = (); // we can use context for everything

    type ScratchSpace = ();
    fn create_tx_loop_scratch_space(_system: &mut System<S>) -> Self::ScratchSpace {}

    type TransactionBuffer<'a> = UsizeAlignedByteBox<S::Allocator>;
    fn try_begin_next_tx<'a>(
        system: &'_ mut System<S>,
        _scratch_space: &'a mut Self::ScratchSpace,
    ) -> Option<Self::TransactionBuffer<'a>> {
        let allocator = system.get_allocator();
        let (tx_length_in_bytes, mut buffer) = system
            .try_begin_next_tx_with_constructor(move |tx_length_in_bytes| {
                UsizeAlignedByteBox::preallocated_in(tx_length_in_bytes, allocator)
            })
            .expect("TX start call must always succeed")?;
        buffer.truncated_to_byte_length(tx_length_in_bytes);

        Some(buffer)
    }

    fn parse_transaction<'a>(
        system: &System<S>,
        source: Self::TransactionBuffer<'a>,
        _tracer: &mut impl Tracer<S>,
    ) -> Result<Self::Transaction<'a>, TxError> {
        let chain_id = system
            .get_chain_id()
            .try_into()
            .expect("too large chain ID");
        EthereumTransactionWithBuffer::parse_from_buffer(source, chain_id)
            .map_err(|_| TxError::Validation(InvalidTransaction::InvalidEncoding))
    }

    fn before_validation<'a>(
        system: &System<S>,
        transaction: &Self::Transaction<'a>,
        _tracer: &mut impl Tracer<S>,
    ) -> Result<(), TxError> {
        if let Some(to) = transaction.destination() {
            let _ = system.get_logger().write_fmt(
                format_args!(
                    "Will try to process transaction to 0x{:040x} with gas limit of {} and value of {:?} and {} bytes of calldata\n",
                    to.as_uint(),
                    transaction.gas_limit(),
                    transaction.value(),
                    transaction.calldata().len(),
                )
            );
        } else {
            let _ = system.get_logger().write_fmt(
                format_args!(
                    "Will try to process deployment with gas limit of {} and value of {:?} and {} bytes of calldata\n",
                    transaction.gas_limit(),
                    transaction.value(),
                    transaction.calldata().len(),
                )
            );
        }

        Ok(())
    }

    fn validate_and_prepare_context<'a, Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        transaction: Self::Transaction<'a>,
        tracer: &mut impl Tracer<S>,
    ) -> Result<(Self::TransactionContext, Self::Transaction<'a>), TxError> {
        self::validation_impl::validate_and_compute_fee_for_transaction::<S, Config, _>(
            system,
            transaction,
            tracer,
        )
    }

    fn before_fee_collection<'a>(
        system: &System<S>,
        transaction: &Self::Transaction<'a>,
        context: &Self::TransactionContext,
        _tracer: &mut impl Tracer<S>,
    ) -> Result<(), TxError> {
        if let Some(to) = transaction.destination() {
            let _ = system.get_logger().write_fmt(
                format_args!(
                    "Will process transaction:\nCall from 0x{:040x} to 0x{:040x} with gas limit of {} and value of {:?} and {} bytes of calldata\n",
                    transaction.signer().as_uint(),
                    to.as_uint(),
                    transaction.gas_limit(),
                    transaction.value(),
                    transaction.calldata().len(),
                )
            );
        } else {
            let _ = system.get_logger().write_fmt(
                format_args!(
                    "Will process transaction:\nDeployment from 0x{:040x} at nonce {} with gas limit of {} and value of {:?} and {} bytes of calldata\n",
                    transaction.signer().as_uint(),
                    context.originator_nonce_to_use,
                    transaction.gas_limit(),
                    transaction.value(),
                    transaction.calldata().len(),
                )
            );
        }

        Ok(())
    }

    fn precharge_fee<Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        transaction: &Self::Transaction<'_>,
        context: &mut Self::TransactionContext,
        _tracer: &mut impl Tracer<S>,
    ) -> Result<(), TxError> {
        let from = transaction.signer();
        let value = context.fee_to_prepay;

        let _ = system.get_logger().write_fmt(format_args!(
            "Will precharge 0x{:040x} with {:?} native tokens for transaction\n",
            from.as_uint(),
            &value
        ));

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
        _transaction: &Self::Transaction<'a>,
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
        transaction: &Self::Transaction<'_>,
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

        let execution_result = match Self::execute_or_deploy_inner::<Config>(
            system,
            system_functions,
            memories,
            &transaction,
            context,
            tracer,
        ) {
            Ok(r) => {
                match r {
                    ExecutionResult::Success { .. } => {
                        system.finish_global_frame(None)?;
                        let _ = system
                            .get_logger()
                            .write_fmt(format_args!("Transaction main payload was processed\n"));
                    }
                    ExecutionResult::Revert { .. } => {
                        system.finish_global_frame(Some(&main_body_rollback_handle))?;
                        let _ = system
                            .get_logger()
                            .write_fmt(format_args!("Transaction main payload was reverted\n"));
                    }
                };

                r
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
        transaction: &Self::Transaction<'a>,
        context: &mut Self::TransactionContext,
        _result: &ExecutionResult<'a, <S as SystemTypes>::IOTypes>,
        _extra_data: Self::ExecutionBodyExtraData,
        _tracer: &mut impl Tracer<S>,
    ) -> Result<(), InternalError> {
        let _ = system.get_logger().write_fmt(format_args!(
            "Have {:?} resources available before refund\n",
            &context.resources.main_resources,
        ));

        let min_gas_used = context.minimal_gas_to_charge;
        // Compute gas used following the same logic as in normal execution

        let (_gas_refund, gas_used, _evm_refund) = BasicBootloader::<S>::compute_gas_refund(
            system,
            S::Resources::empty(),
            transaction.gas_limit(),
            min_gas_used,
            U256::ZERO,
            &mut context.resources.main_resources,
        )?;
        context.gas_used = gas_used;

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
                &context.tx_level_metadata.tx_gas_price
            ));

            // refund
            let receiver = transaction.signer();
            let refund = context.tx_level_metadata.tx_gas_price
                * U256::from(context.tx_gas_limit - context.gas_used); // can not overflow

            let _ = system.get_logger().write_fmt(format_args!(
                "Will refund 0x{:040x} with {:?} native tokens\n",
                receiver.as_uint(),
                &refund
            ));

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

        if context.priority_fee_per_gas.is_zero() == false {
            let _ = system.get_logger().write_fmt(format_args!(
                "Gas price for coinbase fee is {:?}\n",
                &context.priority_fee_per_gas
            ));

            let fee = context.priority_fee_per_gas * U256::from(context.gas_used); // can not overflow
            let coinbase = system.get_coinbase();

            let _ = system
                .get_logger()
                .write_fmt(format_args!("Coinbase's share of fee is {:?}\n", &fee));

            inf_resources.with_infinite_ergs(|resources| {
                system.io.update_account_nominal_token_balance(
                    ExecutionEnvironmentType::NoEE, // out of scope of other interactions
                    resources,
                    &coinbase,
                    &fee,
                    false,
                )
            })?;
        }

        Ok(())
    }

    type ExecutionResult<'a> = EthereumTxResult<'a>;

    fn after_execution<'a, Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        transaction: Self::Transaction<'_>,
        context: Self::TransactionContext,
        result: ExecutionResult<'a, <S as SystemTypes>::IOTypes>,
        transaction_data_collector: &mut impl BlockTransactionsDataCollector<S, Self>,
        _tracer: &mut impl Tracer<S>,
    ) -> Self::ExecutionResult<'a> {
        transaction_data_collector.record_transaction_results(
            &*system,
            transaction,
            &context,
            &result,
        );

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
        let from = transaction.signer();
        let main_calldata = transaction.calldata();
        let to = transaction.destination().unwrap_or_default();
        let nominal_token_value = transaction.value();

        let resources = context.resources.main_resources.take();

        let final_state = BasicBootloader::<S>::run_single_interaction(
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

        assert!(transaction.destination().is_none());

        let from = transaction.signer();
        let main_calldata = transaction.calldata();
        let nominal_token_value = *transaction.value();

        let resources = context.resources.main_resources.take();

        let deployment_parameters = DeploymentPreparationParameters {
            address_of_deployer: *from,
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
    ) -> Result<ExecutionResult<'a, S::IOTypes>, BootloaderSubsystemError>
    where
        S: 'a,
    {
        let _ = system
            .get_logger()
            .write_fmt(format_args!("Start of execution\n"));

        let TxExecutionResult {
            return_values,
            reverted,
            deployed_address,
        } = if transaction.destination().is_some() {
            Self::execute_call::<Config>(
                system,
                system_functions,
                memories,
                transaction,
                context,
                tracer,
            )?
        } else {
            // deployment
            Self::perform_deployment::<Config>(
                system,
                system_functions,
                memories,
                transaction,
                context,
                ExecutionEnvironmentType::EVM,
                tracer,
            )?
        };

        let returndata_region = return_values.returndata;
        let _ = system
            .get_logger()
            .log_data(returndata_region.iter().copied());

        let _ = system
            .get_logger()
            .write_fmt(format_args!("Main TX body successful = {}\n", !reverted));

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

        let _ = system
            .get_logger()
            .write_fmt(format_args!("Transaction execution completed\n"));

        Ok(execution_result)
    }
}
