use crate::system::system_types::ForwardBootloader;
use crate::system::system_types::ForwardRunningSystem;
use crate::system::system_types::ProverInputSystem;
use basic_bootloader::bootloader::block_flow::public_input::BatchOutput;
use basic_bootloader::bootloader::block_header as basic_booltoader_block_header;
use basic_bootloader::bootloader::config::BasicBootloaderExecutionConfig;
use basic_bootloader::bootloader::errors::BootloaderSubsystemError;
use basic_bootloader::bootloader::result_keeper::ResultKeeperExt;
use oracle_provider::ReadWitnessSource;
use oracle_provider::ZkEENonDeterminismSource;
use zk_ee::system::metadata::chain_config::ChainConfig;
use zk_ee::system::tracer::Tracer;
use zk_ee::system::validator::TxValidator;
use zk_ee::types_config::EthereumIOTypesConfig;

use super::system_types::ProverInputBootloader;

///
/// Run bootloader with forward system with a given `oracle`.
/// Returns execution results(tx results, state changes, events, etc) via `results_keeper`.
///
pub fn run_forward<Config: BasicBootloaderExecutionConfig>(
    mut oracle: ZkEENonDeterminismSource,
    result_keeper: &mut impl ResultKeeperExt<
        EthereumIOTypesConfig,
        BlockHeader = basic_booltoader_block_header::BlockHeader,
    >,
    tracer: &mut impl Tracer<ForwardRunningSystem>,
    validator: &mut impl TxValidator<ForwardRunningSystem>,
) {
    let chain_config = ChainConfig::read_from_oracle(&mut oracle)
        .unwrap_or_else(|err| panic!("Failed to read chain config: {err}"));
    if let Err(err) = ForwardBootloader::run_prepared::<Config>(
        oracle,
        &mut (),
        result_keeper,
        tracer,
        validator,
        chain_config,
    ) {
        panic!("Forward run failed with: {err}")
    };
}

pub fn run_forward_no_panic<Config: BasicBootloaderExecutionConfig>(
    mut oracle: ZkEENonDeterminismSource,
    result_keeper: &mut impl ResultKeeperExt<
        EthereumIOTypesConfig,
        BlockHeader = basic_booltoader_block_header::BlockHeader,
    >,
    tracer: &mut impl Tracer<ForwardRunningSystem>,
    validator: &mut impl TxValidator<ForwardRunningSystem>,
) -> Result<(), BootloaderSubsystemError> {
    let chain_config = ChainConfig::read_from_oracle(&mut oracle)?;
    ForwardBootloader::run_prepared::<Config>(
        oracle,
        &mut (),
        result_keeper,
        tracer,
        validator,
        chain_config,
    )
    .map(|_| ())
}

pub fn run_prover_input_no_panic<Config: BasicBootloaderExecutionConfig>(
    oracle: ReadWitnessSource,
    result_keeper: &mut impl ResultKeeperExt<
        EthereumIOTypesConfig,
        BlockHeader = basic_booltoader_block_header::BlockHeader,
    >,
    tracer: &mut impl Tracer<ProverInputSystem>,
    validator: &mut impl TxValidator<ProverInputSystem>,
) -> Result<Vec<u32>, BootloaderSubsystemError> {
    run_prover_input_with_batch_output_no_panic::<Config>(oracle, result_keeper, tracer, validator)
        .map(|(prover_input, _)| prover_input)
}

pub fn run_prover_input_with_batch_output_no_panic<Config: BasicBootloaderExecutionConfig>(
    mut oracle: ReadWitnessSource,
    result_keeper: &mut impl ResultKeeperExt<
        EthereumIOTypesConfig,
        BlockHeader = basic_booltoader_block_header::BlockHeader,
    >,
    tracer: &mut impl Tracer<ProverInputSystem>,
    validator: &mut impl TxValidator<ProverInputSystem>,
) -> Result<(Vec<u32>, BatchOutput), BootloaderSubsystemError> {
    let chain_config = ChainConfig::read_from_oracle(&mut oracle)?;
    ProverInputBootloader::run_prepared::<Config>(
        oracle,
        &mut (),
        result_keeper,
        tracer,
        validator,
        chain_config,
    )
    .map(|o| (o.0.get_read_items().borrow().clone(), o.2))
}
