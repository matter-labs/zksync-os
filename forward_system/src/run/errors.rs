use basic_bootloader::bootloader::errors::BootloaderSubsystemError;
use zk_ee::system::errors::{SubsystemError, SubsystemErrorTypes};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForwardSystemErrors;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WrappedError {
    Bootloader(BootloaderSubsystemError),
}

impl From<BootloaderSubsystemError> for WrappedError {
    fn from(v: BootloaderSubsystemError) -> Self {
        Self::Bootloader(v)
    }
}

impl SubsystemErrorTypes for ForwardSystemErrors {
    type Interface = InterfaceError;
    type Wrapped = WrappedError;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InterfaceError {}

pub type ForwardSystemError = SubsystemError<ForwardSystemErrors>;
