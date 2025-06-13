use std::alloc::Global;

use crate::run::oracle::CallSimulationOracle;
use crate::run::oracle::ForwardRunningOracle;
use basic_bootloader::bootloader::BasicBootloader;
use basic_system::system_functions::NoStdSystemFunctions;
use basic_system::system_implementation::system::EthereumLikeStorageAccessCostModel;
use basic_system::system_implementation::system::FullIO;
use zk_ee::memory::stack_trait::VecStackCtor;
use zk_ee::reference_implementations::BaseResources;
use zk_ee::system::{EthereumLikeTypes, SystemTypes};
use zk_ee::system_io_oracle::IOOracle;
use zk_ee::types_config::EthereumIOTypesConfig;

#[cfg(not(feature = "no_print"))]
type Logger = crate::system::logger::StdIOLogger;

#[cfg(feature = "no_print")]
type Logger = zk_ee::system::NullLogger;

pub struct ForwardSystemTypes<O>(O);

#[cfg(feature = "unlimited_native")]
type Native = zk_ee::reference_implementations::IncreasingNative;

#[cfg(not(feature = "unlimited_native"))]
type Native = zk_ee::reference_implementations::DecreasingNative;

impl<O: IOOracle> SystemTypes for ForwardSystemTypes<O> {
    type IOTypes = EthereumIOTypesConfig;
    type Resources = BaseResources<Native>;
    type IO = FullIO<
        Self::Allocator,
        Self::Resources,
        EthereumLikeStorageAccessCostModel,
        VecStackCtor,
        VecStackCtor,
        O,
        false,
    >;
    type SystemFunctions = NoStdSystemFunctions;
    type Allocator = Global;
    type Logger = Logger;
}

impl<O: IOOracle> EthereumLikeTypes for ForwardSystemTypes<O> {}

pub type ForwardRunningSystem<T, PS, TS> = ForwardSystemTypes<ForwardRunningOracle<T, PS, TS>>;

pub type CallSimulationSystem<T, PS, TS> = ForwardSystemTypes<CallSimulationOracle<T, PS, TS>>;

pub type ForwardBootloader<T, PS, TS> = BasicBootloader<ForwardRunningSystem<T, PS, TS>>;

pub type CallSimulationBootloader<T, PS, TS> = BasicBootloader<CallSimulationSystem<T, PS, TS>>;
