use super::*;
use crate::bootloader::block_flow::zk::post_tx_op::da_commitment_generator::da_commitment_generator_from_scheme;
use crate::bootloader::block_flow::zk::post_tx_op::public_input::{
    BatchOutput, BatchPublicInput, ChainStateCommitment,
};
use basic_system::system_implementation::caches::storage_access_policy::StorageAccessPolicy;
use basic_system::system_implementation::flat_storage_model::{
    FlatStorageCommitment, FlatTreeWithAccountsUnderHashesStorageModel, TREE_HEIGHT,
};
use basic_system::system_implementation::system::FullIO;
use core::alloc::Allocator;
use crypto::blake2s::Blake2s256;
use zk_ee::common_structs::{derive_flat_storage_key_with_hasher, ProofData, WarmStorageKey};
use zk_ee::logger_log;
use zk_ee::memory::stack_trait::StackFactory;
use zk_ee::oracle::basic_queries::{DisconnectOracleQuery, ZKProofDataQuery};
use zk_ee::oracle::simple_oracle_query::SimpleOracleQuery;
use zk_ee::oracle::IOOracle;
use zk_ee::system::metadata::basic_metadata::BasicBlockMetadata;
use zk_ee::system::metadata::zk_metadata::ZkMetadata;
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
                FlatTreeWithAccountsUnderHashesStorageModel<A, R, P, SF, N, true>,
                true,
            >,
            Metadata = ZkMetadata,
        >,
        const STATE_DIFFS_HASH: bool,
    > PostTxLoopOp<S> for ZKHeaderStructurePostTxOpProvingSingleblockBatch<STATE_DIFFS_HASH>
