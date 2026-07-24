use zk_ee::system::IOResultKeeper;

use super::*;
use crate::bootloader::{
    block_flow::pre_tx_loop_op::PreTxLoopOp,
    constants::{
        BLOCK_INTRINSIC_NATIVE, BLOCK_INTRINSIC_PUBDATA_BYTES,
        LOGS_ONLY_BLOCK_INTRINSIC_PUBDATA_BYTES,
    },
};
use zk_ee::common_structs::da_commitment_scheme::PubdataContent;

impl<S: EthereumLikeTypes, EA: TxHashesAccumulator> PreTxLoopOp<S> for ZKHeaderStructurePreTxOp<EA>
where
    S::IO: IOSubsystemExt,
{
    type PreTxLoopResult = ZKBasicBlockDataKeeper<EA, S::Allocator>;

    fn pre_op(
        system: &mut System<S>,
        _result_keeper: &mut impl IOResultKeeper<EthereumIOTypesConfig>,
    ) -> Result<Self::PreTxLoopResult, BootloaderSubsystemError> {
        // Create data keeper and seed block intrinsic constants
        let mut block_data = ZKBasicBlockDataKeeper::new_in(system.get_allocator());
        block_data.block_computational_native_used = BLOCK_INTRINSIC_NATIVE;
        // In Validium only the mandatory logs prefix header is committed; the rest of the block
        // intrinsic pubdata (block context, counters, EIP-2935 diff) is not (see `write_pubdata`).
        block_data.block_pubdata_used = match system.get_chain_config().pubdata_content() {
            PubdataContent::FullPubdata => BLOCK_INTRINSIC_PUBDATA_BYTES,
            PubdataContent::LogsOnly => LOGS_ONLY_BLOCK_INTRINSIC_PUBDATA_BYTES,
        };

        // Snapshot the interop commitment tree (IMT) root before any transaction runs. For the first
        // block of a batch this is the batch-begin root; the batch data keeper commits it (alongside
        // the batch-end root read in `post_op`) into the chain batch root.
        block_data.commitment_tree_root_begin = read_interop_commitment_tree_root(&mut system.io);

        // EIP-2935: store parent block hash in history storage contract
        {
            use crate::bootloader::block_flow::eip_2935_historical_block_hash::eip2935_system_part;
            eip2935_system_part(system)?;
        }

        Ok(block_data)
    }
}
