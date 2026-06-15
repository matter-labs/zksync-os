mod batch;
pub mod errors;
pub mod fri_admission;
mod fri_proof_decode;
mod fri_proof_sidecar;
pub mod output;
mod preimage_source;
mod tree;
mod tx_result_callback;
mod tx_source;

pub mod convert;
pub mod convert_alloy;
mod interface_impl;
pub mod query_processors;
pub mod result_keeper;
pub mod test_impl;
mod tracing_impl;
mod validator_impl;

use crate::run::query_processors::FriProofResponder;
use crate::run::query_processors::GenericPreimageResponder;
use crate::run::query_processors::ReadStorageResponder;
use crate::run::query_processors::ReadTreeResponder;
use crate::run::query_processors::TxDataResponder;
use crate::run::query_processors::UARTPrintResponder;
use crate::run::query_processors::ZKProofDataResponder;
use crate::run::query_processors::{
    BlockMetadataResponder, ChainConfigResponder, DACommitmentSchemeResponder,
};
use crate::run::result_keeper::ForwardRunningResultKeeper;
use crate::system::bootloader::run_forward;
use crate::system::bootloader::run_prover_input_no_panic;
use crate::system::system_types::BatchProverInputBootloader;
use crate::system::system_types::CallSimulationBootloader;
use crate::system::system_types::CallSimulationSystem;
use crate::system::system_types::ForwardRunningSystem;
use basic_bootloader::bootloader::block_flow::public_input::{BatchOutput, BatchPublicInput};
use basic_bootloader::bootloader::config::{
    BasicBootloaderCallSimulationConfig, BasicBootloaderForwardSimulationConfig,
    BasicBootloaderProvingExecutionConfig,
};
use basic_bootloader::bootloader::errors::BootloaderSubsystemError;
use errors::ForwardSubsystemError;
use oracle_provider::ReadWitnessSource;
use oracle_provider::ZkEENonDeterminismSource;
use result_keeper::ProverInputResultKeeper;
use std::sync::Arc;
use zk_ee::common_structs::ProofData;
use zk_ee::oracle::basic_queries::DisconnectOracleQuery;
use zk_ee::oracle::simple_oracle_query::SimpleOracleQuery;
use zk_ee::system::logger::NullLogger;
use zk_ee::system::tracer::NopTracer;
use zk_ee::system::tracer::Tracer;

pub use self::batch::{BatchBlockInput, BatchState};
pub use interface_impl::RunBlockForward;
pub use tree::LeafProof;
pub use tree::ReadStorage;
pub use tree::ReadStorageTree;
use zk_ee::system::validator::NopTxValidator;
use zk_ee::system::validator::TxValidator;
pub use zk_ee::types_config::EthereumIOTypesConfig;

pub use crate::run::fri_admission::{validate_fri_statement, FriAdmissionError};
pub use crate::run::query_processors::FriVerifierArtifacts;
pub use basic_bootloader::bootloader::fri_host_verifier::FriHostVerifyError;
pub use fri_proof_sidecar::{FriProofSidecarSource, NoFriProofSidecar};
pub use preimage_source::PreimageSource;
use zk_ee::wrap_error;
use zksync_os_interface::traits::EncodedTx;

pub use tx_result_callback::TxResultCallback;
pub use tx_source::NextTxResponse;
pub use tx_source::TxSource;

use self::output::BlockOutput;
use crate::run::output::TxResult;
use crate::run::test_impl::NoopTxCallback;
pub use basic_bootloader::bootloader::errors::InvalidTransaction;
use basic_system::system_implementation::flat_storage_model::*;
use zk_ee::common_structs::da_commitment_scheme::DACommitmentScheme;
use zk_ee::oracle::usize_serialization::UsizeSerializable;
use zk_ee::system::metadata::chain_config::ChainConfig;
pub use zk_ee::system::metadata::zk_metadata::BlockMetadataFromOracle as BlockContext;
use zksync_os_interface::traits::TxListSource;

pub type StorageCommitment = FlatStorageCommitment<{ TREE_HEIGHT }>;

/// Result of the batch prover-input run.
pub struct BatchRunOutput {
    /// Canonical batch prover input.
    pub prover_input: Vec<u32>,
    /// Canonical batch pubdata accumulated across all blocks.
    pub pubdata: Vec<u8>,
    /// Batch public input derived by the multiblock post-op.
    pub batch_public_input: BatchPublicInput,
    /// Batch output derived by the multiblock post-op.
    pub batch_output: BatchOutput,
    /// Per-block forward outputs observed while executing the batch.
    pub block_outputs: Vec<BlockOutput>,
}

pub fn run_block<
    T: ReadStorageTree,
    PS: PreimageSource,
    TS: TxSource,
    FS: FriProofSidecarSource,
    TR: TxResultCallback,
>(
    block_context: BlockContext,
    tree: T,
    preimage_source: PS,
    tx_source: TS,
    fri_proof_sidecar: FS,
    fri_verifier_artifacts: Option<Arc<FriVerifierArtifacts>>,
    tx_result_callback: TR,
    tracer: &mut impl Tracer<ForwardRunningSystem>,
    validator: &mut impl TxValidator<ForwardRunningSystem>,
) -> Result<BlockOutput, ForwardSubsystemError> {
    run_block_with_chain_config(
        ChainConfig::default(),
        block_context,
        tree,
        preimage_source,
        tx_source,
        fri_proof_sidecar,
        fri_verifier_artifacts,
        tx_result_callback,
        tracer,
        validator,
    )
}

