use zk_ee::system::{errors::{NoErrors, SubsystemErrorTypes}, CallModifier};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvmSubsystemErrors;

impl SubsystemErrorTypes for EvmSubsystemErrors {
    type Interface = InterfaceError;
    type Wrapped = NoErrors;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InterfaceError {
    NoDeploymentScheme,
    UnknownDeploymentData,
    BytecodeNoPadding,
    UnexpectedModifier(CallModifier),
}

pub type EvmSubsystemError = zk_ee::system::errors::SubsystemError<EvmSubsystemErrors>;
