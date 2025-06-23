use basic_bootloader::bootloader::errors::BootloaderSubsystemError;
use basic_bootloader::bootloader::supported_ees::EESubsystemError;
use zk_ee::system::errors::SubsystemError;
use zk_ee::system::errors::SubsystemErrorTypes;


#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ErrorsDescription;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PublicError {
    EEError(EESubsystemError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WrappedError {
    BootloaderError(SubsystemError<basic_bootloader::bootloader::errors::ErrorsDescription>),
}

impl From<SubsystemError<basic_bootloader::bootloader::errors::ErrorsDescription>> for WrappedError {
    fn from(v: SubsystemError<basic_bootloader::bootloader::errors::ErrorsDescription>) -> Self {
        Self::BootloaderError(v)
    }
}

impl SubsystemErrorTypes for ErrorsDescription {
    type Interface = InterfaceError;
    type Wrapped = WrappedError;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InterfaceError {
}

pub type ForwardSubsystemError = SubsystemError<ErrorsDescription>;
