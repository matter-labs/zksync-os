use super::*;

/// Trait for finalization operations after all transactions have been processed.
///
/// This phase can be used to construct the final block header, calculate block hash, and
/// commit all state changes.
pub trait PostTxLoopOp<S: SystemTypes>
where
    S::IO: IOSubsystemExt,
{
    /// Block-level data accumulated during block processing
    type BlockDataKeeper;
    /// Block header structure for this STF
    type BlockHeader: 'static + Sized;

    /// Finalizes block execution
    fn post_op(
        system: System<S>,
        block_data: Self::BlockDataKeeper,
        result_keeper: &mut impl ResultKeeperExt<S::IOTypes, BlockHeader = Self::BlockHeader>,
    ) -> Result<<S::IO as IOSubsystemExt>::FinalData, BootloaderSubsystemError>;
}
