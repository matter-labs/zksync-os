// TODO remove in favor of subsystem errors
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

// TODO remove in favor of subsystem errors
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

// TODO: try replacing all instantiations with a macro
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

///
/// Errors common for all subsystems.
///
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeError {
    OutOfNativeResources,
}

/// # Types of errors occurring in a subsystem
///
pub trait SubsystemErrorTypes {
    type Interface: Clone + core::fmt::Debug + Eq + Sized;

    /// Errors this subsystem can wrap from its children
    /// Implement a enum with a variant for each child subsystem
    type Wrapped: Clone + core::fmt::Debug + Eq + Sized;
}

/// Use this enum to signal that the type does not have wrapped or interface errors
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoErrors {}

#[derive(Debug)]
pub struct AsInterfaceError<E>(pub E);

///
/// Error on a subsystem boundary
///
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubsystemError<S: SubsystemErrorTypes> {
    /// Meaning of an interface error comes from the subsystem misuse:
    /// - the contract of the subsystem's interface is violated, or
    /// - the protocol or interaction with the subsystem is violated
    /// Fixing it usually requires changing the context of the call.
    Usage(S::Interface),

    /// Internal error, a bug that does not depend on how the subsystem was
    /// used.
    /// Fixing it will probably require changing the code inside the subsystem.
    /// Triggers system restart and the transaction is ignored.
    Defect(InternalError),

    /// Common errors for all subsystems, like `OutOfNativeResources`
    Runtime(RuntimeError),

    /// Error propagated from another subsystem
    Cascaded(S::Wrapped),
}

impl<F: SubsystemErrorTypes> SubsystemError<F> {
    pub fn wrap<T: SubsystemErrorTypes<Wrapped: From<SubsystemError<F>>>>(
        self,
    ) -> SubsystemError<T> {
        SubsystemError::Cascaded(self.into())
    }
}

impl<S: SubsystemErrorTypes> From<RuntimeError> for SubsystemError<S> {
    fn from(v: RuntimeError) -> Self {
        Self::Runtime(v)
    }
}

impl<S: SubsystemErrorTypes> From<InternalError> for SubsystemError<S> {
    fn from(v: InternalError) -> Self {
        Self::Defect(v)
    }
}

impl<S: SubsystemErrorTypes> From<AsInterfaceError<S::Interface>> for SubsystemError<S> {
    fn from(v: AsInterfaceError<S::Interface>) -> Self {
        Self::Usage(v.0)
    }
}

impl<S: SubsystemErrorTypes> From<FatalError> for SubsystemError<S> {
    fn from(value: FatalError) -> Self {
        match value {
            FatalError::OutOfNativeResources => RuntimeError::OutOfNativeResources.into(),
            FatalError::Internal(internal_error) => internal_error.into(),
        }
    }
}

macro_rules! invariant_violation {
macro_rules! internal_error {
    ($msg:expr) => {
        // The concatenation happens at compile time.
        // The result is a single &'static str.
        SubsystemError::Defect(InternalError(concat!(file!(), ":", line!(), ": ", $msg)))
    };
}
