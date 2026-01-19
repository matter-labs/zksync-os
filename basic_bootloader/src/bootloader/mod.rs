use errors::{BootloaderSubsystemError, InvalidTransaction};
use result_keeper::ResultKeeperExt;
use ruint::aliases::*;
use zk_ee::common_structs::system_hooks::HooksStorage;
use zk_ee::common_structs::MAX_NUMBER_OF_LOGS;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::system::tracer::Tracer;
use zk_ee::system::validator::TxValidator;
use zk_ee::system::{EthereumLikeTypes, System, SystemTypes};

pub mod block_flow;
pub mod run_single_interaction;
pub mod runner;
pub mod supported_ees;

pub mod transaction;
pub mod transaction_flow;

pub mod block_header;
pub mod config;
pub mod constants;
pub mod errors;
pub mod result_keeper;
mod rlp;
pub mod stf;

use crate::bootloader::block_flow::{
    MetadataInitOp, PostSystemInitOp, PostTxLoopOp, PreTxLoopOp, TxLoopOp,
};
use crate::bootloader::block_header::BlockHeader;
use crate::bootloader::config::BasicBootloaderExecutionConfig;
use crate::bootloader::errors::TxError;
use crate::bootloader::result_keeper::*;
use crate::bootloader::runner::RunnerMemoryBuffers;
use crate::bootloader::stf::EthereumLikeBasicSTF;
use crate::bootloader::transaction_flow::{BasicTransactionFlow, ExecutionOutput, ExecutionResult};
use alloc::boxed::Box;
use core::fmt::Write;

pub const MAX_HEAP_BUFFER_SIZE: usize = 1 << 27; // 128 MB
pub const MAX_RETURN_BUFFER_SIZE: usize = 1 << 28; // 256 MB

pub(crate) const EVM_EE_BYTE: u8 = ExecutionEnvironmentType::EVM_EE_BYTE;
pub const DEBUG_OUTPUT: bool = false;

/// Generic bootloader implementation using composable block execution flow.
///
/// This bootloader uses the State Transition Function (STF) trait to compose
/// different execution phases (metadata init, system init, transaction loop, finalization)
/// into a complete block execution pipeline.
pub struct BasicBootloader<S: EthereumLikeTypes, F: BasicTransactionFlow<S>>
where
    S::IO: IOSubsystemExt,
{
    _marker: core::marker::PhantomData<(S, F)>,
}

