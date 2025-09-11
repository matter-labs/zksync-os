use super::*;
use crate::bootloader::block_flow::ethereum_block_flow::block_header::PectraForkHeader;
use crate::bootloader::block_flow::ethereum_block_flow::eip_6110_deposit_events_parser::eip6110_events_parser;
use crate::bootloader::block_flow::ethereum_block_flow::eip_7002_withdrawal_contract::eip7002_system_part;
use crate::bootloader::block_flow::ethereum_block_flow::eip_7251_consolidation_contract::eip7251_system_part;
use crate::bootloader::block_flow::ethereum_block_flow::withdrawals::process_withdrawals_list;
use crate::bootloader::block_flow::ethereum_block_flow::{
    oracle_queries::{
        ETHEREUM_WITHDRAWALS_BUFFER_DATA_QUERY_ID, ETHEREUM_WITHDRAWALS_BUFFER_LEN_QUERY_ID,
    },
    withdrawals::WithdrawalsList,
};
use basic_system::system_implementation::ethereum_storage_model::EthereumStorageModel;
use basic_system::system_implementation::ethereum_storage_model::EMPTY_ROOT_HASH;

impl<
        A: Allocator + Clone + Default,
        R: Resources,
        P: StorageAccessPolicy<R, Bytes32> + Default,
        SC: StackCtor<N>,
        O: IOOracle,
        const N: usize,
        S: EthereumLikeTypes<
            IO = BasicStorageModel<
                A,
                R,
                P,
                SC,
                N,
                O,
                EthereumStorageModel<A, R, P, SC, N, false>,
                false,
            >,
            Metadata = EthereumBlockMetadata,
        >,
    > PostTxLoopOp<S> for EthereumPostOp<false>
where
    S::IO: IOSubsystemExt + IOTeardown<S::IOTypes, IOStateCommitment = Bytes32>,
{
    type BlockData = EthereumBasicTransactionDataKeeper<S::Allocator, S::Allocator>;
    type PostTxLoopOpResult = ();
    type BlockHeader = PectraForkHeader;

    fn post_op(
        mut system: System<S>,
        _block_data: Self::BlockData,
        result_keeper: &mut impl ResultKeeperExt<EthereumIOTypesConfig, BlockHeader = Self::BlockHeader>,
    ) -> Self::PostTxLoopOpResult {
        // apply withdrawals
        let withdrawals_root = {
            // apply withdrawals - we will be lazy here and instead will allocate some bytes and parse them. We anyway will need
            // encoding of withdrawal request for root calculation
            let withdrawals_encoding = system
                .get_bytes_from_query(
                    ETHEREUM_WITHDRAWALS_BUFFER_LEN_QUERY_ID,
                    ETHEREUM_WITHDRAWALS_BUFFER_DATA_QUERY_ID,
                )
                .expect("must get withdrawals bytes");
            let withdrawals_root = if let Some(withdrawals) = withdrawals_encoding {
                let Ok(withdrawals_list) =
                    WithdrawalsList::try_parse_slice_in_full(withdrawals.as_slice())
                else {
                    panic!("Withdrawals list is invalid");
                };
                let Some(count) = withdrawals_list.count else {
                    panic!("Withdrawals list was parsed without validation");
                };
                if count > 0 {
                    process_withdrawals_list::<S, S::VecLikeCtor>(&mut system, withdrawals_list)
                        .expect("must process withdrawals list")
                } else {
                    EMPTY_ROOT_HASH
                }
            } else {
                EMPTY_ROOT_HASH
            };

            withdrawals_root
        };

        let _ = system
            .get_logger()
            .write_fmt(format_args!("Withdrawals root = {:?}\n", &withdrawals_root,));

        use crypto::sha256::Digest;
        let mut requests_hasher = crypto::sha256::Sha256::new();

        // Environment may have no such contracts predeployed for tests or sequencing purposes
        let _ = eip6110_events_parser(&system, &mut requests_hasher);
        let _ = eip7002_system_part(&mut system, &mut requests_hasher);
        let _ = eip7251_system_part(&mut system, &mut requests_hasher);

        let requests_hash = Bytes32::from_array(requests_hasher.finalize().into());
        let _ = system
            .get_logger()
            .write_fmt(format_args!("Requests hash = {:?}\n", &requests_hash,));

        // Here we have to cascade everything

        let mut logger = system.get_logger();

        let System {
            mut io, metadata, ..
        } = system;

        result_keeper.record_sealed_block(metadata.block_level.header);

        // Storage

        // 0. Flush accounts into storage, report preimages if needed
        io.flush_caches(result_keeper);
        io.report_new_preimages(result_keeper);

        // These two benefit from filter being applied early, so for now it's kept using internal structure
        result_keeper.basic_account_diffs(io.storage.account_cache.net_diffs_iter());
        result_keeper.storage_diffs(io.storage.storage_cache.net_diffs_iter().map(|(k, v)| {
            let WarmStorageKey { address, key } = k;
            let value = v.current_value;
            (address, key, value)
        }));

        // Events
        result_keeper.events(io.events_iterator());

        // // 3. Verify/apply reads and writes
        cycle_marker::wrap!("verify_and_apply_batch", {
            io.update_commitment(None, &mut logger, result_keeper);
        });
    }
}
