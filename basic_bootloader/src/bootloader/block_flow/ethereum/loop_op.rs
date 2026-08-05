use super::*;
use crate::bootloader::block_flow::tx_loop::TxLoopOp;
use crate::bootloader::transaction_flow::ethereum::EthereumTransactionFlow;
use crate::bootloader::transaction_flow::MinimalTransactionOutput;
use evm_interpreter::precompile_addresses::PRECOMPILE_ADDRESSES_LOWS;
use zk_ee::system::Resource;
use zk_ee::system::{AccountDataRequest, IOTeardown};
use zk_ee::system_log;
use zk_ee::types_config::EthereumIOTypesConfig;
use zk_ee::utils::UsizeAlignedByteBox;

impl<S: EthereumLikeTypes<Metadata = EthereumBlockMetadata>> TxLoopOp<S> for EthereumLoopOp
where
    S::IO: IOSubsystemExt + IOTeardown<S::IOTypes>,
{
    type BlockDataKeeper = EthereumBasicTransactionDataKeeper<S::Allocator, S::Allocator>;
    type BatchDataKeeper = ();

    fn loop_op<'a, Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        memories: RunnerMemoryBuffers<'a>,
        block_data: &mut Self::BlockDataKeeper,
        _batch_data: &mut Self::BatchDataKeeper,
        result_keeper: &mut impl ResultKeeperExt<EthereumIOTypesConfig>,
        tracer: &mut impl Tracer<S>,
        validator: &mut impl TxValidator<S>,
    ) -> Result<(), BootloaderSubsystemError> {
        generic_loop_op::<S, Config, EthereumTransactionFlow<S>>(
            system,
            system_functions,
            memories,
            block_data,
            result_keeper,
            tracer,
            validator,
        )
    }
}

// TODO: unify with ZK version
pub fn generic_loop_op<
    'a,
    S: EthereumLikeTypes,
    Config: BasicBootloaderExecutionConfig,
    F: BasicTransactionFlow<S>,
>(
    system: &mut System<S>,
    system_functions: &mut HooksStorage<S, S::Allocator>,
    mut memories: RunnerMemoryBuffers<'a>,
    block_data_keeper: &mut impl BlockTransactionsDataKeeper<S, F>,
    result_keeper: &mut impl ResultKeeperExt<S::IOTypes>,
    tracer: &mut impl Tracer<S>,
    validator: &mut impl TxValidator<S>,
) -> Result<(), BootloaderSubsystemError>
where
    S::IO: IOSubsystemExt + IOTeardown<S::IOTypes>,
{
    cycle_marker::start!("run_tx_loop");

    let mut tx_counter = 0;

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

                system_log!(system, "====================================\n");
                system_log!(system, "TX execution begins for transaction {tx_counter}\n");
                // all EVM precompiles must be formally warm
                {
                    for &address_low in PRECOMPILE_ADDRESSES_LOWS {
                        let address = B160::from_limbs([address_low as u64, 0, 0]);
                        let mut inf_resources = S::Resources::FORMAL_INFINITE;
                        system
                            .io
                            .touch_account(
                                ExecutionEnvironmentType::NoEE,
                                &mut inf_resources,
                                &address,
                            )
                            .expect("must warm up precompile");
                    }
                }

                tracer.begin_tx(initial_calldata_buffer.as_slice());

                // We will give the full buffer here, and internally we will use parts of it to give forward to EEs
                cycle_marker::start!("process_transaction");

                let tx_result = BasicBootloader::<S, F>::process_transaction::<Config>(
                    initial_calldata_buffer,
                    system,
                    system_functions,
                    memories.reborrow(),
                    tx_counter == 0,
                    block_data_keeper,
                    tracer,
                    validator,
                );

                cycle_marker::end!("process_transaction");

                tracer.finish_tx();

                match tx_result {
                    Err(TxError::Internal(err)) => {
                        system_log!(system, "Tx execution result: Internal error = {err:?}\n");
                        return Err(err);
                    }
                    Err(TxError::Validation(err)) => {
                        system_log!(system, "Tx execution result: Validation error = {err:?}\n");
                        result_keeper.tx_processed(Err(err));
                    }
                    Ok(result) => {
                        let tx_processing_result = result.into_bookkeeper_output();
                        system_log!(
                            system,
                            "Tx execution result = {:?}\n",
                            &tx_processing_result
                        );
                        // anything that is not related to actual validity
                        result_keeper.tx_processed(Ok(tx_processing_result));
                        system.finish_valid_tx()?;
                    }
                }

                system_log!(system, "TX execution ends for transaction {tx_counter}\n");
                system_log!(system, "====================================\n");

                tx_counter += 1;
            }
        }
    }

    system_log!(system, "Bootloader completed\n");
    system_log!(
        system,
        "Bootloader execution is complete, will proceed with applying changes\n"
    );

    cycle_marker::end!("run_tx_loop");

    Ok(())
}
