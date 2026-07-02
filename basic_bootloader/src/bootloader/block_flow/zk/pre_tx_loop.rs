use zk_ee::system::IOResultKeeper;

use super::*;
use crate::bootloader::{
    block_flow::pre_tx_loop_op::PreTxLoopOp,
    constants::{
        BLOBS_ZKSYNC_OS_LENGTH_PREFIX_PUBDATA_BYTES, BLOCK_INTRINSIC_NATIVE,
        BLOCK_INTRINSIC_PUBDATA_BYTES,
    },
};
use zk_ee::common_structs::DACommitmentScheme;

fn block_da_commitment_intrinsic_pubdata(da_commitment_scheme: Option<DACommitmentScheme>) -> u64 {
    match da_commitment_scheme {
        Some(DACommitmentScheme::BlobsZKsyncOS) => BLOBS_ZKSYNC_OS_LENGTH_PREFIX_PUBDATA_BYTES,
        _ => 0,
    }
}

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
        block_data.block_pubdata_used = BLOCK_INTRINSIC_PUBDATA_BYTES
            + block_da_commitment_intrinsic_pubdata(system.io.da_commitment_scheme());

        // EIP-2935: store parent block hash in history storage contract
        {
            use crate::bootloader::block_flow::eip_2935_historical_block_hash::eip2935_system_part;
            eip2935_system_part(system)?;
        }

        Ok(block_data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blobs_zksync_os_prefix_is_only_charged_for_blob_scheme() {
        assert_eq!(
            block_da_commitment_intrinsic_pubdata(None),
            0,
            "missing DA scheme should not add blob framing"
        );
        assert_eq!(
            block_da_commitment_intrinsic_pubdata(Some(
                DACommitmentScheme::BlobsAndPubdataKeccak256
            )),
            0,
            "keccak DA should not add the blob-only length prefix"
        );
        assert_eq!(
            block_da_commitment_intrinsic_pubdata(Some(DACommitmentScheme::BlobsZKsyncOS)),
            BLOBS_ZKSYNC_OS_LENGTH_PREFIX_PUBDATA_BYTES,
            "blob DA must reserve the extra length field element"
        );
    }
}
