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
use basic_bootloader::bootloader::config::{
    BasicBootloaderCallSimulationConfig, BasicBootloaderForwardSimulationConfig,
};
use oracle::CallSimulationOracle;
pub use oracle::ForwardRunningOracle;
pub use oracle::ForwardRunningOracleAux;
use zk_ee::common_structs::BasicIOImplementerFSM;
use zk_ee::utils::Bytes32;

pub use tree::LeafProof;
pub use tree::ReadStorage;
pub use tree::ReadStorageTree;
pub use zk_ee::types_config::EthereumIOTypesConfig;

pub use preimage_source::PreimageSource;

use std::fs::File;
use std::io::{Read, Write};
pub use tx_result_callback::TxResultCallback;
pub use tx_source::NextTxResponse;
pub use tx_source::TxSource;

pub use self::output::BatchOutput;
pub use self::output::ExecutionOutput;
pub use self::output::ExecutionResult;
pub use self::output::Log;
pub use self::output::StorageWrite;
pub use self::output::TxOutput;
use crate::run::output::TxResult;
use crate::run::test_impl::{NoopTxCallback, TxListSource};
pub use basic_bootloader::bootloader::errors::InvalidTransaction;
use basic_system::system_implementation::flat_storage_model::*;
use zk_ee::system::errors::InternalError;
pub use zk_ee::system::metadata::BlockMetadataFromOracle as BatchContext;

pub type StorageCommitment = FlatStorageCommitment<{ TREE_HEIGHT }>;

pub fn run_batch<T: ReadStorageTree, PS: PreimageSource, TS: TxSource, TR: TxResultCallback>(
    batch_context: BatchContext,
    tree: T,
    preimage_source: PS,
    tx_source: TS,
    tx_result_callback: TR,
) -> Result<BatchOutput, InternalError> {
    let oracle = ForwardRunningOracle {
        io_implementer_init_data: None,
        block_metadata: batch_context,
        tree,
        preimage_source,
        tx_source,
        next_tx: None,
    };

    let mut result_keeper = ForwardRunningResultKeeper::new(tx_result_callback);

    run_forward::<BasicBootloaderForwardSimulationConfig, _, _, _>(oracle, &mut result_keeper);
    Ok(result_keeper.into())
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
) -> Result<BatchOutput, InternalError> {
    let oracle = ForwardRunningOracle {
        io_implementer_init_data: None,
        block_metadata: batch_context,
        tree,
        preimage_source,
        tx_source,
        next_tx: None,
    };

    let mut result_keeper = ForwardRunningResultKeeper::new(tx_result_callback);

    if let Ok(path) = std::env::var("ORACLE_DUMP_FILE") {
        let aux_oracle: ForwardRunningOracleAux<T, PS, TS> = oracle.clone().into();
        let serialized_oracle = bincode::serialize(&aux_oracle).expect("should serialize");
        let mut file = File::create(path).expect("should create file");
        file.write_all(&serialized_oracle)
            .expect("should write to file");
    }

    run_forward::<BasicBootloaderForwardSimulationConfig, _, _, _>(oracle, &mut result_keeper);
    Ok(result_keeper.into())
}

pub fn run_batch_from_oracle_dump<
    T: ReadStorageTree + Clone + serde::de::DeserializeOwned,
    PS: PreimageSource + Clone + serde::de::DeserializeOwned,
    TS: TxSource + Clone + serde::de::DeserializeOwned,
>(
    path: Option<String>,
) -> Result<BatchOutput, InternalError> {
    let path = path.unwrap_or_else(|| std::env::var("ORACLE_DUMP_FILE").unwrap());
    let mut file = File::open(path).expect("should open file");
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).expect("should read file");
    let oracle_aux: ForwardRunningOracleAux<T, PS, TS> =
        bincode::deserialize(&buffer).expect("should deserialize");
    let oracle: ForwardRunningOracle<T, PS, TS> = oracle_aux.into();

    let mut result_keeper = ForwardRunningResultKeeper::new(NoopTxCallback);

    run_forward::<BasicBootloaderForwardSimulationConfig, _, _, _>(oracle, &mut result_keeper);
    Ok(result_keeper.into())
}

///
/// Simulate single transaction on top of given state.
/// The validation step is skipped, fields that needed for validation can be empty(any).
///
/// Needed for `eth_call` and `eth_estimateGas`.
///
// TODO: we need to have simplified version of oracle and config to disable tree validation, so we can use `ReadStorage` here
pub fn simulate_tx<S: ReadStorage, PS: PreimageSource>(
    transaction: Vec<u8>,
    batch_context: BatchContext,
    storage: S,
    preimage_source: PS,
) -> Result<TxResult, InternalError> {
    let tx_source = TxListSource {
        transactions: vec![transaction].into(),
    };

    let oracle = CallSimulationOracle {
        io_implementer_init_data: None,
        block_metadata: batch_context,
        storage,
        preimage_source,
        tx_source,
        next_tx: None,
    };

    let mut result_keeper = ForwardRunningResultKeeper::new(NoopTxCallback);

    CallSimulationBootloader::run_prepared::<BasicBootloaderCallSimulationConfig>(
        oracle,
        &mut result_keeper,
    );
    let mut batch_output: BatchOutput = result_keeper.into();
    Ok(batch_output.tx_results.remove(0))
}

pub fn io_implementer_init_data(
    storage_commitment: Option<StorageCommitment>,
) -> BasicIOImplementerFSM<StorageCommitment> {
    BasicIOImplementerFSM {
        state_root_view: match storage_commitment {
            Some(storage_commitment) => storage_commitment,
            None => StorageCommitment {
                root: Default::default(),
                next_free_slot: 0,
            },
        },
        pubdata_diffs_log_hash: Bytes32::ZERO,
        num_pubdata_diffs_logs: 0,
        block_functionality_is_completed: false,
    }
}
