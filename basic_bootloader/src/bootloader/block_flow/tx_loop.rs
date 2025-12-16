use super::*;

pub trait TxLoopOp<S: SystemTypes>
where
    S::IO: IOSubsystemExt,
{
    type BlockData;

    fn loop_op<'a, Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        memories: RunnerMemoryBuffers<'a>,
        block_data: &mut Self::BlockData,
        result_keeper: &mut impl ResultKeeperExt<S::IOTypes>,
        tracer: &mut impl Tracer<S>,
    ) -> Result<(), BootloaderSubsystemError>;
}
