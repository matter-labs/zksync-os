use zk_ee::{system::AccountDataRequest, utils::UsizeAlignedByteBox};

use super::*;
use crate::bootloader::{
    block_flow::tx_loop::TxLoopOp, transaction_flow::zk::ZkTransactionFlowOnlyEOA,
};
use zk_ee::system::Resource;

impl<
        S: EthereumLikeTypes<Metadata = zk_ee::system::metadata::zk_metadata::ZkMetadata>,
        BlockEA: TxHashesAccumulator,
        BatchEA: TxHashesAccumulator,
    > TxLoopOp<S> for ZKHeaderStructureTxLoop<BlockEA, BatchEA>
where
    S::IO: IOSubsystemExt,
    S::Metadata: ZkSpecificPricingMetadata,
{
    type BlockDataKeeper = ZKBasicBlockDataKeeper<BlockEA>;
    // we write only enforced tx hashes to the batch data, so it can be anything that implements tx hashes accumulator
    type BatchDataKeeper = BatchEA;

    fn loop_op<'a, Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        mut memories: RunnerMemoryBuffers<'a>,
        block_data: &mut Self::BlockDataKeeper,
        batch_data: &mut Self::BatchDataKeeper,
        result_keeper: &mut impl ResultKeeperExt<EthereumIOTypesConfig>,
        tracer: &mut impl Tracer<S>,
        validator: &mut impl TxValidator<S>,
    ) -> Result<(), BootloaderSubsystemError> {
        cycle_marker::start!("run_tx_loop");

        let mut is_first_tx = true;
        // Service blocks are blocks that only contain service transactions.
        // Service transactions can only be included in service blocks,
        // unless the first tx in the block was an upgrade tx.
        let mut is_service_block = false;
        let mut first_tx_was_upgrade = false;

        // TODO use preallocated data buffer?

        // now we can run every transaction
        while let Some(r) = {
            let allocator = system.get_allocator();
            system.try_begin_next_tx(move |tx_length_in_bytes| {
                UsizeAlignedByteBox::preallocated_in(tx_length_in_bytes, allocator)
            })
        } {
            match r {
                Err(err) => {
                    system_log!(
                        system,
                        "Failure while reading tx from oracle: decoding error = {err:?}\n",
                    );
                    result_keeper.tx_processed(Err(InvalidTransaction::InvalidEncoding));
                }
                Ok((_next_tx_len_bytes, initial_calldata_buffer)) => {
                    // warm up the coinbase formally
                    {
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
                    }

                    system_log!(system, "====================================\n",);
                    system_log!(system, "TX execution begins\n");

                    tracer.begin_tx(initial_calldata_buffer.as_slice());

                    // Take a snapshot in case we need to invalidate the
                    // transaction to seal the block.
                    // This can happen if any of the block limits (native, gas, pubdata
                    // logs) is reached by the current transaction.
                    let pre_tx_rollback_handle = system.start_global_frame()?;

                    // We will give the full buffer here, and internally we will use parts of it to give forward to EEs
                    cycle_marker::start!("process_transaction");

                    // TODO: consider actually using block_data here
                    let mut nop_keeper = NopTransactionDataKeeper;

                    let tx_result =
                        BasicBootloader::<S, ZkTransactionFlowOnlyEOA<S>>::process_transaction::<
                            Config,
                        >(
                            initial_calldata_buffer,
                            system,
                            system_functions,
                            memories.reborrow(),
                            is_first_tx,
                            &mut nop_keeper,
                            tracer,
                            validator,
                        );

                    cycle_marker::end!("process_transaction");

                    tracer.finish_tx();

                    match tx_result {
                        Err(TxError::Internal(err)) => {
                            system_log!(system, "Tx execution result: Internal error = {err:?}\n",);
                            // Finish the frame opened before processing the tx
                            system.finish_global_frame(None)?; // TODO should we use pre_tx_rollback_handle here?
                            return Err(err);
                        }
                        Err(TxError::Validation(err)) => {
                            system_log!(
                                system,
                                "Tx execution result: Validation error = {err:?}\n",
                            );
                            // Revert to state before transaction
                            system.finish_global_frame(Some(&pre_tx_rollback_handle))?;
                            result_keeper.tx_processed(Err(err));
                        }
                        Ok(tx_processing_result) => {
                            system_log!(
                                system,
                                "Tx execution result = {:?}\n",
                                &tx_processing_result,
                            );

                            // Check for service block invariants
                            check_for_service_block_invariants(
                                &mut is_service_block,
                                is_first_tx,
                                tx_processing_result.is_service_tx,
                                first_tx_was_upgrade,
                            )?;

                            if is_first_tx && tx_processing_result.is_upgrade_tx {
                                first_tx_was_upgrade = true;
                            }

                            // Do not update the accumulators yet, we may need to revert the transaction
                            let next_block_gas_used =
                                block_data.block_gas_used + tx_processing_result.gas_used;
                            let next_block_computational_native_used = block_data
                                .block_computational_native_used
                                + tx_processing_result.computational_native_used;
                            let next_block_pubdata_used =
                                block_data.block_pubdata_used + tx_processing_result.pubdata_used;
                            let block_logs_used = system.io.logs_len();
                            let next_block_blob_gas_used =
                                block_data.block_blob_gas_used + tx_processing_result.blob_gas_used;

                            // Check if the transaction made the block reach any of the limits
                            // for gas, native, pubdata or logs.
                            if let Err(err) = check_for_block_limits(
                                system,
                                next_block_gas_used,
                                next_block_computational_native_used,
                                next_block_pubdata_used,
                                block_logs_used,
                                next_block_blob_gas_used,
                            ) {
                                // Revert to state before transaction
                                system.finish_global_frame(Some(&pre_tx_rollback_handle))?;
                                result_keeper.tx_processed(Err(err));
                            } else {
                                // Now update the accumulators
                                block_data.block_gas_used = next_block_gas_used;
                                block_data.block_computational_native_used =
                                    next_block_computational_native_used;
                                block_data.block_pubdata_used = next_block_pubdata_used;
                                block_data.block_blob_gas_used = next_block_blob_gas_used;
                                is_first_tx = false;

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

                                block_data
                                    .transaction_hashes_accumulator
                                    .add_tx_hash(&tx_processing_result.tx_hash);
                                if tx_processing_result.is_priority_tx {
                                    block_data
                                        .enforced_transaction_hashes_accumulator
                                        .add_tx_hash(&tx_processing_result.tx_hash);
                                    batch_data.add_tx_hash(&tx_processing_result.tx_hash);
                                }
                                if tx_processing_result.is_upgrade_tx {
                                    block_data
                                        .upgrade_tx_recorder
                                        .add_upgrade_tx_hash(&tx_processing_result.tx_hash);
                                }
                                block_data.current_transaction_number += 1;

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

                                // Only bump tx number when tx is successful
                                system.finish_valid_tx()?;
                            }
                        }
                    }

                    system_log!(system, "TX execution ends\n");
                    system_log!(system, "====================================\n");
                }
            }
        }

        cycle_marker::end!("run_tx_loop");

        Ok(())
    }
}
