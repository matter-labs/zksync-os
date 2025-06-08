pub trait BasicBootloaderExecutionConfig: 'static + Clone + Copy + core::fmt::Debug {
    /// Native account abstraction is enabled.
    const AA_ENABLED: bool;
    /// Skip validation
    const ONLY_SIMULATE: bool;
    /// Flag to disable EOA signature validation.
    /// It can be used to optimize forward run.
    const VALIDATE_EOA_SIGNATURE: bool;
}

#[derive(Clone, Copy, Debug)]
pub struct BasicBootloaderProvingExecutionConfig;

impl BasicBootloaderExecutionConfig for BasicBootloaderProvingExecutionConfig {
    const ONLY_SIMULATE: bool = false;
    const AA_ENABLED: bool = true;
    const VALIDATE_EOA_SIGNATURE: bool = true;
}

#[derive(Clone, Copy, Debug)]
pub struct BasicBootloaderForwardSimulationConfig;

impl BasicBootloaderExecutionConfig for BasicBootloaderForwardSimulationConfig {
    const ONLY_SIMULATE: bool = false;
    const AA_ENABLED: bool = true;
    const VALIDATE_EOA_SIGNATURE: bool = false;
}

#[derive(Clone, Copy, Debug)]
pub struct BasicBootloaderCallSimulationConfig;

impl BasicBootloaderExecutionConfig for BasicBootloaderCallSimulationConfig {
    const ONLY_SIMULATE: bool = true;
    const AA_ENABLED: bool = true;
    const VALIDATE_EOA_SIGNATURE: bool = false;
}