pub fn run_block_with_chain_config<
    T: ReadStorageTree,
    PS: PreimageSource,
    TS: TxSource,
    FS: FriProofSidecarSource,
    TR: TxResultCallback,
>(
    chain_config: ChainConfig,
    block_context: BlockContext,
    tree: T,
    preimage_source: PS,
    tx_source: TS,
    fri_proof_sidecar: FS,
    fri_verifier_artifacts: Option<Arc<FriVerifierArtifacts>>,
    tx_result_callback: TR,
    tracer: &mut impl Tracer<ForwardRunningSystem>,
    validator: &mut impl TxValidator<ForwardRunningSystem>,
) -> Result<BlockOutput, ForwardSubsystemError> {
    let block_metadata_responder = BlockMetadataResponder {
        block_metadata: block_context,
    };
    let chain_config_responder = ChainConfigResponder { chain_config };
    let tx_data_responder = TxDataResponder {
        tx_source,
        next_tx: None,
        next_tx_format: None,
        next_tx_from: None,
    };
    let preimage_responder = GenericPreimageResponder { preimage_source };
    let tree_responder = ReadTreeResponder { tree };
    let fri_proof_responder = FriProofResponder {
        sidecar_source: fri_proof_sidecar,
        artifacts: fri_verifier_artifacts,
    };

    let mut oracle = ZkEENonDeterminismSource::default();
    oracle.add_external_processor(block_metadata_responder);
    oracle.add_external_processor(chain_config_responder);
    oracle.add_external_processor(tx_data_responder);
    oracle.add_external_processor(preimage_responder);
    oracle.add_external_processor(tree_responder);
    oracle.add_external_processor(fri_proof_responder);

    let mut result_keeper = ForwardRunningResultKeeper::new(tx_result_callback);

    run_forward::<BasicBootloaderForwardSimulationConfig>(
        oracle,
        &mut result_keeper,
        tracer,
        validator,
    );
    Ok(result_keeper.into())
}

pub fn generate_proof_input<
    T: ReadStorageTree,
    PS: PreimageSource,
    TS: TxSource,
    FS: FriProofSidecarSource,
    TR: TxResultCallback,
>(
    block_context: BlockContext,
    proof_data: ProofData<StorageCommitment>,
    da_commitment_scheme: DACommitmentScheme,
    tree: T,
    preimage_source: PS,
    tx_source: TS,
    fri_proof_sidecar: FS,
    fri_verifier_artifacts: Option<Arc<FriVerifierArtifacts>>,
    tx_result_callback: TR,
) -> Result<(Vec<u32>, BlockOutput, Vec<u8>), ForwardSubsystemError> {
    generate_proof_input_with_chain_config(
        ChainConfig::default(),
        block_context,
        proof_data,
        da_commitment_scheme,
        tree,
        preimage_source,
        tx_source,
        fri_proof_sidecar,
        fri_verifier_artifacts,
        tx_result_callback,
    )
}

// Returns (prover_input, block_output, pubdata)
pub fn generate_proof_input_with_chain_config<
    T: ReadStorageTree,
    PS: PreimageSource,
    TS: TxSource,
    FS: FriProofSidecarSource,
    TR: TxResultCallback,
>(
    chain_config: ChainConfig,
    block_context: BlockContext,
    proof_data: ProofData<StorageCommitment>,
    da_commitment_scheme: DACommitmentScheme,
    tree: T,
    preimage_source: PS,
    tx_source: TS,
    fri_proof_sidecar: FS,
    fri_verifier_artifacts: Option<Arc<FriVerifierArtifacts>>,
    tx_result_callback: TR,
) -> Result<(Vec<u32>, BlockOutput, Vec<u8>), ForwardSubsystemError> {
    let block_metadata_responder = BlockMetadataResponder {
        block_metadata: block_context,
    };
    let chain_config_responder = ChainConfigResponder { chain_config };
    let tx_data_responder = TxDataResponder {
        tx_source,
        next_tx: None,
        next_tx_format: None,
        next_tx_from: None,
    };
    let zk_proof_data_responder = ZKProofDataResponder {
        data: Some(proof_data),
    };
    let da_commitment_scheme_responder = DACommitmentSchemeResponder {
        da_commitment_scheme: Some(da_commitment_scheme),
    };
    let preimage_responder = GenericPreimageResponder { preimage_source };
    let tree_responder = ReadTreeResponder { tree };
    let fri_proof_responder = FriProofResponder {
        sidecar_source: fri_proof_sidecar,
        artifacts: fri_verifier_artifacts,
    };

    let mut oracle = ZkEENonDeterminismSource::default();
    oracle.add_external_processor(block_metadata_responder);
    oracle.add_external_processor(chain_config_responder);
    oracle.add_external_processor(tx_data_responder);
    oracle.add_external_processor(zk_proof_data_responder);
    oracle.add_external_processor(da_commitment_scheme_responder);
    oracle.add_external_processor(preimage_responder);
    oracle.add_external_processor(tree_responder);
    oracle.add_external_processor(fri_proof_responder);
    oracle.add_external_processor(callable_oracles::arithmetic::NativeArithmeticQuery);
    oracle.add_external_processor(
        callable_oracles::blob_kzg_commitment::NativeBlobCommitmentAndProofQuery,
    );
    oracle.add_external_processor(callable_oracles::field_hints::NativeFieldOpsQuery);

    // We'll wrap the source, to collect all the reads.
    let copy_source = ReadWitnessSource::new(oracle);

    let mut tracer = NopTracer::default();
    let mut result_keeper = ProverInputResultKeeper::new(tx_result_callback);

    let prover_input = run_prover_input_no_panic::<BasicBootloaderProvingExecutionConfig>(
        copy_source,
        &mut result_keeper,
        &mut tracer,
        &mut NopTxValidator,
    )
    .map_err(|e| wrap_error!(e))?;
    let pubdata = std::mem::take(&mut result_keeper.pubdata);
    let block_output = result_keeper.into();

    Ok((prover_input, block_output, pubdata))
}

