use super::*;
use crate::bootloader::errors::BootloaderSubsystemError;
use crate::bootloader::runner::RunnerMemoryBuffers;
use zk_ee::common_structs::system_hooks::HooksStorage;
use zk_ee::system::IOResultKeeper;

/// Trait for operations performed before the transaction processing loop begins.
pub trait PreTxLoopOp<S: SystemTypes>
where
    S::IO: IOSubsystemExt,
{
    /// Structure that is created during this step
    type PreTxLoopResult;

    /// Performs pre-transaction-loop setup
    fn pre_op(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        memories: RunnerMemoryBuffers<'_>,
        result_keeper: &mut impl IOResultKeeper<S::IOTypes>,
    ) -> Result<Self::PreTxLoopResult, BootloaderSubsystemError>;
}
