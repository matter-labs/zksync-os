use zk_ee::system::errors::SubsystemErrors;


#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvmSubsystemErrors;

impl SubsystemErrors for EvmSubsystemErrors {
    type Public = zksync_os_error::exec_env::evm::EVMError;
    type Interface = InterfaceError;
    type Wrapped = ();
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InterfaceError {
    NoDeploymentScheme,
    UnknownDeploymentData,
}

pub type EvmSubsystemError = zk_ee::system::errors::Fault<EvmSubsystemErrors>;
pub type EvmSystemWideError = zk_ee::system::errors::Error<EvmSubsystemErrors>;