/// Legacy helper that derives the multiblock witness from per-block witnesses.
///
/// This matches the existing RISC-V-based multiblock flow, where each block is
/// executed independently first and then combined into a batch witness.
///
/// Important: `da_commitment_scheme` must match the scheme used for the
/// per-block proof input generation.
///
/// Single-block prover-input recording includes one chain-config oracle response
/// at the beginning of every block input. The multiblock proving guest reads
/// chain config once after the block count and reuses that frozen value for the
/// whole batch, so the batch input keeps the first response and removes the
/// duplicate responses after asserting they are byte-for-byte equal.
///
pub fn generate_legacy_batch_proof_input(
    blocks_proof_inputs: Vec<&[u32]>,
    da_commitment_scheme: DACommitmentScheme,
    blocks_pubdata: Vec<&[u8]>,
) -> Vec<u32> {
    fn disconnect_marker_idx(block_proof_input: &[u32]) -> usize {
        assert!(
            !block_proof_input.is_empty(),
            "block proof input must contain a disconnect marker"
        );
        let disconnect_marker_idx = block_proof_input.len() - 1;
        assert_eq!(
            block_proof_input[disconnect_marker_idx], 0,
            "expected disconnect query to have an empty response marker"
        );

        disconnect_marker_idx
    }

    let mut trimmed_blocks_proof_inputs = Vec::with_capacity(blocks_proof_inputs.len());
    let blobs_advice = match da_commitment_scheme {
        DACommitmentScheme::BlobsZKsyncOS => {
            let total_pubdata_length: usize = blocks_pubdata
                .iter()
                .map(|blocks_pubdata| blocks_pubdata.len())
                .sum();
            let mut blobs_data = Vec::with_capacity(total_pubdata_length + 31);
            blobs_data.extend_from_slice(&(total_pubdata_length as u64).to_be_bytes());
            blobs_data.extend_from_slice(&[0u8; 23]); // pad to 31
            for (block_proof_input, block_pubdata) in
                blocks_proof_inputs.iter().zip(blocks_pubdata.into_iter())
            {
                blobs_data.extend_from_slice(block_pubdata);
                let advice_words = (block_pubdata.len() + 31).div_ceil(31 * 4096) * 25;
                assert!(
                    block_proof_input.len() > advice_words,
                    "block proof input is too short to contain blob advice and disconnect marker"
                );
                let disconnect_marker_idx = disconnect_marker_idx(block_proof_input);
                let advice_start_idx = disconnect_marker_idx - advice_words;
                trimmed_blocks_proof_inputs.push(block_proof_input[..advice_start_idx].to_vec());
            }
            let mut blobs_advice = Vec::with_capacity(25 * blobs_data.len().div_ceil(31 * 4096));
            for blob_data in blobs_data.chunks(31 * 4096) {
                let advice =
                    callable_oracles::blob_kzg_commitment::blob_kzg_commitment_and_proof(blob_data);
                blobs_advice.push(24);
                for word in advice.iter() {
                    #[cfg(target_pointer_width = "32")]
                    blobs_advice.push(word as u32);
                    #[cfg(target_pointer_width = "64")]
                    {
                        let low = word as u32;
                        let high = (word >> 32) as u32;
                        blobs_advice.push(low);
                        blobs_advice.push(high);
                    }
                }
            }
            blobs_advice
        }
        _ => {
            trimmed_blocks_proof_inputs.extend(blocks_proof_inputs.into_iter().map(
                |block_proof_input| {
                    let disconnect_marker_idx = disconnect_marker_idx(block_proof_input);
                    block_proof_input[..disconnect_marker_idx].to_vec()
                },
            ));
            vec![]
        }
    };
    keep_single_chain_config_response(&mut trimmed_blocks_proof_inputs);
    let mut proof_input = Vec::with_capacity(
        trimmed_blocks_proof_inputs
            .iter()
            .map(|block_proof_input| block_proof_input.len())
            .sum::<usize>()
            + 1
            + blobs_advice.len()
            + 1,
    );
    proof_input.push(trimmed_blocks_proof_inputs.len() as u32);
    for block_proof_input in trimmed_blocks_proof_inputs {
        proof_input.extend_from_slice(block_proof_input.as_slice());
    }
    proof_input.extend_from_slice(blobs_advice.as_slice());
    proof_input.push(0);
    proof_input
}

