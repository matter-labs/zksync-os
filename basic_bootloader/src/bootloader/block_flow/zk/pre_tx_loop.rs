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
        system: &mut System<S>,
        _result_keeper: &mut impl IOResultKeeper<EthereumIOTypesConfig>,
    ) -> Self::PreTxLoopResult {
        // Create data keeper and seed block intrinsic constants
        let mut block_data = ZKBasicBlockDataKeeper::new();
        block_data.block_computational_native_used = BLOCK_INTRINSIC_NATIVE;
        block_data.block_pubdata_used = BLOCK_INTRINSIC_PUBDATA_BYTES;

        // EIP-2935: store parent block hash in history storage contract
        #[cfg(feature = "eip-2935")]
        {
            use crate::bootloader::block_flow::eip_2935_historical_block_hash::eip2935_system_part;
            eip2935_system_part(system).expect("must perform EIP-2935");
        }
        #[cfg(not(feature = "eip-2935"))]
        let _ = system;

        block_data
    }
}
