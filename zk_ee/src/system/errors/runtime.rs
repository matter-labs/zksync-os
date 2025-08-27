use strum_macros::IntoStaticStr;

use super::location::{ErrorLocation, Localizable};
use super::metadata::Metadata;

/// Errors that lead to a transaction-level revert.
#[cfg_attr(target_arch = "riscv32", derive(Copy))]
#[derive(Clone, Debug, PartialEq, Eq, IntoStaticStr)]
pub enum FatalRuntimeError {
    OutOfNativeResources(Metadata),
    OutOfReturnMemory(Metadata),
}

#[cfg_attr(target_arch = "riscv32", derive(Copy))]
#[derive(Clone, Debug, PartialEq, Eq, IntoStaticStr)]
pub enum RuntimeError {
    FatalRuntimeError(FatalRuntimeError),
    OutOfErgs(Metadata),
}

#[macro_export]
macro_rules! out_of_return_memory {
    () => {
        $crate::system::errors::runtime::RuntimeError::FatalRuntimeError(
            $crate::system::errors::runtime::FatalRuntimeError::OutOfReturnMemory(
                $crate::location!().into(),
            ),
        )
    };
}

#[macro_export]
macro_rules! out_of_native_resources {
    () => {
        $crate::system::errors::runtime::RuntimeError::FatalRuntimeError(
            $crate::system::errors::runtime::FatalRuntimeError::OutOfNativeResources(
                $crate::location!().into(),
            ),
        )
    };
}

impl Localizable for RuntimeError {
    fn get_location(&self) -> ErrorLocation {
        match self {
            RuntimeError::FatalRuntimeError(FatalRuntimeError::OutOfReturnMemory(metadata))
            | RuntimeError::FatalRuntimeError(FatalRuntimeError::OutOfNativeResources(metadata))
            | RuntimeError::OutOfErgs(metadata) => metadata.location,
        }
    }
}
