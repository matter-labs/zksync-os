use zk_ee::oracle::query_ids::CHAIN_CONFIG_QUERY_ID;
use zk_ee::oracle::IOOracle;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::system::metadata::chain_config::ChainConfig;

pub trait BasicBootloaderExecutionConfig: 'static + Clone + Copy + core::fmt::Debug {
    /// Flag to disable EOA signature validation.
    /// It can be used to optimize forward run.
    const VALIDATE_EOA_SIGNATURE: bool;
    /// Simulation flag(used for `eth_call` and `estimate_gas`)
    const SIMULATION: bool;
    /// Flag to disable FRI proof verification. Disabled for sequencing.
    const VERIFY_FRI_PROOFS: bool;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BootloaderStaticConfig {
    pub chain_config: ChainConfig,
}

impl BootloaderStaticConfig {
    pub fn read_from_oracle(oracle: &mut impl IOOracle) -> Result<Self, InternalError> {
        let chain_config = oracle.query_with_empty_input(CHAIN_CONFIG_QUERY_ID)?;
        Ok(Self { chain_config })
    }
}

impl Default for BootloaderStaticConfig {
    fn default() -> Self {
        Self {
            chain_config: ChainConfig::default(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BasicBootloaderProvingExecutionConfig;

impl BasicBootloaderExecutionConfig for BasicBootloaderProvingExecutionConfig {
    const SIMULATION: bool = false;
    const VALIDATE_EOA_SIGNATURE: bool = true;
    const VERIFY_FRI_PROOFS: bool = true;
}

#[derive(Clone, Copy, Debug)]
pub struct BasicBootloaderForwardSimulationConfig;

impl BasicBootloaderExecutionConfig for BasicBootloaderForwardSimulationConfig {
    const VALIDATE_EOA_SIGNATURE: bool = false;
    const SIMULATION: bool = false;
    const VERIFY_FRI_PROOFS: bool = false;
}

#[derive(Clone, Copy, Debug)]
pub struct BasicBootloaderCallSimulationConfig;

impl BasicBootloaderExecutionConfig for BasicBootloaderCallSimulationConfig {
    // doesn't really matter, as `SIMULATION` disables signature validation anyway
    const VALIDATE_EOA_SIGNATURE: bool = true;
    const SIMULATION: bool = true;
    const VERIFY_FRI_PROOFS: bool = false;
}

#[derive(Clone, Copy, Debug)]
pub struct BasicBootloaderForwardETHLikeConfig;

impl BasicBootloaderExecutionConfig for BasicBootloaderForwardETHLikeConfig {
    const VALIDATE_EOA_SIGNATURE: bool = true;
    const SIMULATION: bool = false;
    const VERIFY_FRI_PROOFS: bool = false;
}
