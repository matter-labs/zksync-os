use super::*;
use crate::bootloader::block_flow::zk::post_tx_op::da_commitment_generator::NopCommitmentGenerator;
use basic_system::system_implementation::caches::storage_access_policy::StorageAccessPolicy;
use basic_system::system_implementation::flat_storage_model::FlatTreeWithAccountsUnderHashesStorageModel;
use basic_system::system_implementation::system::FullIO;
use core::alloc::Allocator;
use zk_ee::common_structs::WarmStorageKey;
use zk_ee::logger_log;
use zk_ee::memory::stack_trait::StackFactory;
use zk_ee::oracle::IOOracle;
use zk_ee::system::metadata::basic_metadata::BasicBlockMetadata;
use zk_ee::system::{IOTeardown, Resources};

impl<
        A: Allocator + Clone + Default,
        R: Resources,
        P: StorageAccessPolicy<R, Bytes32> + Default,
        SF: StackFactory<N>,
        const N: usize,
        O: IOOracle,
        S: EthereumLikeTypes<
            IO = FullIO<
                A,
                R,
                P,
                SF,
                N,
                O,
                FlatTreeWithAccountsUnderHashesStorageModel<A, R, P, SF, N, false>,
                false,
            >,
        >,
    > PostTxLoopOp<S> for ZKHeaderStructurePostTxOpSequencing
where
    S::IO: IOSubsystemExt + IOTeardown<S::IOTypes>,
{
    type PostTxLoopOpResult = ();
    type BlockDataKeeper = ZKBasicBlockDataKeeper<NopTxHashesAccumulator>;
    type BatchDataKeeper = ();
    type BlockHeader = crate::bootloader::block_header::BlockHeader;

    fn post_op(
        system: System<S>,
        block_data: Self::BlockDataKeeper,
        _batch_data: &mut Self::BatchDataKeeper,
        result_keeper: &mut impl ResultKeeperExt<EthereumIOTypesConfig, BlockHeader = Self::BlockHeader>,
    ) -> Result<Self::PostTxLoopOpResult, BootloaderSubsystemError> {
        let block_header = form_block_header(
            &system,
            block_data.transaction_hashes_accumulator.finish().0,
            block_data.block_gas_used,
        )?;
        let block_hash = Bytes32::from(block_header.hash());
        result_keeper.block_sealed(block_header);

        let mut logger = system.get_logger();
        logger_log!(logger, "Basic header information was created\n");

        let System {
            mut io, metadata, ..
        } = system;

        io.flush_caches(result_keeper);

        result_keeper.storage_diffs(io.storage.storage_cache.net_diffs_iter().map(|(k, v)| {
            let WarmStorageKey { address, key } = k;
            let value = v.current_value;
            (address, key, value)
        }));
        io.report_new_preimages(result_keeper);
        result_keeper.logs(io.logs_storage.messages_ref_iter());
        result_keeper.events(io.events_storage.events_ref_iter());

        write_pubdata(
            &mut NopCommitmentGenerator,
            result_keeper,
            block_hash,
            metadata.block_timestamp(),
            &mut io,
        );

        cycle_marker::wrap!("verify_and_apply_batch", {
            io.update_commitment(None, &mut logger, result_keeper);
        });
        Ok(())
    }
}
