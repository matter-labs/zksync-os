use super::*;
use crate::bootloader::BasicBootloader;
use crate::bootloader::InvalidTransaction;
use core::fmt::Write;
use zk_ee::system::EthereumLikeTypes;
use zk_ee::system_log;
use zk_ee::utils::UsizeAlignedByteBox;

impl<'a, S: EthereumLikeTypes + 'a, F: BasicTransactionFlow<S>> BasicBootloader<S, F>
where
    S::IO: IOSubsystemExt,
{
    /// Main transaction processing entrypoint
    pub fn process_transaction<Config: BasicBootloaderExecutionConfig>(
        initial_calldata_buffer: UsizeAlignedByteBox<S::Allocator>,
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        memories: RunnerMemoryBuffers<'a>,
        is_first_tx: bool,
        tracer: &mut impl Tracer<S>,
    ) -> Result<F::ExecutionResult<'a>, TxError> {
        let transaction = Transaction::try_from_buffer(initial_calldata_buffer, system)?;

        match &transaction {
            Transaction::Abi(zk_tx) => {
                if transaction.is_upgrade() {
                    if !is_first_tx {
                        Err(TxError::Validation(InvalidTransaction::UpgradeTxNotFirst))
                    } else {
                        F::process_l1_transaction::<Config>(
                            system,
                            system_functions,
                            memories,
                            zk_tx,
                            false,
                            tracer,
                        )
                    }
                } else if transaction.is_l1_l2() {
                    F::process_l1_transaction::<Config>(
                        system,
                        system_functions,
                        memories,
                        zk_tx,
                        true,
                        tracer,
                    )
                } else {
                    Self::process_l2_transaction::<Config>(
                        system,
                        system_functions,
                        memories,
                        transaction,
                        tracer,
                    )
                }
            }
            Transaction::Rlp(_) => Self::process_l2_transaction::<Config>(
                system,
                system_functions,
                memories,
                transaction,
                tracer,
            ),
        }
    }

    /// Generic processing of L2 transactions, based on the
    /// steps defined by the `BasicTransactionFlow` trait.
    pub fn process_l2_transaction<Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        memories: RunnerMemoryBuffers<'a>,
        mut transaction: Transaction<S::Allocator>,
        tracer: &mut impl Tracer<S>,
    ) -> Result<F::ExecutionResult<'a>, TxError>
    where
        S::IO: IOSubsystemExt,
    {
        F::before_validation(system, &transaction, tracer)?;

        let validation_rollback_handle = system.start_global_frame()?;

        // Tx validation
        let mut tx_context =
            match F::validate_and_prepare_context::<Config>(system, &mut transaction, tracer) {
                Ok(v) => v,
                Err(e) => {
                    system.finish_global_frame(Some(&validation_rollback_handle))?;
                    return Err(e);
                }
            };

        system_log!(
            system,
            "Transaction was validated and can be processed to collect fees\n"
        );

        F::before_fee_collection(system, &transaction, &tx_context, tracer)?;

        // Fee collection
        match F::precharge_fee::<Config>(system, &transaction, &mut tx_context, tracer) {
            Ok(_) => {
                system.finish_global_frame(None)?;
            }
            Err(e) => {
                system.finish_global_frame(Some(&validation_rollback_handle))?;
                return Err(e);
            }
        };
        drop(validation_rollback_handle);

        system_log!(system, "Fees were collected\n");

        F::before_execute_transaction_payload(system, &transaction, &mut tx_context, tracer)?;

        // Execute main body
        let (execution_result, extra_info) =
            F::create_frame_and_execute_transaction_payload::<Config>(
                system,
                system_functions,
                memories,
                &transaction,
                &mut tx_context,
                tracer,
            )?;

        F::before_refund::<Config>(
            system,
            &transaction,
            &mut tx_context,
            &execution_result,
            extra_info,
            tracer,
        )?;

        system_log!(system, "Start of refund\n");

        let refund_rollback_handle = system.start_global_frame()?;

        // Refund
        match F::refund_and_commit_fee::<Config>(system, &transaction, &mut tx_context, tracer) {
            Ok(_) => {
                system.finish_global_frame(None)?;
            }
            Err(e) => {
                system_log!(system, "Error on refund {:?}\n", &e);
                system.finish_global_frame(Some(&refund_rollback_handle))?;
                return Err(e.into());
            }
        }
        drop(refund_rollback_handle);

        let execution_result = F::after_execution::<Config>(
            system,
            &transaction,
            tx_context,
            execution_result,
            tracer,
        );

        Ok(execution_result)
    }
}