fn keep_single_chain_config_response(blocks_proof_inputs: &mut [Vec<u32>]) {
    let Some((first, rest)) = blocks_proof_inputs.split_first_mut() else {
        return;
    };
    let prefix_len = chain_config_response_len_in_u32_words();
    assert!(
        first.len() >= prefix_len,
        "block proof input is too short to contain chain config response"
    );
    assert_eq!(
        first[0],
        (ChainConfig::USIZE_LEN * 2) as u32,
        "expected block proof input to start with chain config response length"
    );
    let expected_prefix = first[..prefix_len].to_vec();

    for block_proof_input in rest {
        assert!(
            block_proof_input.len() >= prefix_len,
            "block proof input is too short to contain chain config response"
        );
        assert_eq!(
            &block_proof_input[..prefix_len],
            expected_prefix.as_slice(),
            "multiblock proof input cannot span different chain configs"
        );
        block_proof_input.drain(..prefix_len);
    }
}

fn chain_config_response_len_in_u32_words() -> usize {
    1 + ChainConfig::USIZE_LEN * 2
}

/// Execute a whole batch and return canonical batch prover input and pubdata.
///
/// The caller provides:
/// - the batch pre-state as `initial_proof_data`
/// - the mutable batch state before block 1
/// - per-block metadata and transaction sources
///
/// The runner derives later `ProofData` values internally and mutates the batch
/// state between blocks using the observed `BlockOutput`, so the next block sees
/// the correct pre-state.
pub fn generate_batch_proof_input<BS: BatchState, TS: TxSource>(
    initial_proof_data: ProofData<StorageCommitment>,
    batch_state: BS,
    blocks: Vec<BatchBlockInput<TS>>,
    da_commitment_scheme: DACommitmentScheme,
    chain_config: ChainConfig,
) -> Result<BatchRunOutput, ForwardSubsystemError> {
    assert!(
        !blocks.is_empty(),
        "batch prover input requires at least one block",
    );

    let batch_len = blocks.len();
    let batch_index = batch::BatchIndex::new(batch_len);

    let mut block_metadata = Vec::with_capacity(batch_len);
    let mut tx_sources = Vec::with_capacity(batch_len);

    for block in blocks {
        block_metadata.push(block.block_context);
        tx_sources.push(block.tx_source);
    }
    let proof_data = batch::SharedProofData::new(initial_proof_data);
    let batch_state = batch::BatchStateHandle::new(batch_state);

    let mut oracle = ZkEENonDeterminismSource::default();
    oracle.add_external_processor(ChainConfigResponder { chain_config });
    oracle.add_external_processor(batch::BatchBlockMetadataResponder::new(
        block_metadata,
        batch_index.clone(),
    ));
    oracle.add_external_processor(TxDataResponder {
        tx_source: batch::BatchTxSource::new(tx_sources, batch_index.clone()),
        next_tx: None,
        next_tx_format: None,
        next_tx_from: None,
    });
    oracle.add_external_processor(batch::BatchZKProofDataResponder::new(proof_data.clone()));
    oracle.add_external_processor(batch::BatchDACommitmentSchemeResponder::new(
        da_commitment_scheme,
    ));
    oracle.add_external_processor(GenericPreimageResponder {
        preimage_source: batch_state.clone(),
    });
    oracle.add_external_processor(ReadTreeResponder {
        tree: batch_state.clone(),
    });
    oracle.add_external_processor(callable_oracles::arithmetic::NativeArithmeticQuery);
    oracle.add_external_processor(
        callable_oracles::blob_kzg_commitment::NativeBlobCommitmentAndProofQuery,
    );
    oracle.add_external_processor(callable_oracles::field_hints::NativeFieldOpsQuery);

    // Keep a single witness stream across all block re-entries so the final
    // prover input matches the guest-side multiblock flow.
    let mut oracle = ReadWitnessSource::new(oracle);
    let chain_config = ChainConfig::read_from_oracle(&mut oracle)
        .map_err(BootloaderSubsystemError::from)
        .map_err(wrap_error!())?;
    let mut tracer = NopTracer::default();
    let mut validator = NopTxValidator;
    let mut result_keeper = ProverInputResultKeeper::new(NoopTxCallback);
    let mut batch_data = basic_bootloader::bootloader::block_flow::ZKBatchDataKeeper::new();
    let mut block_outputs = Vec::with_capacity(batch_len);

    for block_idx in 0..batch_len {
        // Re-enter the proving bootloader for the next block while preserving the
        // shared witness stream and the multiblock batch keeper.
        oracle = BatchProverInputBootloader::run_prepared_with_chain_config::<
            BasicBootloaderProvingExecutionConfig,
        >(
            oracle,
            &mut batch_data,
            &mut result_keeper,
            &mut tracer,
            &mut validator,
            chain_config,
        )
        .map_err(wrap_error!())?;

        // `result_keeper` accumulates batch-wide pubdata across re-entries, but
        // `forward_running_rk` contains only the just-finished block output.
        let current_forward_result = std::mem::replace(
            &mut result_keeper.forward_running_rk,
            ForwardRunningResultKeeper::new(NoopTxCallback),
        );
        let block_output = current_forward_result.into();

        if block_idx + 1 != batch_len {
            // Make the current block's writes and newly published preimages
            // visible to the next block in the batch.
            batch_state.apply_block_output(&block_output);
            let next_proof_data = batch_data
                .current_proof_data()
                .expect("batch prover input must expose next proof data");
            proof_data.set(next_proof_data);
            batch_index.advance();
        }

        block_outputs.push(block_output);
    }

    // At this point `batch_data` contains the canonical batch PI/output, while
    // `result_keeper.pubdata` contains the concatenated pubdata for the whole
    // batch.
    let (batch_public_input, batch_output) =
        batch_data.into_public_input_and_output(NullLogger, &mut oracle);
    // Multiblock proving cannot emit the final disconnect from the per-block
    // post-op: only the outer runner knows when the last block has finished.
    <DisconnectOracleQuery as SimpleOracleQuery>::get(&mut oracle, &())
        .expect("disconnect query must not fail");
    let mut prover_input = Vec::with_capacity(1 + oracle.get_read_items().borrow().len());
    prover_input.push(batch_len as u32);
    prover_input.extend(oracle.get_read_items().borrow().iter().copied());

    Ok(BatchRunOutput {
        prover_input,
        pubdata: result_keeper.pubdata,
        batch_public_input,
        batch_output,
        block_outputs,
    })
}

