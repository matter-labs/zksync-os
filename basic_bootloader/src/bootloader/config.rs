#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExecutionVersion {
    V1,
    V2,
    V3,
}

impl ExecutionVersion {
    pub fn latest() -> Self {
        ExecutionVersion::V3
    }
}

pub trait BasicBootloaderExecutionConfig: 'static + Clone + Copy + core::fmt::Debug {
    /// Native account abstraction is enabled.
    const AA_ENABLED: bool;
    /// Flag to disable EOA signature validation.
    /// It can be used to optimize forward run.
    const VALIDATE_EOA_SIGNATURE: bool;
    /// Simulation flag(used for `eth_call` and `estimate_gas`)
    const SIMULATION: bool;

    fn execution_version(&self) -> ExecutionVersion;

    fn allow_eip_712(&self) -> bool {
        self.execution_version() >= ExecutionVersion::V2
    }

    fn handle_zero_native_per_gas(&self) -> bool {
        self.execution_version() >= ExecutionVersion::V3
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BasicBootloaderProvingExecutionConfig;

impl BasicBootloaderExecutionConfig for BasicBootloaderProvingExecutionConfig {
    const SIMULATION: bool = false;
    const VALIDATE_EOA_SIGNATURE: bool = true;
    const AA_ENABLED: bool = false;

    fn execution_version(&self) -> ExecutionVersion {
        ExecutionVersion::latest()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BasicBootloaderForwardSimulationConfig(pub ExecutionVersion);

impl BasicBootloaderExecutionConfig for BasicBootloaderForwardSimulationConfig {
    const AA_ENABLED: bool = false;
    const VALIDATE_EOA_SIGNATURE: bool = false;
    const SIMULATION: bool = false;

    fn execution_version(&self) -> ExecutionVersion {
        self.0
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BasicBootloaderCallSimulationConfig(pub ExecutionVersion);

impl BasicBootloaderExecutionConfig for BasicBootloaderCallSimulationConfig {
    const AA_ENABLED: bool = false;
    // doesn't really matter, as `SIMULATION` disables signature validation anyway
    const VALIDATE_EOA_SIGNATURE: bool = true;
    const SIMULATION: bool = true;

    fn execution_version(&self) -> ExecutionVersion {
        self.0
    }
}
