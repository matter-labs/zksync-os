use crate::system::system::*;
use basic_bootloader::bootloader::config::BasicBootloaderExecutionConfig;
use basic_bootloader::bootloader::errors::BootloaderSubsystemError;
use basic_bootloader::bootloader::result_keeper::ResultKeeperExt;
use oracle_provider::DummyMemorySource;
use oracle_provider::ZkEENonDeterminismSource;
use zk_ee::system::tracer::Tracer;

///
/// Run bootloader with forward system with a given `oracle`.
/// Returns execution results(tx results, state changes, events, etc) via `results_keeper`.
///
pub fn run_forward<Config: BasicBootloaderExecutionConfig>(
    oracle: ZkEENonDeterminismSource<DummyMemorySource>,
    result_keeper: &mut impl ResultKeeperExt,
    tracer: &mut impl Tracer<ForwardRunningSystem>,
) {
    if let Err(err) = ForwardBootloader::run_prepared::<Config>(oracle, result_keeper, tracer) {
        panic!("Forward run failed with: {err}")
    };
}

pub fn run_forward_no_panic<Config: BasicBootloaderExecutionConfig>(
    oracle: ZkEENonDeterminismSource<DummyMemorySource>,
    result_keeper: &mut impl ResultKeeperExt,
    tracer: &mut impl Tracer<ForwardRunningSystem>,
) -> Result<(), BootloaderSubsystemError> {
    ForwardBootloader::run_prepared::<Config>(oracle, result_keeper, tracer).map(|_| ())
}