pub fn make_oracle_for_proofs_and_dumps<
    T: ReadStorageTree,
    PS: PreimageSource,
    TS: TxSource,
    FS: FriProofSidecarSource,
>(
    block_context: BlockContext,
    tree: T,
    preimage_source: PS,
    tx_source: TS,
    fri_proof_sidecar: FS,
    fri_verifier_artifacts: Option<Arc<FriVerifierArtifacts>>,
    proof_data: Option<ProofData<StorageCommitment>>,
    da_commitment_scheme: Option<DACommitmentScheme>,
    add_uart: bool,
    use_native_callable_oracles: bool,
) -> ZkEENonDeterminismSource {
    make_oracle_for_proofs_and_dumps_with_chain_config(
        ChainConfig::default(),
        block_context,
        tree,
        preimage_source,
        tx_source,
        fri_proof_sidecar,
        fri_verifier_artifacts,
        proof_data,
        da_commitment_scheme,
        add_uart,
        use_native_callable_oracles,
    )
}

pub fn make_oracle_for_proofs_and_dumps_with_chain_config<
    T: ReadStorageTree,
    PS: PreimageSource,
    TS: TxSource,
    FS: FriProofSidecarSource,
>(
    chain_config: ChainConfig,
    block_context: BlockContext,
    tree: T,
    preimage_source: PS,
    tx_source: TS,
    fri_proof_sidecar: FS,
    fri_verifier_artifacts: Option<Arc<FriVerifierArtifacts>>,
    proof_data: Option<ProofData<StorageCommitment>>,
    da_commitment_scheme: Option<DACommitmentScheme>,
    add_uart: bool,
    use_native_callable_oracles: bool,
) -> ZkEENonDeterminismSource {
    make_oracle_for_proofs_and_dumps_for_init_data_with_chain_config(
        chain_config,
        block_context,
        tree,
        preimage_source,
        tx_source,
        fri_proof_sidecar,
        fri_verifier_artifacts,
        proof_data,
        da_commitment_scheme,
        add_uart,
        use_native_callable_oracles,
    )
}

pub fn make_oracle_for_proofs_and_dumps_for_init_data<
    T: ReadStorageTree,
    PS: PreimageSource,
    TS: TxSource,
    FS: FriProofSidecarSource,
>(
    block_context: BlockContext,
    tree: T,
    preimage_source: PS,
    tx_source: TS,
    fri_proof_sidecar: FS,
    fri_verifier_artifacts: Option<Arc<FriVerifierArtifacts>>,
    proof_data: Option<ProofData<StorageCommitment>>,
    da_commitment_scheme: Option<DACommitmentScheme>,
    add_uart: bool,
    use_native_callable_oracles: bool,
) -> ZkEENonDeterminismSource {
    make_oracle_for_proofs_and_dumps_for_init_data_with_chain_config(
        ChainConfig::default(),
        block_context,
        tree,
        preimage_source,
        tx_source,
        fri_proof_sidecar,
        fri_verifier_artifacts,
        proof_data,
        da_commitment_scheme,
        add_uart,
        use_native_callable_oracles,
    )
}

pub fn make_oracle_for_proofs_and_dumps_for_init_data_with_chain_config<
    T: ReadStorageTree,
    PS: PreimageSource,
    TS: TxSource,
    FS: FriProofSidecarSource,
>(
    chain_config: ChainConfig,
    block_context: BlockContext,
    tree: T,
    preimage_source: PS,
    tx_source: TS,
    fri_proof_sidecar: FS,
    fri_verifier_artifacts: Option<Arc<FriVerifierArtifacts>>,
    proof_data: Option<ProofData<StorageCommitment>>,
    da_commitment_scheme: Option<DACommitmentScheme>,
    add_uart: bool,
    use_native_callable_oracles: bool,
) -> ZkEENonDeterminismSource {
    let block_metadata_responder = BlockMetadataResponder {
        block_metadata: block_context,
    };
    let chain_config_responder = ChainConfigResponder { chain_config };
    let tx_data_responder = TxDataResponder {
        tx_source,
        next_tx: None,
        next_tx_format: None,
        next_tx_from: None,
    };
    let preimage_responder = GenericPreimageResponder { preimage_source };
    let tree_responder = ReadTreeResponder { tree };
    let fri_proof_responder = FriProofResponder {
        sidecar_source: fri_proof_sidecar,
        artifacts: fri_verifier_artifacts,
    };
    let zk_proof_data_responder = ZKProofDataResponder { data: proof_data };
    let da_commitment_scheme_responder = DACommitmentSchemeResponder {
        da_commitment_scheme,
    };

    let mut oracle = ZkEENonDeterminismSource::default();
    oracle.add_external_processor(block_metadata_responder);
    oracle.add_external_processor(chain_config_responder);
    oracle.add_external_processor(tx_data_responder);
    oracle.add_external_processor(preimage_responder);
    oracle.add_external_processor(tree_responder);
    oracle.add_external_processor(fri_proof_responder);
    oracle.add_external_processor(zk_proof_data_responder);
    oracle.add_external_processor(da_commitment_scheme_responder);
    if use_native_callable_oracles {
        oracle.add_external_processor(callable_oracles::arithmetic::NativeArithmeticQuery);
        oracle.add_external_processor(
            callable_oracles::blob_kzg_commitment::NativeBlobCommitmentAndProofQuery,
        );
        oracle.add_external_processor(callable_oracles::field_hints::NativeFieldOpsQuery);
    } else {
        oracle.add_external_processor(callable_oracles::arithmetic::ArithmeticQuery);
        oracle.add_external_processor(
            callable_oracles::blob_kzg_commitment::BlobCommitmentAndProofQuery,
        );
        oracle.add_external_processor(callable_oracles::field_hints::FieldOpsQuery);
    }

    if add_uart {
        let uart_responder = UARTPrintResponder;
        oracle.add_external_processor(uart_responder);
    }

    oracle
}

