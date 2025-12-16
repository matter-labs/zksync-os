use super::*;

pub trait PostTxLoopOp<S: SystemTypes>
where
    S::IO: IOSubsystemExt,
{
    type BlockData;
    type BlockHeader: 'static + Sized;

    fn post_op(
        system: System<S>,
        block_data: Self::BlockData,
        result_keeper: &mut impl ResultKeeperExt<S::IOTypes, BlockHeader = Self::BlockHeader>,
    ) -> Result<<S::IO as IOSubsystemExt>::FinalData, BootloaderSubsystemError>;
}
