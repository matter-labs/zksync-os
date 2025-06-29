pub mod cascade;
pub mod interface;
pub mod internal;
pub mod location;
pub mod no_errors;
pub mod root_cause;
pub mod runtime;
pub mod subsystem;
pub mod system;

use internal::InternalError;
use location::{ErrorLocation, Localizable};
use runtime::RuntimeError;
use system::SystemError;

// TODO remove in favor of subsystem errors
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum FatalError {
    /// EE execution exhausted the resources passed.
    OutOfNativeResources(ErrorLocation),
    Internal(InternalError),
}

impl From<FatalError> for SystemError {
    fn from(e: FatalError) -> Self {
        match e {
            FatalError::Internal(e) => Self::LeafDefect(e),
            FatalError::OutOfNativeResources(loc) => {
                Self::LeafRuntime(RuntimeError::OutOfNativeResources(loc))
            }
        }
    }
}

impl From<InternalError> for FatalError {
    fn from(e: InternalError) -> Self {
        Self::Internal(e)
    }
}

//TODO remove in favor of subsystem errors
#[derive(Debug)]
pub enum UpdateQueryError {
    /// Attempted an update that over/underflows the numerical bound.
    /// Can be due to:
    /// - An account's balance update that would result in a negative value.
    /// - An account's nonce update that would overflow u64.
    NumericBoundsError,
    System(SystemError),
}

impl From<SystemError> for UpdateQueryError {
    fn from(e: SystemError) -> Self {
        UpdateQueryError::System(e)
    }
}

//TODO  remove in favor of subsystem errors
#[derive(Debug, PartialEq, Eq)]
pub enum SystemFunctionError {
    /// Invalid input passed to system function.
    ///
    /// For example, invalid length for pairing check, or values that don't represent a point for ecadd.
    ///
    /// Please note, that system function decides when to return this error.
    /// For example ecrecover(according to EVM specs) returns empty output instead of error in all the cases.
    InvalidInput,
    System(SystemError),
}

impl From<SystemError> for SystemFunctionError {
    fn from(e: SystemError) -> Self {
        SystemFunctionError::System(e)
    }
}

#[macro_export]
macro_rules! out_of_native_resources_fatal_error {
    () => {
        $crate::system::errors::FatalError::OutOfNativeResources($crate::location!())
    };
}

#[macro_export]
macro_rules! out_of_native_resources_system_error {
    () => {
        $crate::system::errors::system::SystemError::LeafRuntime(
            $crate::system::errors::runtime::RuntimeError::OutOfNativeResources(
                $crate::location!(),
            ),
        )
    };
}
