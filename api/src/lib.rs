#![feature(allocator_api)]

use std::{path::PathBuf, str::FromStr};

use forward_system::run::{
    io_implementer_init_data,
    test_impl::{InMemoryPreimageSource, InMemoryTree, TxListSource},
    BatchContext, EthereumIOTypesConfig, ForwardRunningOracle, StorageCommitment,
};
use oracle_provider::{BasicZkEEOracleWrapper, ReadWitnessSource, ZkEENonDeterminismSource};
pub mod helpers;

/// Runs the batch, and returns the output (that contains gas usage, transaction status etc.).
pub use forward_system::run::run_batch;

/// Runs a batch in riscV - using zksync_os binary - and returns the
/// witness that can be passed to the prover subsystem.
pub fn run_batch_generate_witness(
    batch_context: BatchContext,
    tree: InMemoryTree,
    preimage_source: InMemoryPreimageSource,
    tx_source: TxListSource,
    storage_commitment: StorageCommitment,
    zksync_os_bin_path: &str,
) -> Vec<u32> {
    let oracle: ForwardRunningOracle<InMemoryTree, InMemoryPreimageSource, TxListSource> =
        ForwardRunningOracle {
            io_implementer_init_data: Some(io_implementer_init_data(Some(storage_commitment))),
            block_metadata: batch_context,
            tree,
            preimage_source,
            tx_source,
            next_tx: None,
        };

    let oracle_wrapper = BasicZkEEOracleWrapper::<EthereumIOTypesConfig, _>::new(oracle.clone());
    let mut non_determinism_source = ZkEENonDeterminismSource::default();
    non_determinism_source.add_external_processor(oracle_wrapper);

    // We'll wrap the source, to collect all the reads.
    let copy_source = ReadWitnessSource::new(non_determinism_source);

    let items = copy_source.get_read_items();
    // By default - enable diagnostics is false (which makes the test run faster).
    let path = PathBuf::from_str(zksync_os_bin_path).unwrap();
    let output = zksync_os_runner::run(path, None, 1 << 30, copy_source);

    // We return 0s in case of failure.
    assert_ne!(output, [0u32; 8]);

    let result = items.borrow().clone();
    result
}
