pub mod errors;
pub mod output;
mod preimage_source;
mod tree;
mod tx_result_callback;
mod tx_source;

pub(crate) mod query_processors;
pub mod result_keeper;
pub mod test_impl;

use crate::run::result_keeper::ForwardRunningResultKeeper;
use crate::system::bootloader::run_forward;
use crate::system::system::ForwardBootloader;
use crate::system::system::ForwardRunningSystem;
use basic_bootloader::bootloader::config::{
    BasicBootloaderCallSimulationConfig, BasicBootloaderExecutionConfig,
    BasicBootloaderForwardSimulationConfig,
};
use errors::ForwardSubsystemError;
use oracle_provider::MemorySource;
use zk_ee::common_structs::ProofData;
use zk_ee::system::tracer::Tracer;
use zk_ee::utils::Bytes32;

pub use tree::LeafProof;
pub use tree::ReadStorage;
pub use tree::ReadStorageTree;
pub use zk_ee::types_config::EthereumIOTypesConfig;

pub use preimage_source::PreimageSource;
use zk_ee::wrap_error;

use std::fs::File;
use std::path::PathBuf;
pub use tx_result_callback::TxResultCallback;
pub use tx_source::NextTxResponse;
pub use tx_source::TxSource;

pub use self::output::BlockOutput;
pub use self::output::ExecutionOutput;
pub use self::output::ExecutionResult;
pub use self::output::Log;
pub use self::output::StorageWrite;
pub use self::output::TxOutput;
pub use self::output::TxResult;
use crate::run::test_impl::{NoopTxCallback, TxListSource};
pub use basic_bootloader::bootloader::errors::InvalidTransaction;
use basic_system::system_implementation::flat_storage_model::*;
use oracle_provider::{ReadWitnessSource, ZkEENonDeterminismSource};
pub use zk_ee::system::metadata::BlockMetadataFromOracle as BatchContext;

pub use self::query_processors::*;

pub type StorageCommitment = FlatStorageCommitment<{ TREE_HEIGHT }>;

pub fn run_batch<T: ReadStorageTree, PS: PreimageSource, TS: TxSource, TR: TxResultCallback>(
    batch_context: BatchContext,
    tree: T,
    preimage_source: PS,
    tx_source: TS,
    tx_result_callback: TR,
    tracer: &mut impl Tracer<ForwardRunningSystem>,
) -> Result<BlockOutput, ForwardSubsystemError> {
    let block_metadata_reponsder = BlockMetadataResponder {
        block_metadata: batch_context,
    };
    let tx_data_reponder = TxDataResponder {
        tx_source,
        next_tx: None,
    };
    let preimage_responder = GenericPreimageResponder { preimage_source };
    let tree_responder = ReadTreeResponder { tree };

    let mut oracle = ZkEENonDeterminismSource::default();
    oracle.add_external_processor(block_metadata_reponsder);
    oracle.add_external_processor(tx_data_reponder);
    oracle.add_external_processor(preimage_responder);
    oracle.add_external_processor(tree_responder);

    let mut result_keeper = ForwardRunningResultKeeper::new(tx_result_callback);

    run_forward::<BasicBootloaderForwardSimulationConfig>(oracle, &mut result_keeper, tracer);
    Ok(result_keeper.into())
}

// TODO: we should run it on native arch and it should return pubdata and other outputs via result keeper
pub fn generate_proof_input<T: ReadStorageTree, PS: PreimageSource, TS: TxSource>(
    zk_os_program_path: PathBuf,
    batch_context: BatchContext,
    proof_data: ProofData<StorageCommitment>,
    tree: T,
    preimage_source: PS,
    tx_source: TS,
) -> Result<Vec<u32>, ForwardSubsystemError> {
    let block_metadata_reponsder = BlockMetadataResponder {
        block_metadata: batch_context,
    };
    let tx_data_reponder = TxDataResponder {
        tx_source,
        next_tx: None,
    };
    let zk_proof_data_responder = ZKProofDataResponder {
        data: Some(proof_data),
    };
    let preimage_responder = GenericPreimageResponder { preimage_source };
    let tree_responder = ReadTreeResponder { tree };

    let mut oracle = ZkEENonDeterminismSource::default();
    oracle.add_external_processor(block_metadata_reponsder);
    oracle.add_external_processor(tx_data_reponder);
    oracle.add_external_processor(zk_proof_data_responder);
    oracle.add_external_processor(preimage_responder);
    oracle.add_external_processor(tree_responder);
    oracle.add_external_processor(callable_oracles::arithmetic::ArithmeticQuery::default());

    // We'll wrap the source, to collect all the reads.
    let copy_source = ReadWitnessSource::new(oracle);
    let items = copy_source.get_read_items();

    let _proof_output = zksync_os_runner::run(zk_os_program_path, None, 1 << 36, copy_source);

    Ok(std::rc::Rc::try_unwrap(items).unwrap().into_inner())
}

