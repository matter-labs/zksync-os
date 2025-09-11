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
use zk_ee::system::errors::internal::InternalError;

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
                EthereumStorageModel<A, R, P, SC, N, true>,
                true,
            >,
            Metadata = EthereumBlockMetadata,
        >,
    > PostTxLoopOp<S> for EthereumPostOp<true>
where
    S::IO: IOSubsystemExt + IOTeardown<S::IOTypes, IOStateCommitment = Bytes32>,
{
    type BlockData = EthereumBasicTransactionDataKeeper<S::Allocator, S::Allocator>;
    type PostTxLoopOpResult = (O, Bytes32);
    type BlockHeader = PectraForkHeader;

    fn post_op(
        mut system: System<S>,
        block_data: Self::BlockData,
        result_keeper: &mut impl ResultKeeperExt<EthereumIOTypesConfig, BlockHeader = Self::BlockHeader>,
    ) -> Self::PostTxLoopOpResult {
        let _handle = system
            .start_global_frame()
            .expect("must open frame to handle storage access");

        Self::post_op_io_touching_impl(&mut system, block_data)
            .expect("must process IO related part");

        system
            .finish_global_frame(None)
            .expect("must finish frame to handle storage access");

        // IO related part ends here, and we can flush all our changes

        let mut logger = system.get_logger();
        let allocator = system.get_allocator();

        let System {
            mut io, metadata, ..
        } = system;

        // peek into history, but not further than we actually need
        let num_to_verify_from_history_cache = unsafe {
            metadata
                .block_level
                .history_cache
                .as_ref_unchecked()
                .num_elements_to_verify()
        };
        let num_to_verify = core::cmp::max(num_to_verify_from_history_cache, 1);

        let initial_state_commitment = ChainChecker::verify_chain(
            &metadata.block_level.header,
            metadata.block_level.header.number,
            num_to_verify,
            io.oracle(),
            unsafe { metadata.block_level.history_cache.as_ref_unchecked() },
            allocator.clone(),
        )
        .expect("chain must be consistent");

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

        let _ = logger.write_fmt(format_args!(
            "Initial state commitment is {:?}\n",
            &initial_state_commitment
        ));

        // // 3. Verify/apply reads and writes
        let mut updated_state_commitment = initial_state_commitment;
        cycle_marker::wrap!("verify_and_apply_batch", {
            io.update_commitment(
                Some(&mut updated_state_commitment),
                &mut logger,
                result_keeper,
            );
        });

        let _ = logger.write_fmt(format_args!(
            "Updated state commitment is {:?}\n",
            &updated_state_commitment
        ));

        assert_eq!(
            metadata.block_level.header.state_root, updated_state_commitment,
            "state root diverged",
        );

        let _ = logger.write_fmt(format_args!(
            "Finished processing block hash {:?}\n",
            &metadata.block_level.computed_header_hash,
        ));

        (io.oracle, metadata.block_level.computed_header_hash)
    }
}

impl EthereumPostOp<true> {
    fn post_op_io_touching_impl<
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
                EthereumStorageModel<A, R, P, SC, N, true>,
                true,
            >,
            Metadata = EthereumBlockMetadata,
        >,
    >(
        system: &mut System<S>,
        block_data: <Self as PostTxLoopOp<S>>::BlockData,
    ) -> Result<(), InternalError>
    where
        S::IO: IOSubsystemExt + IOTeardown<S::IOTypes, IOStateCommitment = Bytes32>,
    {
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
                    process_withdrawals_list::<S, S::VecLikeCtor>(system, withdrawals_list)
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
        let mut intermediate_hasher = crypto::sha256::Sha256::new();
        if eip6110_events_parser(&*system, &mut intermediate_hasher)
            .expect("must filter EIP-6110 deposit requests")
        {
            let requests_hash = intermediate_hasher.finalize_reset();
            let _ = system.get_logger().write_fmt(format_args!(
                "EIP-6110 ops hash = {:?}\n",
                Bytes32::from_array(requests_hash.into()),
            ));
            requests_hasher.update(requests_hash);
        }
        if eip7002_system_part(system, &mut intermediate_hasher)
            .expect("withdrawal requests must be processed")
        {
            let requests_hash = intermediate_hasher.finalize_reset();
            let _ = system.get_logger().write_fmt(format_args!(
                "EIP-7002 ops hash = {:?}\n",
                Bytes32::from_array(requests_hash.into()),
            ));
            requests_hasher.update(requests_hash);
        }
        if eip7251_system_part(system, &mut intermediate_hasher)
            .expect("consolidation requests must be processed")
        {
            let requests_hash = intermediate_hasher.finalize_reset();
            let _ = system.get_logger().write_fmt(format_args!(
                "EIP-7251 ops hash = {:?}\n",
                Bytes32::from_array(requests_hash.into()),
            ));
            requests_hasher.update(requests_hash);
        }
        let requests_hash = Bytes32::from_array(requests_hasher.finalize().into());
        let _ = system
            .get_logger()
            .write_fmt(format_args!("Requests hash = {:?}\n", &requests_hash,));

        let block_data_results = block_data.compute_header_values::<S, S::VecLikeCtor>(&system);

        // Now we will check that header is consistent with out claims about it
        // - withdrawals root
        assert_eq!(
            withdrawals_root, system.metadata.block_level.header.withdrawals_root,
            "withdrawals root diverged",
        );
        // - transactions root
        assert_eq!(
            block_data_results.transactions_root,
            system.metadata.block_level.header.transactions_root,
            "transactions root diverged",
        );
        // - receipts root
        assert_eq!(
            block_data_results.receipts_root, system.metadata.block_level.header.receipts_root,
            "receipts root diverged",
        );
        // - bloom
        assert_eq!(
            block_data_results.block_bloom, system.metadata.block_level.header.logs_bloom,
            "block Bloom filter diverged",
        );
        // - requests
        assert_eq!(
            requests_hash, system.metadata.block_level.header.requests_hash,
            "requests hash diverged",
        );

        Ok(())
    }
}