#[cfg(feature = "testing")]
pub fn run_block_with_oracle_dump<
    T: ReadStorageTree + Clone + serde::Serialize,
    PS: PreimageSource + Clone + serde::Serialize,
    TS: TxSource + Clone + serde::Serialize,
    TR: TxResultCallback,
>(
    block_context: BlockContext,
    tree: T,
    preimage_source: PS,
    tx_source: TS,
    tx_result_callback: TR,
    proof_data: Option<ProofData<StorageCommitment>>,
    da_commitment_scheme: Option<DACommitmentScheme>,
    tracer: &mut impl Tracer<ForwardRunningSystem>,
    validator: &mut impl TxValidator<ForwardRunningSystem>,
) -> Result<BlockOutput, ForwardSubsystemError> {
    run_block_with_oracle_dump_ext::<T, PS, TS, TR, BasicBootloaderForwardSimulationConfig>(
        block_context,
        tree,
        preimage_source,
        tx_source,
        tx_result_callback,
        proof_data,
        da_commitment_scheme,
        tracer,
        validator,
    )
}

#[cfg(feature = "testing")]
pub fn run_block_with_oracle_dump_ext<
    T: ReadStorageTree + Clone + serde::Serialize,
    PS: PreimageSource + Clone + serde::Serialize,
    TS: TxSource + Clone + serde::Serialize,
    TR: TxResultCallback,
    Config: basic_bootloader::bootloader::config::BasicBootloaderExecutionConfig,
>(
    block_context: BlockContext,
    tree: T,
    preimage_source: PS,
    tx_source: TS,
    tx_result_callback: TR,
    proof_data: Option<ProofData<StorageCommitment>>,
    da_commitment_scheme: Option<DACommitmentScheme>,
    tracer: &mut impl Tracer<ForwardRunningSystem>,
    validator: &mut impl TxValidator<ForwardRunningSystem>,
) -> Result<BlockOutput, ForwardSubsystemError> {
    run_block_with_oracle_dump_ext_with_chain_config::<T, PS, TS, TR, Config>(
        ChainConfig::default(),
        block_context,
        tree,
        preimage_source,
        tx_source,
        tx_result_callback,
        proof_data,
        da_commitment_scheme,
        tracer,
        validator,
    )
}

#[cfg(feature = "testing")]
pub fn run_block_with_oracle_dump_ext_with_chain_config<
    T: ReadStorageTree + Clone + serde::Serialize,
    PS: PreimageSource + Clone + serde::Serialize,
    TS: TxSource + Clone + serde::Serialize,
    TR: TxResultCallback,
    Config: basic_bootloader::bootloader::config::BasicBootloaderExecutionConfig,
>(
    chain_config: ChainConfig,
    block_context: BlockContext,
    tree: T,
    preimage_source: PS,
    tx_source: TS,
    tx_result_callback: TR,
    proof_data: Option<ProofData<StorageCommitment>>,
    da_commitment_scheme: Option<DACommitmentScheme>,
    tracer: &mut impl Tracer<ForwardRunningSystem>,
    validator: &mut impl TxValidator<ForwardRunningSystem>,
) -> Result<BlockOutput, ForwardSubsystemError> {
    let block_metadata_responder = BlockMetadataResponder {
        block_metadata: block_context,
    };
    let chain_config_responder = ChainConfigResponder { chain_config };
    let tx_data_responder = TxDataResponder {
        tx_source,
        next_tx: None,
        next_tx_format: None,
        next_tx_from: None,
    };
    let preimage_responder = GenericPreimageResponder { preimage_source };
    let tree_responder = ReadTreeResponder { tree };
    let fri_proof_responder = FriProofResponder {
        sidecar_source: NoFriProofSidecar,
        artifacts: None,
    };
    let zk_proof_data_responder = ZKProofDataResponder { data: proof_data };
    let da_commitment_scheme_responder = DACommitmentSchemeResponder {
        da_commitment_scheme,
    };

    if let Ok(path) = std::env::var("ORACLE_DUMP_FILE") {
        let dump = crate::run::query_processors::ForwardRunningOracleDump {
            zk_proof_data_responder: zk_proof_data_responder.clone(),
            da_commitment_scheme_responder: da_commitment_scheme_responder.clone(),
            chain_config_responder,
            block_metadata_responder,
            tree_responder: tree_responder.clone(),
            tx_data_responder: tx_data_responder.clone(),
            preimage_responder: preimage_responder.clone(),
        };
        let file = std::fs::File::create(path).expect("should create file");
        bincode::serialize_into(file, &dump).expect("should write to file");
    }

    let mut oracle = ZkEENonDeterminismSource::default();
    oracle.add_external_processor(block_metadata_responder);
    oracle.add_external_processor(chain_config_responder);
    oracle.add_external_processor(tx_data_responder);
    oracle.add_external_processor(preimage_responder);
    oracle.add_external_processor(tree_responder);
    oracle.add_external_processor(fri_proof_responder);
    oracle.add_external_processor(zk_proof_data_responder);
    oracle.add_external_processor(da_commitment_scheme_responder);
    oracle.add_external_processor(callable_oracles::arithmetic::ArithmeticQuery);
    oracle
        .add_external_processor(callable_oracles::blob_kzg_commitment::BlobCommitmentAndProofQuery);
    oracle.add_external_processor(callable_oracles::field_hints::FieldOpsQuery);
    oracle.add_external_processor(UARTPrintResponder);

    let mut result_keeper = ForwardRunningResultKeeper::new(tx_result_callback);

    crate::system::bootloader::run_forward_no_panic::<Config>(
        oracle,
        &mut result_keeper,
        tracer,
        validator,
    )
    .map_err(wrap_error!())?;
    Ok(result_keeper.into())
}

