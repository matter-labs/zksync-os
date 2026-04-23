use zk_ee::system::IOResultKeeper;

use super::*;
use crate::bootloader::{
    block_flow::pre_tx_loop_op::PreTxLoopOp,
    constants::{BLOCK_INTRINSIC_NATIVE, BLOCK_INTRINSIC_PUBDATA_BYTES},
};

impl<S: EthereumLikeTypes, EA: TxHashesAccumulator> PreTxLoopOp<S> for ZKHeaderStructurePreTxOp<EA>
where
    S::IO: IOSubsystemExt,
{
    type PreTxLoopResult = ZKBasicBlockDataKeeper<EA>;

    fn pre_op(
        _system: &mut System<S>,
        _result_keeper: &mut impl IOResultKeeper<EthereumIOTypesConfig>,
    ) -> Self::PreTxLoopResult {
        // Just create data keeper and set block intrinsic constants
        let mut block_data = ZKBasicBlockDataKeeper::new();
        block_data.block_computational_native_used = BLOCK_INTRINSIC_NATIVE;
        block_data.block_pubdata_used = BLOCK_INTRINSIC_PUBDATA_BYTES;
        block_data
    }
}
