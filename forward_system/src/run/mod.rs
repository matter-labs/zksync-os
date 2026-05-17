pub mod errors;
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

use crate::run::query_processors::GenericPreimageResponder;
use crate::run::query_processors::ReadStorageResponder;
use crate::run::query_processors::ReadTreeResponder;
use crate::run::query_processors::TxDataResponder;
use crate::run::query_processors::UARTPrintResponder;
use crate::run::query_processors::ZKProofDataResponder;
use crate::run::query_processors::{BlockMetadataResponder, DACommitmentSchemeResponder};
use crate::run::result_keeper::ForwardRunningResultKeeper;
use crate::system::bootloader::run_forward;
use crate::system::bootloader::run_prover_input_no_panic;
use crate::system::system_types::CallSimulationBootloader;
use crate::system::system_types::CallSimulationSystem;
use crate::system::system_types::ForwardRunningSystem;
use airbender_codec::{AirbenderCodec, AirbenderCodecV0};
use airbender_host::Inputs;
use basic_bootloader::bootloader::config::{
    BasicBootloaderCallSimulationConfig, BasicBootloaderForwardSimulationConfig,
    BasicBootloaderProvingExecutionConfig,
};
use errors::ForwardSubsystemError;
use oracle_provider::witness_recording::WitnessRecordingOracle;
use oracle_provider::ZkEENonDeterminismSource;
use result_keeper::ProverInputResultKeeper;
use zk_ee::common_structs::ProofData;
use zk_ee::system::tracer::NopTracer;
use zk_ee::system::tracer::Tracer;

pub use interface_impl::RunBlockForward;
pub use tree::LeafProof;
pub use tree::ReadStorage;
pub use tree::ReadStorageTree;
use zk_ee::system::validator::NopTxValidator;
use zk_ee::system::validator::TxValidator;
pub use zk_ee::types_config::EthereumIOTypesConfig;

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
pub use zk_ee::system::metadata::zk_metadata::BlockMetadataFromOracle as BlockContext;
use zksync_os_interface::traits::TxListSource;

pub type StorageCommitment = FlatStorageCommitment<{ TREE_HEIGHT }>;

pub fn run_block<T: ReadStorageTree, PS: PreimageSource, TS: TxSource, TR: TxResultCallback>(
    block_context: BlockContext,
    tree: T,
    preimage_source: PS,
    tx_source: TS,
    tx_result_callback: TR,
    tracer: &mut impl Tracer<ForwardRunningSystem>,
    validator: &mut impl TxValidator<ForwardRunningSystem>,
) -> Result<BlockOutput, ForwardSubsystemError> {
    let block_metadata_responder = BlockMetadataResponder {
        block_metadata: block_context,
    };
    let tx_data_responder = TxDataResponder {
        tx_source,
        next_tx: None,
        next_tx_format: None,
        next_tx_from: None,
    };
    let preimage_responder = GenericPreimageResponder { preimage_source };
    let tree_responder = ReadTreeResponder { tree };

    let mut oracle = ZkEENonDeterminismSource::default();
    oracle.add_external_processor(block_metadata_responder);
    oracle.add_external_processor(tx_data_responder);
    oracle.add_external_processor(preimage_responder);
    oracle.add_external_processor(tree_responder);

    let mut result_keeper = ForwardRunningResultKeeper::new(tx_result_callback);

    run_forward::<BasicBootloaderForwardSimulationConfig>(
        oracle,
        &mut result_keeper,
        tracer,
        validator,
    );
    Ok(result_keeper.into())
}

// Returns (prover_input, block_output, pubdata)
pub fn generate_proof_input<
    T: ReadStorageTree,
    PS: PreimageSource,
    TS: TxSource,
    TR: TxResultCallback,
>(
    block_context: BlockContext,
    proof_data: ProofData<StorageCommitment>,
    da_commitment_scheme: DACommitmentScheme,
    tree: T,
    preimage_source: PS,
    tx_source: TS,
    tx_result_callback: TR,
) -> Result<(Vec<u32>, BlockOutput, Vec<u8>), ForwardSubsystemError> {
    let block_metadata_responder = BlockMetadataResponder {
        block_metadata: block_context,
    };
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

    let mut oracle = ZkEENonDeterminismSource::default();
    oracle.add_external_processor(block_metadata_responder);
    oracle.add_external_processor(tx_data_responder);
    oracle.add_external_processor(zk_proof_data_responder);
    oracle.add_external_processor(da_commitment_scheme_responder);
    oracle.add_external_processor(preimage_responder);
    oracle.add_external_processor(tree_responder);
    oracle.add_external_processor(callable_oracles::arithmetic::NativeArithmeticQuery);
    oracle.add_external_processor(
        callable_oracles::blob_kzg_commitment::NativeBlobCommitmentAndProofQuery,
    );
    oracle.add_external_processor(callable_oracles::field_hints::NativeFieldOpsQuery);

    // We'll wrap the source, to collect all the reads.
    let recording_oracle = WitnessRecordingOracle::new(oracle);

    let mut tracer = NopTracer::default();
    let mut result_keeper = ProverInputResultKeeper::new(tx_result_callback);

    let inputs = run_prover_input_no_panic::<BasicBootloaderProvingExecutionConfig>(
        recording_oracle,
        &mut result_keeper,
        &mut tracer,
        &mut NopTxValidator,
    )
    .map_err(|e| wrap_error!(e))?;
    // Take pubdata, as it's not part of BlockOutput
    let pubdata = std::mem::take(&mut result_keeper.pubdata);
    let prover_input = inputs.words().to_vec();

    Ok((prover_input, result_keeper.into(), pubdata))
}