#[cfg(feature = "testing")]
pub fn run_block_from_oracle_dump<
    T: ReadStorageTree + Clone + serde::de::DeserializeOwned,
    PS: PreimageSource + Clone + serde::de::DeserializeOwned,
    TS: TxSource + Clone + serde::de::DeserializeOwned,
>(
    path: Option<String>,
    tracer: &mut impl Tracer<ForwardRunningSystem>,
    validator: &mut impl TxValidator<ForwardRunningSystem>,
) -> Result<BlockOutput, ForwardSubsystemError> {
    let path = path.unwrap_or_else(|| std::env::var("ORACLE_DUMP_FILE").unwrap());
    let file = std::fs::File::open(path).expect("should open file");
    let dump: crate::run::query_processors::ForwardRunningOracleDump<T, PS, TS> =
        bincode::deserialize_from(file).expect("should deserialize");

    let crate::run::query_processors::ForwardRunningOracleDump {
        zk_proof_data_responder,
        da_commitment_scheme_responder,
        chain_config_responder,
        block_metadata_responder,
        tree_responder,
        tx_data_responder,
        preimage_responder,
    } = dump;

    let mut oracle = ZkEENonDeterminismSource::default();
    oracle.add_external_processor(block_metadata_responder);
    oracle.add_external_processor(chain_config_responder);
    oracle.add_external_processor(tx_data_responder);
    oracle.add_external_processor(preimage_responder);
    oracle.add_external_processor(tree_responder);
    oracle.add_external_processor(FriProofResponder {
        sidecar_source: NoFriProofSidecar,
        artifacts: None,
    });
    oracle.add_external_processor(zk_proof_data_responder);
    oracle.add_external_processor(da_commitment_scheme_responder);
    oracle.add_external_processor(callable_oracles::arithmetic::ArithmeticQuery);
    oracle
        .add_external_processor(callable_oracles::blob_kzg_commitment::BlobCommitmentAndProofQuery);
    oracle.add_external_processor(callable_oracles::field_hints::FieldOpsQuery);

    let mut result_keeper = ForwardRunningResultKeeper::new(NoopTxCallback);

    run_forward::<BasicBootloaderForwardSimulationConfig>(
        oracle,
        &mut result_keeper,
        tracer,
        validator,
    );
    Ok(result_keeper.into())
}

///
/// Simulate single transaction on top of given state.
/// Some validation steps are skipped (signature check,
/// nonce check and EIP-3607 check)
///
/// Needed for `eth_call` and `eth_estimateGas`.
pub fn simulate_tx<S: ReadStorage, PS: PreimageSource>(
    transaction: EncodedTx,
    block_context: BlockContext,
    storage: S,
    preimage_source: PS,
    tracer: &mut impl Tracer<CallSimulationSystem>,
    validator: &mut impl TxValidator<CallSimulationSystem>,
) -> Result<TxResult, ForwardSubsystemError> {
    simulate_tx_with_chain_config(
        ChainConfig::default(),
        transaction,
        block_context,
        storage,
        preimage_source,
        tracer,
        validator,
    )
}

