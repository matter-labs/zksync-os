use super::*;

/// Trait for the transaction processing loop within a block.
///
/// This is the main execution phase that processes all transactions in the block,
/// validates them, applies state changes, and accumulates results. Handles both
/// successful transactions and validation failures with appropriate rollback logic.
pub trait TxLoopOp<S: SystemTypes>
where
    S::IO: IOSubsystemExt,
{
    /// Block-level data structure for tracking accumulated state
    type BlockDataKeeper;
    /// Batch-level data structure for tracking accumulated state
    type BatchDataKeeper;

    /// Executes the transaction processing loop for the entire block
    ///
    /// Reads transactions from oracle, validates, executes, and handles rollback
    /// for transactions that exceed block limits or fail validation.
    fn loop_op<'a, Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        memories: RunnerMemoryBuffers<'a>,
        block_data: &mut Self::BlockDataKeeper,
        batch_data: &mut Self::BatchDataKeeper,
        result_keeper: &mut impl ResultKeeperExt<S::IOTypes>,
        tracer: &mut impl Tracer<S>,
        validator: &mut impl TxValidator<S>,
    ) -> Result<(), BootloaderSubsystemError>;
}