// TODO(EVM-1184): in future we should generate input per batch
///
/// Generate batch proof input from blocks proof inputs.
///
/// Important: da_commitment_scheme should correspond to one used for blocks proof input generation.
///
pub fn generate_batch_proof_input(
    blocks_proof_inputs: Vec<&[u32]>,
    da_commitment_scheme: DACommitmentScheme,
    blocks_pubdata: Vec<&[u8]>,
) -> Vec<u32> {
    use airbender_host::Inputs;

    // Build batch count as a framed bincode value (matching what the guest reads
    // via oracle.query(0xdeadbeef, &()) → ProvingOracle → read::<usize>())
    let mut batch_header = Inputs::new();
    batch_header
        .push(&blocks_proof_inputs.len())
        .expect("encode batch count");

    // For blob DA, we need to strip per-block blob advice from each block's proof
    // input and recompute it for the whole batch. The block proof inputs are in
    // airbender wire format (framed bincode u32 words), so we strip from the end.
    let mut trimmed_blocks: Vec<Vec<u32>> = Vec::with_capacity(blocks_proof_inputs.len());
    let blobs_advice: Inputs = match da_commitment_scheme {
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

                // Each block's proof input ends with:
                //   ... [blob_advice_frame] [disconnect_response_frame]
                // The disconnect response is a framed empty tuple ().
                // The blob advice is a framed KZGCommitmentAndProof per blob.
                // Count how many blob frames to strip.
                let num_blobs = (block_pubdata.len() + 31).div_ceil(31 * 4096);

                // Each KZGCommitmentAndProof: 96 bytes. bincode serialize_bytes
                // adds a varint length prefix (1 byte for len < 251), total 97 bytes.
                // Wire frame: 1 length word + ceil(97/4) = 1 + 25 = 26 words.
                let kzg_frame_words = {
                    let kzg_encoded = AirbenderCodecV0::encode(
                        &basic_bootloader::bootloader::block_flow::zk::da_commitment_generator::KZGCommitmentAndProof {
                            commitment: [0u8; 48],
                            proof: [0u8; 48],
                        },
                    )
                    .expect("encode KZG size probe");
                    1 + kzg_encoded.len().div_ceil(4)
                };

                // Disconnect frame: framed () — bincode encodes () as 0 bytes,
                // so the frame is just the length word (0).
                let disconnect_frame_words = {
                    let disconnect_encoded =
                        AirbenderCodecV0::encode(&()).expect("encode disconnect size probe");
                    1 + disconnect_encoded.len().div_ceil(4)
                };

                let words_to_strip = num_blobs * kzg_frame_words + disconnect_frame_words;
                assert!(
                    block_proof_input.len() > words_to_strip,
                    "block proof input too short for blob advice + disconnect"
                );
                let trim_point = block_proof_input.len() - words_to_strip;
                // Keep everything up to the blob advice, then append disconnect frame
                let disconnect_start = block_proof_input.len() - disconnect_frame_words;
                let mut trimmed = block_proof_input[..trim_point].to_vec();
                trimmed.extend_from_slice(&block_proof_input[disconnect_start..]);
                trimmed_blocks.push(trimmed);
            }

            // Recompute blob advice for the whole batch
            let mut advice = Inputs::new();
            for blob_data in blobs_data.chunks(31 * 4096) {
                let kzg =
                    callable_oracles::blob_kzg_commitment::blob_kzg_commitment_and_proof(blob_data);
                advice.push(&kzg).expect("encode KZG batch advice");
            }
            advice
        }
        _ => {
            trimmed_blocks.extend(
                blocks_proof_inputs
                    .iter()
                    .map(|block_proof_input| block_proof_input.to_vec()),
            );
            Inputs::new()
        }
    };

    let mut proof_input = Vec::with_capacity(
        batch_header.words().len()
            + trimmed_blocks.iter().map(|b| b.len()).sum::<usize>()
            + blobs_advice.words().len(),
    );
    proof_input.extend_from_slice(batch_header.words());
    for block_proof_input in trimmed_blocks {
        proof_input.extend_from_slice(&block_proof_input);
    }
    proof_input.extend_from_slice(blobs_advice.words());
    proof_input
}

#[cfg(test)]
mod tests {
    use super::generate_batch_proof_input;
    use airbender_codec::{AirbenderCodec, AirbenderCodecV0};
    use airbender_core::wire::frame_words_from_bytes;
    use airbender_guest::input::read_with;
    use airbender_guest::transport::MockTransport;
    use zk_ee::common_structs::DACommitmentScheme;