pub fn simulate_tx_with_chain_config<S: ReadStorage, PS: PreimageSource>(
    chain_config: ChainConfig,
    transaction: EncodedTx,
    block_context: BlockContext,
    storage: S,
    preimage_source: PS,
    tracer: &mut impl Tracer<CallSimulationSystem>,
    validator: &mut impl TxValidator<CallSimulationSystem>,
) -> Result<TxResult, ForwardSubsystemError> {
    let tx_source = TxListSource {
        transactions: vec![transaction].into(),
    };

    let block_metadata_responder = BlockMetadataResponder {
        block_metadata: block_context,
    };
    let chain_config_responder = ChainConfigResponder { chain_config };
    let tx_data_responder = TxDataResponder {
        tx_source,
        next_tx: None,
        next_tx_format: None,
        next_tx_from: None,
    };
    let preimage_responder = GenericPreimageResponder { preimage_source };
    let storage_responder = ReadStorageResponder { storage };
    let fri_proof_responder = FriProofResponder {
        sidecar_source: NoFriProofSidecar,
        artifacts: None,
    };

    let mut oracle = ZkEENonDeterminismSource::default();
    oracle.add_external_processor(block_metadata_responder);
    oracle.add_external_processor(chain_config_responder);
    oracle.add_external_processor(tx_data_responder);
    oracle.add_external_processor(preimage_responder);
    oracle.add_external_processor(storage_responder);
    oracle.add_external_processor(fri_proof_responder);

    let mut result_keeper = ForwardRunningResultKeeper::new(NoopTxCallback);

    let chain_config = ChainConfig::read_from_oracle(&mut oracle)
        .map_err(BootloaderSubsystemError::from)
        .map_err(wrap_error!())?;
    CallSimulationBootloader::run_prepared_with_chain_config::<BasicBootloaderCallSimulationConfig>(
        oracle,
        &mut (),
        &mut result_keeper,
        tracer,
        validator,
        chain_config,
    )
    .map_err(wrap_error!())?;
    let mut block_output: BlockOutput = result_keeper.into();
    Ok(block_output.tx_results.remove(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use zk_ee::common_structs::DACommitmentScheme;

    fn chain_config_response() -> Vec<u32> {
        let len = chain_config_response_len_in_u32_words();
        let mut response = vec![0; len];
        response[0] = (len - 1) as u32;
        response
    }

    #[test]
    fn replaces_per_block_disconnect_with_single_final_disconnect() {
        let chain_config = chain_config_response();
        let mut block_proof_input = chain_config.clone();
        block_proof_input.extend_from_slice(&[11, 12, 24]);
        block_proof_input.extend(100..124);
        block_proof_input.push(0);

        let batch_input = generate_legacy_batch_proof_input(
            vec![block_proof_input.as_slice()],
            DACommitmentScheme::BlobsZKsyncOS,
            vec![&[1, 2, 3]],
        );

        let mut expected = vec![1];
        expected.extend_from_slice(&chain_config);
        expected.extend_from_slice(&[11, 12]);
        expected.extend_from_slice(&blob_advice(&[1, 2, 3]));
        expected.push(0);

        assert_eq!(batch_input, expected);
    }

    fn blob_advice(pubdata: &[u8]) -> Vec<u32> {
        let mut blobs_data = Vec::with_capacity(pubdata.len() + 31);
        blobs_data.extend_from_slice(&(pubdata.len() as u64).to_be_bytes());
        blobs_data.extend_from_slice(&[0u8; 23]);
        blobs_data.extend_from_slice(pubdata);

        let mut blobs_advice = Vec::with_capacity(25 * blobs_data.len().div_ceil(31 * 4096));
        for blob_data in blobs_data.chunks(31 * 4096) {
            let advice =
                callable_oracles::blob_kzg_commitment::blob_kzg_commitment_and_proof(blob_data);
            blobs_advice.push(24);
            for word in advice.iter() {
                #[cfg(target_pointer_width = "32")]
                blobs_advice.push(word as u32);
                #[cfg(target_pointer_width = "64")]
                {
                    let low = word as u32;
                    let high = (word >> 32) as u32;
                    blobs_advice.push(low);
                    blobs_advice.push(high);
                }
            }
        }
        blobs_advice
    }

    #[test]
    fn legacy_batch_input_handles_empty_blob_pubdata() {
        let chain_config = chain_config_response();
        let block_witness_payload = [11, 22, 33];
        let mut single_block_witness = chain_config.clone();
        single_block_witness.extend_from_slice(&block_witness_payload);
        single_block_witness.extend_from_slice(&[100; 25]);
        single_block_witness.push(0);

        let batch_witness = generate_legacy_batch_proof_input(
            vec![single_block_witness.as_slice()],
            DACommitmentScheme::BlobsZKsyncOS,
            vec![&[]],
        );

        let mut expected = vec![1];
        expected.extend_from_slice(&chain_config);
        expected.extend_from_slice(&block_witness_payload);
        expected.extend_from_slice(&blob_advice(&[]));
        expected.push(0);

        assert_eq!(batch_witness, expected);
    }

    #[test]
    fn legacy_batch_input_contains_chain_config_once() {
        let chain_config = chain_config_response();
        let mut first = chain_config.clone();
        first.extend_from_slice(&[11, 12, 0]);
        let mut second = chain_config.clone();
        second.extend_from_slice(&[21, 22, 0]);

        let batch_input = generate_legacy_batch_proof_input(
            vec![first.as_slice(), second.as_slice()],
            DACommitmentScheme::PubdataKeccak256,
            vec![&[], &[]],
        );

        let mut expected = vec![2];
        expected.extend_from_slice(&chain_config);
        expected.extend_from_slice(&[11, 12, 21, 22, 0]);

        assert_eq!(batch_input, expected);
    }

    #[test]
    #[should_panic(expected = "multiblock proof input cannot span different chain configs")]
    fn legacy_batch_input_rejects_different_chain_configs() {
        let mut first = chain_config_response();
        first.extend_from_slice(&[11, 12, 0]);
        let mut second = chain_config_response();
        second[1] = 1;
        second.extend_from_slice(&[21, 22, 0]);

        generate_legacy_batch_proof_input(
            vec![first.as_slice(), second.as_slice()],
            DACommitmentScheme::PubdataKeccak256,
            vec![&[], &[]],
        );
    }
}