where
    S::IO: IOSubsystemExt
        + IOTeardown<S::IOTypes, IOStateCommitment = FlatStorageCommitment<TREE_HEIGHT>>, // IOStateCommitment bound is trivial, most likely needed due to missing associated types equality feature in the current state of the compiler
{
    type PostTxLoopOpResult = (O, Bytes32, public_input::BatchOutput);
    type BlockDataKeeper = ZKBasicBlockDataKeeper<TransactionsRollingKeccakHasher>;
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

        let mut da_commitment_generator =
            da_commitment_generator_from_scheme(io.da_commitment_scheme.unwrap(), A::default())
                .unwrap();
        write_pubdata(
            da_commitment_generator.as_mut(),
            result_keeper,
            block_hash,
            metadata.block_timestamp(),
            &mut io,
        );

        let (multichain_root, settlement_layer_chain_id) = read_batch_context_inputs(&mut io);

        let mut full_root_hasher = crypto::sha3::Keccak256::new();
        full_root_hasher.update(io.logs_storage.tree_root().as_u8_ref());
        full_root_hasher.update(multichain_root.as_u8_ref());
        let full_l2_to_l1_logs_root = full_root_hasher.finalize().into();

        let (priority_operations_hash, number_of_layer_1_txs) =
            block_data.enforced_transaction_hashes_accumulator.finish();
        // Number of L2 transactions can be calculated as:
        // Total txs - l1 txs - upgrade txs
        let mut number_of_layer_2_txs =
            block_data.current_transaction_number - number_of_layer_1_txs;
        if !block_data.upgrade_tx_recorder.is_empty() {
            number_of_layer_2_txs -= 1;
        }
        let upgrade_tx_hash = block_data.upgrade_tx_recorder.finish();
        let interop_roots_rolling_hash = calculate_interop_roots_rolling_hash(
            Bytes32::zero(),
            io.interop_root_storage.iter(),
            &mut crypto::sha3::Keccak256::new(),
        );

        let (mut state_commitment, last_block_timestamp) = {
            let proof_data: ProofData<FlatStorageCommitment<TREE_HEIGHT>> =
                ZKProofDataQuery::get(&mut io.oracle, &())
                    .expect("must get proof data from oracle");
            (proof_data.state_root_view, proof_data.last_block_timestamp)
        };

        logger_log!(
            logger,
            "Initial state commitment is {:?}\n",
            &state_commitment
        );
        // validate that timestamp didn't decrease
        assert!(metadata.block_timestamp() >= last_block_timestamp);

        // chain state commitment before
        let mut blocks_hasher = Blake2s256::new();
        for block_hash in metadata.block_level.block_hashes.0.iter() {
            blocks_hasher.update(&block_hash.to_be_bytes::<32>());
        }
        let chain_state_commitment_before = ChainStateCommitment {
            state_root: state_commitment.root,
            next_free_slot: state_commitment.next_free_slot,
            block_number: metadata.block_number() - 1,
            last_256_block_hashes_blake: blocks_hasher.finalize().into(),
            last_block_timestamp,
        };
        logger_log!(
            logger,
            "PI calculation: state commitment before {:?}\n",
            chain_state_commitment_before
        );

        // update state commitment
        cycle_marker::wrap!("verify_and_apply_batch", {
            IOTeardown::<_>::update_commitment(
                &mut io,
                Some(&mut state_commitment),
                &mut logger,
                result_keeper,
            );
        });

        // chain state commitment after
        let mut blocks_hasher = Blake2s256::new();
        for block_hash in metadata.block_level.block_hashes.0.iter().skip(1) {
            blocks_hasher.update(&block_hash.to_be_bytes::<32>());
        }
        blocks_hasher.update(block_hash.as_u8_ref());
        let chain_state_commitment_after = ChainStateCommitment {
            state_root: state_commitment.root,
            next_free_slot: state_commitment.next_free_slot,
            block_number: metadata.block_number(),
            last_256_block_hashes_blake: blocks_hasher.finalize().into(),
            last_block_timestamp: metadata.block_timestamp(),
        };
        logger_log!(
            logger,
            "PI calculation: state commitment after {:?}\n",
            chain_state_commitment_after
        );
        // We need to do this after the tree finalization, as we need to preserve
        // the order of oracle queries.
        let da_commitment = da_commitment_generator.finalize(io.oracle());

        let batch_output = BatchOutput {
            chain_id: U256::from(metadata.chain_id()),
            first_block_timestamp: metadata.block_timestamp(),
            last_block_timestamp: metadata.block_timestamp(),
            da_commitment_scheme: io.da_commitment_scheme.unwrap(),
            pubdata_commitment: da_commitment,
            number_of_layer_1_txs: U256::try_from(number_of_layer_1_txs).unwrap(),
            number_of_layer_2_txs: U256::from(number_of_layer_2_txs),
            priority_operations_hash,
            l2_logs_tree_root: full_l2_to_l1_logs_root,
            upgrade_tx_hash,
            interop_roots_rolling_hash,
            settlement_layer_chain_id,
        };
        logger_log!(logger, "PI calculation: batch output {:?}\n", batch_output,);

        let public_input = BatchPublicInput {
            state_before: chain_state_commitment_before.hash().into(),
            state_after: chain_state_commitment_after.hash().into(),
            batch_output: batch_output.hash().into(),
        };
        logger_log!(
            logger,
            "PI calculation: final batch public input {:?}\n",
            public_input,
        );
        let public_input_hash = public_input.hash().into();
        logger_log!(
            logger,
            "PI calculation: final batch public input hash {:?}\n",
            public_input_hash,
        );

        if STATE_DIFFS_HASH {
            let mut hasher = crypto::blake2s::Blake2s256::new();
            let mut state_diffs_hasher = crypto::blake2s::Blake2s256::new();

            // Iterate through all modified storage entries and hash them deterministically
            io.storage
                .storage_cache
                .net_diffs_iter()
                .for_each(|(key, value)| {
                    let derived_key =
                        derive_flat_storage_key_with_hasher(&key.address, &key.key, &mut hasher);
                    logger_log!(
                        logger,
                        "State diffs hash - key: {:?}, new value: {:?}\n",
                        derived_key,
                        value.current_value
                    );
                    // Hash the derived key and new value together to create deterministic state diff hash
                    state_diffs_hasher.update(derived_key.as_u8_ref());
                    state_diffs_hasher.update(value.current_value.as_u8_ref());
                });
            let state_diffs_hash = state_diffs_hasher.finalize().into();

            <DisconnectOracleQuery as SimpleOracleQuery>::get(&mut io.oracle, &())?;
            Ok((io.oracle, state_diffs_hash, batch_output))
        } else {
            <DisconnectOracleQuery as SimpleOracleQuery>::get(&mut io.oracle, &())?;
            Ok((io.oracle, public_input_hash, batch_output))
        }
    }
}
