use zk_ee::system::metadata::basic_metadata::{BasicMetadata, ZkSpecificMetadata};
use zk_ee::system::metadata::zk_metadata::TxLevelMetadata;
use zk_ee::system::IOResultKeeper;

use super::*;
use crate::bootloader::{
    block_flow::pre_tx_loop_op::PreTxLoopOp,
    constants::{BLOCK_INTRINSIC_NATIVE, BLOCK_INTRINSIC_PUBDATA_BYTES},
};

impl<S: EthereumLikeTypes, EA: TxHashesAccumulator> PreTxLoopOp<S> for ZKHeaderStructurePreTxOp<EA>
where
    S::IO: IOSubsystemExt,
    S::Metadata: ZkSpecificMetadata
        + BasicMetadata<S::IOTypes, TransactionMetadata = TxLevelMetadata<S::IOTypes>>,
{
    type PreTxLoopResult = ZKBasicBlockDataKeeper<EA, S::Allocator>;

    fn pre_op(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
        memories: RunnerMemoryBuffers<'_>,
        _result_keeper: &mut impl IOResultKeeper<EthereumIOTypesConfig>,
    ) -> Result<Self::PreTxLoopResult, BootloaderSubsystemError> {
        // Create data keeper and seed block intrinsic constants
        let mut block_data = ZKBasicBlockDataKeeper::new_in(system.get_allocator());
        block_data.block_computational_native_used = BLOCK_INTRINSIC_NATIVE;
        block_data.block_pubdata_used = BLOCK_INTRINSIC_PUBDATA_BYTES;

        // EIP-2935: store parent block hash in history storage contract
        {
            use crate::bootloader::block_flow::eip_2935_historical_block_hash::eip2935_system_part;
            eip2935_system_part(system)?;
        }

        crate::bootloader::transaction_flow::zk::process_l1_transaction::prewarm_l1_postprocessing::<
            S,
        >(system, system_functions, memories)?;

        Ok(block_data)
    }
}
