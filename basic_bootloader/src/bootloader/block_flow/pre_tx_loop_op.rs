use super::*;

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
        result_keeper: &mut impl IOResultKeeper<S::IOTypes>,
    ) -> Self::PreTxLoopResult;
}