    #[test]
    fn batch_count_is_readable_via_transport() {
        let block1_words: Vec<u32> = vec![0xAA, 0xBB]; // dummy block proof input
        let block2_words: Vec<u32> = vec![0xCC, 0xDD];

        let batch_input = generate_batch_proof_input(
            vec![block1_words.as_slice(), block2_words.as_slice()],
            DACommitmentScheme::BlobsAndPubdataKeccak256,
            vec![&[], &[]],
        );

        // The batch_input should start with a framed bincode usize (the count = 2),
        // followed by the concatenated block proof inputs.
        let mut transport = MockTransport::new(batch_input);
        let count: usize = read_with(&mut transport).unwrap();
        assert_eq!(count, 2);
    }
}

pub fn make_oracle_for_proofs_and_dumps<T: ReadStorageTree, PS: PreimageSource, TS: TxSource>(
    block_context: BlockContext,
    tree: T,
    preimage_source: PS,
    tx_source: TS,
    proof_data: Option<ProofData<StorageCommitment>>,
    da_commitment_scheme: Option<DACommitmentScheme>,
    add_uart: bool,
    use_native_callable_oracles: bool,
) -> ZkEENonDeterminismSource {
    make_oracle_for_proofs_and_dumps_for_init_data(
        block_context,
        tree,
        preimage_source,
        tx_source,
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
>(
    block_context: BlockContext,
    tree: T,
    preimage_source: PS,
    tx_source: TS,
    proof_data: Option<ProofData<StorageCommitment>>,
    da_commitment_scheme: Option<DACommitmentScheme>,
    add_uart: bool,
    use_native_callable_oracles: bool,
) -> ZkEENonDeterminismSource {
    let block_metadata_responder = BlockMetadataResponder {
        block_metadata: block_context,
    };
    let tx_data_responder = TxDataResponder {
        tx_source,
        next_tx: None,
        next_tx_format: None,
        next_tx_from: None,
    };
    let preimage_responder = GenericPreimageResponder { preimage_source };
    let tree_responder = ReadTreeResponder { tree };
    let zk_proof_data_responder = ZKProofDataResponder { data: proof_data };
    let da_commitment_scheme_responder = DACommitmentSchemeResponder {
        da_commitment_scheme,
    };

    let mut oracle = ZkEENonDeterminismSource::default();
    oracle.add_external_processor(block_metadata_responder);
    oracle.add_external_processor(tx_data_responder);
    oracle.add_external_processor(preimage_responder);
    oracle.add_external_processor(tree_responder);
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
    let block_metadata_responder = BlockMetadataResponder {
        block_metadata: block_context,
    };
    let tx_data_responder = TxDataResponder {
        tx_source,
        next_tx: None,
        next_tx_format: None,
        next_tx_from: None,
    };
    let preimage_responder = GenericPreimageResponder { preimage_source };
    let tree_responder = ReadTreeResponder { tree };
    let zk_proof_data_responder = ZKProofDataResponder { data: proof_data };
    let da_commitment_scheme_responder = DACommitmentSchemeResponder {
        da_commitment_scheme,
    };

    if let Ok(path) = std::env::var("ORACLE_DUMP_FILE") {
        let dump = crate::run::query_processors::ForwardRunningOracleDump {
            zk_proof_data_responder: zk_proof_data_responder.clone(),
            da_commitment_scheme_responder: da_commitment_scheme_responder.clone(),
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
    oracle.add_external_processor(tx_data_responder);
    oracle.add_external_processor(preimage_responder);
    oracle.add_external_processor(tree_responder);
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
        block_metadata_responder,
        tree_responder,
        tx_data_responder,
        preimage_responder,
    } = dump;

    let mut oracle = ZkEENonDeterminismSource::default();
    oracle.add_external_processor(block_metadata_responder);
    oracle.add_external_processor(tx_data_responder);
    oracle.add_external_processor(preimage_responder);
    oracle.add_external_processor(tree_responder);
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
    let tx_source = TxListSource {
        transactions: vec![transaction].into(),
    };

    let block_metadata_responder = BlockMetadataResponder {
        block_metadata: block_context,
    };
    let tx_data_responder = TxDataResponder {
        tx_source,
        next_tx: None,
        next_tx_format: None,
        next_tx_from: None,
    };
    let preimage_responder = GenericPreimageResponder { preimage_source };
    let storage_responder = ReadStorageResponder { storage };

    let mut oracle = ZkEENonDeterminismSource::default();
    oracle.add_external_processor(block_metadata_responder);
    oracle.add_external_processor(tx_data_responder);
    oracle.add_external_processor(preimage_responder);
    oracle.add_external_processor(storage_responder);

    let mut result_keeper = ForwardRunningResultKeeper::new(NoopTxCallback);

    CallSimulationBootloader::run_prepared::<BasicBootloaderCallSimulationConfig>(
        oracle,
        &mut (),
        &mut result_keeper,
        tracer,
        validator,
    )
    .map_err(wrap_error!())?;
    let mut block_output: BlockOutput = result_keeper.into();
    Ok(block_output.tx_results.remove(0))
}
