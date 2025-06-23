use zk_ee::system::errors::SubsystemErrorTypes;


#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvmSubsystemErrors;

impl SubsystemErrorTypes for EvmSubsystemErrors {
    type Interface = InterfaceError;
    type Wrapped = ();
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InterfaceError {
    NoDeploymentScheme,
    UnknownDeploymentData,
}

pub type EvmSubsystemError = zk_ee::system::errors::SubsystemError<EvmSubsystemErrors>;