impl<S: EthereumLikeBasicSTF, F: BasicTransactionFlow<S>> BasicBootloader<S, F>
where
    S::IO: IOSubsystemExt,
{
    /// Runs the transactions that it loads from the oracle.
    /// This code runs both in sequencer (then it uses ForwardOracle - that stores data in local variables)
    /// and in prover (where oracle uses CRS registers to communicate).
    pub fn run_prepared<Config: BasicBootloaderExecutionConfig>(
        mut oracle: <S::IO as IOSubsystemExt>::IOOracle,
        result_keeper: &mut impl ResultKeeperExt<S::IOTypes, BlockHeader = S::BlockHeader>,
        tracer: &mut impl Tracer<S>,
        validator: &mut impl TxValidator<S>,
    ) -> Result<<S::IO as IOSubsystemExt>::FinalData, BootloaderSubsystemError>
    where
        S::IO: IOSubsystemExt,
    {
        cycle_marker::start!("process_block");
        // initialize the system
        cycle_marker::start!("system_init");

        let metadata = <S::MetadataOp as MetadataInitOp<S>>::metadata_op::<Config>(
            &mut oracle,
            S::Allocator::default(),
        )?;

        // we will model initial calldata buffer as just another "heap"
        let mut system: System<S> = System::init_from_metadata_and_oracle(metadata, oracle)?;
        let mut system_functions = HooksStorage::new_in(system.get_allocator());

        <S::PostSystemInitOp as PostSystemInitOp<S>>::post_init_op::<Config>(
            &mut system,
            &mut system_functions,
        )?;

        let mut heaps = Box::new_uninit_slice_in(MAX_HEAP_BUFFER_SIZE, system.get_allocator());
        let mut return_data =
            Box::new_uninit_slice_in(MAX_RETURN_BUFFER_SIZE, system.get_allocator());

        let memories = RunnerMemoryBuffers {
            heaps: &mut heaps,
            return_data: &mut return_data,
        };

        cycle_marker::end!("system_init");

        // Pre-op
        let mut block_data_keeper =
            <S::PreTxLoopOp as PreTxLoopOp<S>>::pre_op(&mut system, result_keeper);

        // TX loop
        <S::TxLoopOp as TxLoopOp<S>>::loop_op::<Config>(
            &mut system,
            &mut system_functions,
            memories,
            &mut block_data_keeper,
            result_keeper,
            tracer,
            validator,
        )?;

        // whatever the non-persistent data was there, it's now gone

        // Post-op

        // now we can run every transaction
        while let Some(r) = Self::try_begin_next_tx(&mut system) {
            match r {
                Err(err) => {
                    let _ = system.get_logger().write_fmt(format_args!(
                        "Failure while reading tx from oracle: decoding error = {err:?}\n",
                    ));
                    result_keeper.tx_processed(Err(InvalidTransaction::InvalidEncoding));
                }
                Ok(initial_calldata_buffer) => {
                    let mut inf_resources = S::Resources::FORMAL_INFINITE;
                    system
                        .io
                        .read_account_properties(
                            ExecutionEnvironmentType::NoEE,
                            &mut inf_resources,
                            &system.get_coinbase(),
                            AccountDataRequest::empty(),
                        )
                        .expect("must heat coinbase");

                    let mut logger: <S as SystemTypes>::Logger = system.get_logger();
                    let _ =
                        logger.write_fmt(format_args!("====================================\n"));
                    let _ = logger.write_fmt(format_args!("TX execution begins\n"));

                    tracer.begin_tx(initial_calldata_buffer.as_slice());

                    // Take a snapshot in case we need to invalidate the
                    // transaction to seal the block.
                    // This can happen if any of the block limits (native, gas, pubdata
                    // logs) is reached by the current transaction.
                    let pre_tx_rollback_handle = system.start_global_frame()?;

                    // We will give the full buffer here, and internally we will use parts of it to give forward to EEs
                    cycle_marker::start!("process_transaction");

                    let tx_result = Self::process_transaction::<Config>(
                        initial_calldata_buffer,
                        &mut system,
                        &mut system_functions,
                        memories.reborrow(),
                        first_tx,
                        tracer,
                        validator,
                    );

                    cycle_marker::end!("process_transaction");

                    tracer.finish_tx();
                    let post_validation = validator.finish_tx();

                    let tx_result = match tx_result {
                        Ok(ok) => post_validation
                            .map(|_| ok)
                            .map_err(|e| TxError::Validation(e.into())),
                        Err(err) => Err(err),
                    };

                    match tx_result {
                        Err(TxError::Internal(err)) => {
                            let _ = system.get_logger().write_fmt(format_args!(
                                "Tx execution result: Internal error = {err:?}\n",
                            ));
                            // Finish the frame opened before processing the tx
                            system.finish_global_frame(None)?;
                            return Err(err);
                        }
                        Err(TxError::Validation(err)) => {
                            let _ = system.get_logger().write_fmt(format_args!(
                                "Tx execution result: Validation error = {err:?}\n",
                            ));
                            // Revert to state before transaction
                            system.finish_global_frame(Some(&pre_tx_rollback_handle))?;
                            result_keeper.tx_processed(Err(err));
                        }
                        Ok(tx_processing_result) => {
                            // TODO: debug implementation for ruint types uses global alloc, which panics in ZKsync OS
                            #[cfg(not(target_arch = "riscv32"))]
                            let _ = system.get_logger().write_fmt(format_args!(
                                "Tx execution result = {:?}\n",
                                &tx_processing_result,
                            ));

                            // Check for service block invariants
                            Self::check_for_service_block_invariants(
                                &mut is_service_block,
                                first_tx,
                                tx_processing_result.is_service_tx,
                            )?;

                            // Do not update the accumulators yet, we may need to revert the transaction
                            let next_block_gas_used =
                                block_gas_used + tx_processing_result.gas_used;
                            let next_block_computational_native_used =
                                block_computational_native_used
                                    + tx_processing_result.computational_native_used;
                            let next_block_pubdata_used =
                                block_pubdata_used + tx_processing_result.pubdata_used;
                            let block_logs_used = system.io.logs_len();

                            // Check if the transaction made the block reach any of the limits
                            // for gas, native, pubdata or logs.
                            if let Err(err) = Self::check_for_block_limits(
                                &mut system,
                                next_block_gas_used,
                                next_block_computational_native_used,
                                next_block_pubdata_used,
                                block_logs_used,
                            ) {
                                // Revert to state before transaction
                                system.finish_global_frame(Some(&pre_tx_rollback_handle))?;
                                result_keeper.tx_processed(Err(err));
                            } else {
                                // Now update the accumulators
                                block_gas_used = next_block_gas_used;
                                block_computational_native_used =
                                    next_block_computational_native_used;
                                block_pubdata_used = next_block_pubdata_used;
                                first_tx = false;

                                // Finish the frame opened before processing the tx
                                system.finish_global_frame(None)?;

                                let (status, output, contract_address) =
                                    match tx_processing_result.result {
                                        ExecutionResult::Success { output } => match output {
                                            ExecutionOutput::Call(output) => (true, output, None),
                                            ExecutionOutput::Create(output, contract_address) => {
                                                (true, output, Some(contract_address))
                                            }
                                        },
                                        ExecutionResult::Revert { output } => (false, output, None),
                                    };
                                result_keeper.tx_processed(Ok(TxProcessingOutput {
                                    status,
                                    output: &output,
                                    contract_address,
                                    gas_used: tx_processing_result.gas_used,
                                    gas_refunded: tx_processing_result.gas_refunded,
                                    computational_native_used: tx_processing_result
                                        .computational_native_used,
                                    native_used: tx_processing_result.native_used,
                                    pubdata_used: tx_processing_result.pubdata_used,
                                }));

                                let mut keccak = crypto::sha3::Keccak256::new();
                                keccak.update(tx_rolling_hash);
                                keccak.update(tx_processing_result.tx_hash.as_u8_ref());
                                tx_rolling_hash = keccak.finalize();

                                if tx_processing_result.is_l1_tx {
                                    l1_to_l2_txs_hasher
                                        .update(tx_processing_result.tx_hash.as_u8_ref());
                                }

                                if tx_processing_result.is_upgrade_tx {
                                    upgrade_tx_hash = tx_processing_result.tx_hash;
                                }

                                // Only bump tx number when tx is successful
                                system.finish_valid_tx()?;
                            }
                        }
                    }

                    let mut logger = system.get_logger();
                    let _ = logger.write_fmt(format_args!("TX execution ends\n"));
                    let _ =
                        logger.write_fmt(format_args!("====================================\n"));
                }
            }
        }

        let block_number = system.get_block_number();

        let previous_block_hash = if block_number == 0 {
            Bytes32::ZERO
        } else {
            system.get_blockhash(block_number - 1)?
        };
        let beneficiary = system.get_coinbase();
        let gas_limit = system.get_gas_limit();
        let timestamp = system.get_timestamp();
        let consensus_random = system.get_mix_hash()?;
        let base_fee_per_gas = system.get_eip1559_basefee();
        // TODO: add pubdata price and native price
        let base_fee_per_gas = base_fee_per_gas
            .try_into()
            .map_err(|_| internal_error!("base_fee_per_gas exceeds max u64"))?;
        let block_header = BlockHeader::new(
            previous_block_hash,
            beneficiary,
            tx_rolling_hash.into(),
            block_number,
            gas_limit,
            block_gas_used,
            timestamp,
            consensus_random,
            base_fee_per_gas,
        );
        let block_hash = Bytes32::from(block_header.hash());
        result_keeper.block_sealed(block_header);

        let l1_to_l2_tx_hash = Bytes32::from(l1_to_l2_txs_hasher.finalize());

        // uses global alloc
        #[cfg(not(target_arch = "riscv32"))]
        cycle_marker::log_marker(
            alloc::format!(
                "Spent ergs for [run_prepared]: {}",
                result_keeper.get_gas_used() * evm_interpreter::ERGS_PER_GAS
            )
            .as_str(),
        );

        let _ = system
            .get_logger()
            .write_fmt(format_args!("Bootloader completed\n"));

        let mut logger = system.get_logger();
        let _ = logger.write_fmt(format_args!(
            "Bootloader execution is complete, will proceed with applying changes\n"
        ));

        let r = system.finish(block_hash, l1_to_l2_tx_hash, upgrade_tx_hash, result_keeper);
        cycle_marker::end!("run_prepared");
        #[allow(clippy::let_and_return)]
        res
    }
}
