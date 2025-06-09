///
/// Possible errors raised by the system.
///
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SystemError {
    /// System execution exhausted the native resources passed.
    OutOfNativeResources,
    /// Execution exhausted the EE resource.
    OutOfErgs,
    /// Internal error.
    /// Note that currently it means internal error in terms of whole zksync_os program execution.
    /// Not the component/function internal error.
    ///
    /// For example if you'll try to finish unstarted frame on `System` - internal error will be returned.
    /// But it doesn't mean that it's internal `System` error, the failure happened on caller(EE/bootlaoder side).
    Internal(InternalError),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum FatalError {
    /// EE execution exhausted the resources passed.
    OutOfNativeResources,
    Internal(InternalError),
}

impl From<FatalError> for SystemError {
    fn from(e: FatalError) -> Self {
        match e {
            FatalError::Internal(e) => Self::Internal(e),
            FatalError::OutOfNativeResources => Self::OutOfNativeResources,
        }
    }
}

impl From<InternalError> for FatalError {
    fn from(e: InternalError) -> Self {
        Self::Internal(e)
    }
}

impl SystemError {
    pub fn into_fatal(self) -> FatalError {
        match self {
            SystemError::Internal(e) => FatalError::Internal(e),
            SystemError::OutOfNativeResources => FatalError::OutOfNativeResources,
            SystemError::OutOfErgs => unreachable!(),
        }
    }
}

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

///
/// Internal error, should not be triggered by user input.
///
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct InternalError(pub &'static str);

impl From<InternalError> for SystemError {
    fn from(e: InternalError) -> Self {
        SystemError::Internal(e)
    }
}

impl From<InternalError> for UpdateQueryError {
    fn from(e: InternalError) -> Self {
        SystemError::Internal(e).into()
    }
}

impl From<InternalError> for SystemFunctionError {
    fn from(e: InternalError) -> Self {
        SystemError::Internal(e).into()
    }
}
