use core::alloc::Allocator;
use ruint::Uint;
use basic_system::system_implementation::caches::storage_access_policy::StorageAccessPolicy;
use basic_system::system_implementation::flat_storage_model::{FlatStorageCommitment, FlatTreeWithAccountsUnderHashesStorageModel, TREE_HEIGHT};
use basic_system::system_implementation::system::FullIO;
use crypto::blake2s::Blake2s256;
use zk_ee::common_structs::{ProofData, WarmStorageKey};
use zk_ee::memory::stack_trait::StackFactory;
use zk_ee::oracle::basic_queries::ZKProofDataQuery;
use super::*;
use zk_ee::oracle::IOOracle;
use zk_ee::oracle::simple_oracle_query::SimpleOracleQuery;
use zk_ee::system::{IOTeardown, Resources};
use zk_ee::system::metadata::basic_metadata::BasicBlockMetadata;
use zk_ee::system::metadata::zk_metadata::ZkMetadata;
use zk_ee::utils::write_bytes::WriteBytes;
use crate::bootloader::block_flow::zk::post_tx_op::da_commitment_generator::{Blake2sCommitmentGenerator, DACommitmentGenerator, NopCommitmentGenerator};
use crate::bootloader::block_flow::zk::post_tx_op::public_input::{BlocksOutput, BlocksPublicInput, ChainStateCommitment};

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
        Metadata = ZkMetadata
    >,
> PostTxLoopOp<S> for ZKHeaderStructurePostTxOpProvingAggregation
where
    S::IO: IOSubsystemExt
    + IOTeardown<S::IOTypes, IOStateCommitment = FlatStorageCommitment<TREE_HEIGHT>>, // IOStateCommitment bound is trivial, most likely needed due to missing associated types equality feature in the current state of the compiler
{
    type PostTxLoopOpResult = (O, Bytes32);
    type BlockDataKeeper = ZKBasicBlockDataKeeper<AccumulatingBlake2sTransactionsHasher>;
    type BatchDataKeeper = ();
    type BlockHeader = BlockHeader;

    fn post_op(
        system: System<S>,
        block_data: Self::BlockDataKeeper,
        _batch_data: &mut Self::BatchDataKeeper,
        result_keeper: &mut impl ResultKeeperExt<EthereumIOTypesConfig, BlockHeader = Self::BlockHeader>,
    ) -> Result<Self::PostTxLoopOpResult, BootloaderSubsystemError> {
        let block_header = form_block_header(&system, block_data.transaction_hashes_accumulator.finish().0, block_data.block_gas_used)?;
        let block_hash = Bytes32::from(block_header.hash());
        result_keeper.block_sealed(block_header);

        let mut logger = system.get_logger();
        let _ = logger.write_fmt(format_args!("Basic header information was created\n"));

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

        let mut da_commitment_generator = Blake2sCommitmentGenerator::new();
        write_pubdata(&mut da_commitment_generator, result_keeper, block_hash, metadata.block_timestamp(), &mut io);
        let da_commitment = da_commitment_generator.finalize(io.oracle());

        let mut l2_to_l1_logs_hasher = crypto::blake2s::Blake2s256::new();
        io.logs_storage.apply_l2_to_l1_logs_hashes_to_hasher(&mut l2_to_l1_logs_hasher);
        let l2_to_l1_logs_hashes_hash = Bytes32::from(l2_to_l1_logs_hasher.finalize());

        let l1_to_l2_tx_hash = block_data.enforced_transaction_hashes_accumulator.finish();
        let upgrade_tx_hash = block_data.upgrade_tx_recorder.finish();
        let mut interop_roots_hasher = Blake2s256::new();
        for root in io.interop_root_storage.iter() {
            interop_roots_hasher.update(&root.chain_id.to_be_bytes::<{ U256::BYTES }>());
            interop_roots_hasher.update(&root.block_or_batch_number.to_be_bytes::<{ U256::BYTES }>());
            interop_roots_hasher.update(root.root.as_u8_ref());
        }
        let interop_roots_hash = interop_roots_hasher.finalize().into();
        let settlement_layer_chain_id = io.read_settlement_layer_chain_id();
        if let Some(new_settlement_layer_chain_id) =
            io.new_settlement_layer_chain_id_storage.value()
        {
            // If the SL chain id was updated, make sure the updated one matches
            // the one read from storage
            assert_eq!(new_settlement_layer_chain_id, &settlement_layer_chain_id)
        }

        let (mut state_commitment, last_block_timestamp) = {
            let proof_data: ProofData<FlatStorageCommitment<TREE_HEIGHT>> =
                ZKProofDataQuery::get(&mut io.oracle, &())
                    .expect("must get proof data from oracle");
            (proof_data.state_root_view, proof_data.last_block_timestamp)
        };

        let _ = logger.write_fmt(format_args!(
            "Initial state commitment is {:?}\n",
            &state_commitment
        ));
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

        let block_output = BlocksOutput {
            chain_id: U256::from(metadata.chain_id()),
            first_block_timestamp: metadata.block_timestamp(),
            last_block_timestamp: metadata.block_timestamp(),
            pubdata_hash: da_commitment,
            priority_ops_hashes_hash: l1_to_l2_tx_hash,
            l2_to_l1_logs_hashes_hash,
            upgrade_tx_hash,
            interop_roots_hash,
            settlement_layer_chain_id,
        };

        let public_input = BlocksPublicInput {
            state_before: chain_state_commitment_before.hash().into(),
            state_after: chain_state_commitment_after.hash().into(),
            blocks_output: block_output.hash().into(),
        };

        Ok((io.oracle, public_input.hash().into()))
    }
}