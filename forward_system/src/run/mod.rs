pub mod errors;
pub(crate) mod oracle;
pub mod output;
mod preimage_source;
mod tree;
mod tx_result_callback;
mod tx_source;

pub mod result_keeper;
pub mod test_impl;

use crate::run::result_keeper::ForwardRunningResultKeeper;
use crate::system::bootloader::run_forward;
use crate::system::system::CallSimulationBootloader;
use crate::system::system::CallSimulationSystem;
use crate::system::system::ForwardRunningSystem;
use basic_bootloader::bootloader::config::{
    BasicBootloaderCallSimulationConfig, BasicBootloaderForwardSimulationConfig,
};
use errors::ForwardSubsystemError;
use oracle::CallSimulationOracle;
pub use oracle::ForwardRunningOracle;
use zk_ee::common_structs::ProofData;
use zk_ee::system::tracer::Tracer;

pub use tree::LeafProof;
pub use tree::ReadStorage;
pub use tree::ReadStorageTree;
pub use zk_ee::types_config::EthereumIOTypesConfig;

pub use preimage_source::PreimageSource;
use zk_ee::wrap_error;

use std::fs::File;
use std::io::{Read, Write};
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
use crate::run::output::TxResult;
use crate::run::test_impl::{NoopTxCallback, TxListSource};
pub use basic_bootloader::bootloader::errors::InvalidTransaction;
use basic_system::system_implementation::flat_storage_model::*;
use oracle_provider::{BasicZkEEOracleWrapper, ReadWitnessSource, ZkEENonDeterminismSource};
pub use zk_ee::system::metadata::BlockMetadataFromOracle as BlockContext;

pub type StorageCommitment = FlatStorageCommitment<{ TREE_HEIGHT }>;

pub fn run_batch<T: ReadStorageTree, PS: PreimageSource, TS: TxSource, TR: TxResultCallback>(
    block_context: BlockContext,
    tree: T,
    preimage_source: PS,
    tx_source: TS,
    tx_result_callback: TR,
    tracer: &mut impl Tracer<ForwardRunningSystem<T, PS, TS>>,
) -> Result<BlockOutput, ForwardSubsystemError> {
    let oracle = ForwardRunningOracle {
        proof_data: None,
        block_metadata: block_context,
        tree,
        preimage_source,
        tx_source,
        next_tx: None,
    };

    let mut result_keeper = ForwardRunningResultKeeper::new(tx_result_callback);

    run_forward::<BasicBootloaderForwardSimulationConfig, _, _, _>(
        oracle,
        &mut result_keeper,
        tracer,
    );
    Ok(result_keeper.into())
}

// TODO: we should run it on native arch and it should return pubdata and other outputs via result keeper
pub fn generate_proof_input<T: ReadStorageTree, PS: PreimageSource, TS: TxSource>(
    zk_os_program_path: PathBuf,
    block_context: BlockContext,
    proof_data: ProofData<StorageCommitment>,
    tree: T,
    preimage_source: PS,
    tx_source: TS,
) -> Result<Vec<u32>, ForwardSubsystemError> {
    let oracle = ForwardRunningOracle {
        proof_data: Some(proof_data),
        block_metadata: block_context,
        tree,
        preimage_source,
        tx_source,
        next_tx: None,
    };
    let oracle_wrapper = BasicZkEEOracleWrapper::<EthereumIOTypesConfig, _>::new(oracle);

    let mut non_determinism_source = ZkEENonDeterminismSource::default();
    non_determinism_source.add_external_processor(oracle_wrapper);
    non_determinism_source.add_external_processor(callable_oracles::arithmetic::ArithmeticQuery {
        marker: std::marker::PhantomData,
    });

    // We'll wrap the source, to collect all the reads.
    let copy_source = ReadWitnessSource::new(non_determinism_source);
    let items = copy_source.get_read_items();

    let _proof_output = zksync_os_runner::run(zk_os_program_path, None, 1 << 36, copy_source);

    Ok(std::rc::Rc::try_unwrap(items).unwrap().into_inner())
}

pub fn run_batch_with_oracle_dump<
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
    tracer: &mut impl Tracer<ForwardRunningSystem<T, PS, TS>>,
) -> Result<BlockOutput, ForwardSubsystemError> {
    let oracle = ForwardRunningOracle {
        proof_data: None,
        block_metadata: block_context,
        tree,
        preimage_source,
        tx_source,
        next_tx: None,
    };

    let mut result_keeper = ForwardRunningResultKeeper::new(tx_result_callback);

    if let Ok(path) = std::env::var("ORACLE_DUMP_FILE") {
        let serialized_oracle = bincode::serialize(&oracle).expect("should serialize");
        let mut file = File::create(path).expect("should create file");
        file.write_all(&serialized_oracle)
            .expect("should write to file");
    }

    run_forward::<BasicBootloaderForwardSimulationConfig, _, _, _>(
        oracle,
        &mut result_keeper,
        tracer,
    );
    Ok(result_keeper.into())
}

pub fn run_batch_from_oracle_dump<
    T: ReadStorageTree + Clone + serde::de::DeserializeOwned,
    PS: PreimageSource + Clone + serde::de::DeserializeOwned,
    TS: TxSource + Clone + serde::de::DeserializeOwned,
>(
    path: Option<String>,
    tracer: &mut impl Tracer<ForwardRunningSystem<T, PS, TS>>,
) -> Result<BlockOutput, ForwardSubsystemError> {
    let path = path.unwrap_or_else(|| std::env::var("ORACLE_DUMP_FILE").unwrap());
    let mut file = File::open(path).expect("should open file");
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).expect("should read file");
    let oracle: ForwardRunningOracle<T, PS, TS> =
        bincode::deserialize(&buffer).expect("should deserialize");

    let mut result_keeper = ForwardRunningResultKeeper::new(NoopTxCallback);

    run_forward::<BasicBootloaderForwardSimulationConfig, _, _, _>(
        oracle,
        &mut result_keeper,
        tracer,
    );
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
// TODO: we need to have simplified version of oracle and config to disable tree validation, so we can use `ReadStorage` here
pub fn simulate_tx<S: ReadStorage, PS: PreimageSource>(
    transaction: Vec<u8>,
    block_context: BlockContext,
    storage: S,
    preimage_source: PS,
    tracer: &mut impl Tracer<CallSimulationSystem<S, PS, TxListSource>>,
) -> Result<TxResult, ForwardSubsystemError> {
    let tx_source = TxListSource {
        transactions: vec![transaction].into(),
    };

    let oracle = CallSimulationOracle {
        proof_data: None,
        block_metadata: block_context,
        storage,
        preimage_source,
        tx_source,
        next_tx: None,
    };

    let mut result_keeper = ForwardRunningResultKeeper::new(NoopTxCallback);

    CallSimulationBootloader::run_prepared::<BasicBootloaderCallSimulationConfig>(
        oracle,
        &mut result_keeper,
        tracer,
    )
    .map_err(wrap_error!())?;
    let mut block_output: BlockOutput = result_keeper.into();
    Ok(block_output.tx_results.remove(0))
}
