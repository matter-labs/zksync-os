pub trait BasicBootloaderExecutionConfig: 'static + Clone + Copy + core::fmt::Debug {
    /// Native account abstraction is enabled.
    const AA_ENABLED: bool;
    /// Flag to disable EOA signature validation.
    /// It can be used to optimize forward run.
    const VALIDATE_EOA_SIGNATURE: bool;
    /// Simulation flag(used for `eth_call` and `estimate_gas`)
    const SIMULATION: bool;
}

#[derive(Clone, Copy, Debug)]
pub struct BasicBootloaderProvingExecutionConfig;

impl BasicBootloaderExecutionConfig for BasicBootloaderProvingExecutionConfig {
    const SIMULATION: bool = false;
    const VALIDATE_EOA_SIGNATURE: bool = true;
    const AA_ENABLED: bool = false;
}

#[derive(Clone, Copy, Debug)]
pub struct BasicBootloaderForwardSimulationConfig;

impl BasicBootloaderExecutionConfig for BasicBootloaderForwardSimulationConfig {
    const AA_ENABLED: bool = false;
    const VALIDATE_EOA_SIGNATURE: bool = false;
    const SIMULATION: bool = false;
}

#[derive(Clone, Copy, Debug)]
pub struct BasicBootloaderCallSimulationConfig;

impl BasicBootloaderExecutionConfig for BasicBootloaderCallSimulationConfig {
    const AA_ENABLED: bool = false;
    // doesn't really matter, as `SIMULATION` disables signature validation anyway
    const VALIDATE_EOA_SIGNATURE: bool = true;
    const SIMULATION: bool = true;
}