pub fn make_oracle_for_proofs_and_dumps<
    T: ReadStorageTree,
    PS: PreimageSource,
    TS: TxSource,
    M: MemorySource + 'static,
>(
    batch_context: BatchContext,
    tree: T,
    preimage_source: PS,
    tx_source: TS,
    proof_data: Option<ProofData<StorageCommitment>>,
    add_uart: bool,
) -> ZkEENonDeterminismSource<M> {
    make_oracle_for_proofs_and_dumps_for_init_data(
        batch_context,
        tree,
        preimage_source,
        tx_source,
        proof_data,
        add_uart,
    )
}

pub fn make_oracle_for_proofs_and_dumps_for_init_data<
    T: ReadStorageTree,
    PS: PreimageSource,
    TS: TxSource,
    M: MemorySource + 'static,
>(
    batch_context: BatchContext,
    tree: T,
    preimage_source: PS,
    tx_source: TS,
    proof_data: Option<ProofData<StorageCommitment>>,
    add_uart: bool,
) -> ZkEENonDeterminismSource<M> {
    let block_metadata_reponsder = BlockMetadataResponder {
        block_metadata: batch_context,
    };
    let tx_data_responder = TxDataResponder {
        tx_source,
        next_tx: None,
    };
    let preimage_responder = GenericPreimageResponder { preimage_source };
    let tree_responder = ReadTreeResponder { tree };
    let zk_proof_data_responder = ZKProofDataResponder { data: proof_data };

    let mut oracle = ZkEENonDeterminismSource::default();
    oracle.add_external_processor(block_metadata_reponsder);
    oracle.add_external_processor(tx_data_responder);
    oracle.add_external_processor(preimage_responder);
    oracle.add_external_processor(tree_responder);
    oracle.add_external_processor(zk_proof_data_responder);
    oracle.add_external_processor(callable_oracles::arithmetic::ArithmeticQuery::default());

    if add_uart {
        let uart_responder = UARTPrintReponsder;
        oracle.add_external_processor(uart_responder);
    }

    oracle
}

pub fn run_batch_with_oracle_dump<
    T: ReadStorageTree + Clone + serde::Serialize,
    PS: PreimageSource + Clone + serde::Serialize,
    TS: TxSource + Clone + serde::Serialize,
    TR: TxResultCallback,
>(
    batch_context: BatchContext,
    tree: T,
    preimage_source: PS,
    tx_source: TS,
    tx_result_callback: TR,
    proof_data: Option<ProofData<StorageCommitment>>,
    tracer: &mut impl Tracer<ForwardRunningSystem>,
) -> Result<BlockOutput, ForwardSubsystemError> {
    run_batch_with_oracle_dump_ext::<T, PS, TS, TR, BasicBootloaderForwardSimulationConfig>(
        batch_context,
        tree,
        preimage_source,
        tx_source,
        tx_result_callback,
        proof_data,
        tracer,
    )
}

pub fn run_batch_with_oracle_dump_ext<
    T: ReadStorageTree + Clone + serde::Serialize,
    PS: PreimageSource + Clone + serde::Serialize,
    TS: TxSource + Clone + serde::Serialize,
    TR: TxResultCallback,
    Config: BasicBootloaderExecutionConfig,
