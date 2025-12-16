use super::*;

pub trait PreTxLoopOp<S: SystemTypes>
where
    S::IO: IOSubsystemExt,
{
    type PreTxLoopResult;

    fn pre_op(
        system: &mut System<S>,
        result_keeper: &mut impl IOResultKeeper<S::IOTypes>,
    ) -> Self::PreTxLoopResult;
}
