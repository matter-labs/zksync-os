use super::*;
use zk_ee::system::IOTeardown;

/// Trait for finalization operations after all transactions have been processed.
///
/// This phase can be used to construct the final block header, calculate block hash, and
/// commit all state changes.
pub trait PostTxLoopOp<S: SystemTypes>
where
    S::IO: IOSubsystemExt + IOTeardown<S::IOTypes>,
{
    /// Type, which post op returns
    type PostTxLoopOpResult;
    /// Block-level data accumulated during block processing
    type BlockDataKeeper;
    /// Batch-level data accumulated during batch processing
    type BatchDataKeeper;
    /// Block header structure for this STF
    type BlockHeader: 'static + Sized;

    /// Finalizes block execution
    fn post_op(
        system: System<S>,
        block_data: Self::BlockDataKeeper,
        batch_data: &mut Self::BatchDataKeeper,
        result_keeper: &mut impl ResultKeeperExt<S::IOTypes, BlockHeader = Self::BlockHeader>,
    ) -> Result<Self::PostTxLoopOpResult, BootloaderSubsystemError>;
}
