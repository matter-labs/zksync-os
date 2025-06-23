use basic_bootloader::bootloader::errors::BootloaderSubsystemError;
use basic_bootloader::bootloader::supported_ees::EESubsystemError;
use zk_ee::system::errors::Fault;
use zk_ee::system::errors::SubsystemErrors;

use zk_ee::system::errors::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ErrorsDescription;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PublicError {
    EEError(EESubsystemError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WrappedError {
    BootloaderError(Error<basic_bootloader::bootloader::errors::ErrorsDescription>),
}

impl From<Error<basic_bootloader::bootloader::errors::ErrorsDescription>> for WrappedError {
    fn from(v: Error<basic_bootloader::bootloader::errors::ErrorsDescription>) -> Self {
        Self::BootloaderError(v)
    }
}

impl SubsystemErrors for ErrorsDescription {
    type Public = PublicError;
    type Interface = InterfaceError;
    type Wrapped = WrappedError;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InterfaceError {
}

pub type ForwardSubsystemError = Fault<ErrorsDescription>;

pub fn propagate(error: BootloaderSubsystemError) -> Fault<ErrorsDescription> {
    Fault::Cascaded(WrappedError::BootloaderError(error.into()))
}
