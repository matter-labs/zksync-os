#![feature(allocator_api)]

use std::{path::PathBuf, str::FromStr};

use forward_system::run::{
    test_impl::{InMemoryPreimageSource, InMemoryTree, TxListSource},
    BlockContext, EthereumIOTypesConfig, ForwardRunningOracle, StorageCommitment,
};
use oracle_provider::{BasicZkEEOracleWrapper, ReadWitnessSource, ZkEENonDeterminismSource};
pub mod helpers;

/// Runs the batch, and returns the output (that contains gas usage, transaction status etc.).
pub use forward_system::run::run_batch;
use zk_ee::common_structs::ProofData;

/// Runs a batch in riscV - using zksync_os binary - and returns the
/// witness that can be passed to the prover subsystem.
pub fn run_batch_generate_witness(
    block_context: BlockContext,
    tree: InMemoryTree,
    preimage_source: InMemoryPreimageSource,
    tx_source: TxListSource,
    proof_data: ProofData<StorageCommitment>,
    zksync_os_bin_path: &str,
) -> Vec<u32> {
    let oracle: ForwardRunningOracle<InMemoryTree, InMemoryPreimageSource, TxListSource> =
        ForwardRunningOracle {
            proof_data: Some(proof_data),
            block_metadata: block_context,
            tree,
            preimage_source,
            tx_source,
            next_tx: None,
        };

    let oracle_wrapper = BasicZkEEOracleWrapper::<EthereumIOTypesConfig, _>::new(oracle.clone());
    let mut non_determinism_source = ZkEENonDeterminismSource::default();
    non_determinism_source.add_external_processor(oracle_wrapper);
    non_determinism_source.add_external_processor(callable_oracles::arithmetic::ArithmeticQuery {
        marker: std::marker::PhantomData,
    });

    // We'll wrap the source, to collect all the reads.
    let copy_source = ReadWitnessSource::new(non_determinism_source);

    let items = copy_source.get_read_items();
    // By default - enable diagnostics is false (which makes the test run faster).
    let path = PathBuf::from_str(zksync_os_bin_path).unwrap();
    let output = zksync_os_runner::run(path, None, 1 << 36, copy_source);

    // We return 0s in case of failure.
    assert_ne!(output, [0u32; 8]);

    let result = items.borrow().clone();
    result
}
