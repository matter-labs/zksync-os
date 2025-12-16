use super::*;
use crate::bootloader::block_flow::pre_tx_loop_op::PreTxLoopOp;

impl<S: EthereumLikeTypes> PreTxLoopOp<S> for ZKHeaderStructurePreTxOp
where
    S::IO: IOSubsystemExt,
{
    type PreTxLoopResult = ZKBasicTransactionDataKeeper;

    fn pre_op(
        _system: &mut System<S>,
        _result_keeper: &mut impl IOResultKeeper<EthereumIOTypesConfig>,
    ) -> Self::PreTxLoopResult {
        // Just create data keeper
        ZKBasicTransactionDataKeeper::new()
    }
}