>(
    batch_context: BatchContext,
    tree: T,
    preimage_source: PS,
    tx_source: TS,
    tx_result_callback: TR,
    proof_data: Option<ProofData<StorageCommitment>>,
    tracer: &mut impl Tracer<ForwardRunningSystem>,
) -> Result<BlockOutput, ForwardSubsystemError> {
    let block_metadata_reponsder = BlockMetadataResponder {
        block_metadata: batch_context,
    };
    let tx_data_responder = TxDataResponder {
        tx_source,
        next_tx: None,
    };
    let preimage_responder = GenericPreimageResponder { preimage_source };
    let tree_responder = ReadTreeResponder { tree };
    let zk_proof_data_responder = ZKProofDataResponder { data: proof_data };

    if let Ok(path) = std::env::var("ORACLE_DUMP_FILE") {
        let dump = ForwardRunningOracleDump {
            zk_proof_data_responder: zk_proof_data_responder.clone(),
            block_metadata_reponsder,
            tree_responder: tree_responder.clone(),
            tx_data_responder: tx_data_responder.clone(),
            preimage_responder: preimage_responder.clone(),
        };
        let file = File::create(path).expect("should create file");
        bincode::serialize_into(file, &dump).expect("should write to file");
    }

    let mut oracle = ZkEENonDeterminismSource::default();
    oracle.add_external_processor(block_metadata_reponsder);
    oracle.add_external_processor(tx_data_responder);
    oracle.add_external_processor(preimage_responder);
    oracle.add_external_processor(tree_responder);
    oracle.add_external_processor(zk_proof_data_responder);
    oracle.add_external_processor(callable_oracles::arithmetic::ArithmeticQuery::default());
    oracle.add_external_processor(UARTPrintReponsder);

    let mut result_keeper = ForwardRunningResultKeeper::new(tx_result_callback);

    run_forward::<Config>(oracle, &mut result_keeper, tracer);
    Ok(result_keeper.into())
}

pub fn run_block_from_oracle_dump<
    T: ReadStorageTree + Clone + serde::de::DeserializeOwned,
    PS: PreimageSource + Clone + serde::de::DeserializeOwned,
    TS: TxSource + Clone + serde::de::DeserializeOwned,
>(
    path: Option<String>,
    tracer: &mut impl Tracer<ForwardRunningSystem>,
) -> Result<BlockOutput, ForwardSubsystemError> {
    let path = path.unwrap_or_else(|| std::env::var("ORACLE_DUMP_FILE").unwrap());
    let file = File::open(path).expect("should open file");
    let dump: ForwardRunningOracleDump<T, PS, TS> =
        bincode::deserialize_from(file).expect("should deserialize");

    let ForwardRunningOracleDump {
        zk_proof_data_responder,
        block_metadata_reponsder,
        tree_responder,
        tx_data_responder,
        preimage_responder,
    } = dump;

    let mut oracle = ZkEENonDeterminismSource::default();
    oracle.add_external_processor(block_metadata_reponsder);
    oracle.add_external_processor(tx_data_responder);
    oracle.add_external_processor(preimage_responder);
    oracle.add_external_processor(tree_responder);
    oracle.add_external_processor(zk_proof_data_responder);
    oracle.add_external_processor(callable_oracles::arithmetic::ArithmeticQuery::default());

    let mut result_keeper = ForwardRunningResultKeeper::new(NoopTxCallback);

    run_forward::<BasicBootloaderForwardSimulationConfig>(oracle, &mut result_keeper, tracer);
    Ok(result_keeper.into())
}

///
/// Simulate single transaction on top of given state.
/// The validation step is skipped, fields that needed for validation can be empty(any).
/// Note that, as the validation step is skipped, an internal error is returned
/// if the sender does not have enough balance for the top-level call value transfer.
///
/// Needed for `eth_call` and `eth_estimateGas`.
///
pub fn simulate_tx<S: ReadStorage, PS: PreimageSource>(
    transaction: Vec<u8>,
    block_context: BatchContext,
    storage: S,
    preimage_source: PS,
    tracer: &mut impl Tracer<ForwardRunningSystem>,
) -> Result<TxResult, ForwardSubsystemError> {
    let tx_source = TxListSource {
        transactions: vec![transaction].into(),
    };

    let block_metadata_reponsder = BlockMetadataResponder {
        block_metadata: block_context,
    };
    let tx_data_reponder = TxDataResponder {
        tx_source,
        next_tx: None,
    };
    let preimage_responder = GenericPreimageResponder { preimage_source };
    let storage_responder = ReadStorageResponder { storage };

    let mut oracle = ZkEENonDeterminismSource::default();
    oracle.add_external_processor(block_metadata_reponsder);
    oracle.add_external_processor(tx_data_reponder);
    oracle.add_external_processor(preimage_responder);
    oracle.add_external_processor(storage_responder);

    let mut result_keeper = ForwardRunningResultKeeper::new(NoopTxCallback);

    ForwardBootloader::run::<BasicBootloaderCallSimulationConfig>(
        oracle,
        &mut result_keeper,
        tracer,
    )
    .map_err(wrap_error!())?;
    let mut block_output: BlockOutput = result_keeper.into();
    Ok(block_output.tx_results.remove(0))
}
