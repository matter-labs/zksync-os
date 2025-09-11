use super::*;

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
                FlatTreeWithAccountsUnderHashesStorageModel<A, R, P, SC, N, false>,
                false,
            >,
        >,
    > PostTxLoopOp<S> for ZKHeaderStructurePostTxOp<false>
where
    S::IO: IOSubsystemExt + IOTeardown<S::IOTypes>,
{
    type BlockData = ZKBasicTransactionDataKeeper;
    type PostTxLoopOpResult = ();
    type BlockHeader = crate::bootloader::block_header::BlockHeader;

    fn post_op(
        system: System<S>,
        block_data: Self::BlockData,
        result_keeper: &mut impl ResultKeeperExt<EthereumIOTypesConfig, BlockHeader = Self::BlockHeader>,
    ) -> Self::PostTxLoopOpResult {
        // form block header
        let tx_rolling_hash = block_data.transaction_hashes_accumulator.finish();
        let block_gas_used = block_data.block_gas_used;

        let block_number = system.get_block_number();
        let previous_block_hash = system.get_blockhash(block_number);
        let beneficiary = system.get_coinbase();
        // TODO: Gas limit should be constant
        let gas_limit = system.get_gas_limit();
        // TODO: gas used shouldn't be zero
        let timestamp = system.get_timestamp();
        let consensus_random = system.get_mix_hash();
        let base_fee_per_gas = system.get_eip1559_basefee();
        // TODO: add gas_per_pubdata and native price

        // TODO: check if it's indeed u64
        let base_fee_per_gas = base_fee_per_gas.try_into().unwrap();
        // let base_fee_per_gas = base_fee_per_gas
        //     .try_into()
        //     .map_err(|_| internal_error!("base_fee_per_gas exceeds max u64"))?;
        let block_header = BlockHeader::new(
            previous_block_hash,
            beneficiary,
            tx_rolling_hash,
            block_number,
            gas_limit,
            block_gas_used,
            timestamp,
            consensus_random,
            base_fee_per_gas,
        );
        let current_block_hash = Bytes32::from(block_header.hash());
        result_keeper.block_sealed(block_header);

        // then perform IO related part

        let mut logger = system.get_logger();
        let _ = logger.write_fmt(format_args!("Basic header information was created\n"));

        let System { mut io, .. } = system;

        // Pubdata the block hash - we do not materialize root in it
        result_keeper.pubdata(current_block_hash.as_u8_ref());

        // Storage

        // 0. Flush accounts into storage, report preimages if needed
        io.flush_caches(result_keeper);
        io.report_new_preimages(result_keeper);

        // 1. Return uncompressed NET state diffs for sequencer
        // It benefit from filter being applied early, so for now it's kept using internal structure
        result_keeper.storage_diffs(io.storage.storage_cache.net_diffs_iter().map(|(k, v)| {
            let WarmStorageKey { address, key } = k;
            let value = v.current_value;
            (address, key, value)
        }));

        // 2. Commit to/return compressed pubdata
        io.storage
            .apply_storage_diffs_pubdata(result_keeper, &mut NopHasher, &mut io.oracle);

        // Logs pubdata
        // use concrete type as it's non-trivial
        io.logs_storage
            .apply_logs_to_pubdata(result_keeper, &mut NopHasher);

        // Logs themselves
        result_keeper.logs(io.logs_storage.messages_ref_iter());

        // Events
        result_keeper.events(io.events_storage.events_ref_iter());

        // // 3. Verify/apply reads and writes
        cycle_marker::wrap!("verify_and_apply_batch", {
            io.update_commitment(None, &mut logger, result_keeper);
        });
    }
}
