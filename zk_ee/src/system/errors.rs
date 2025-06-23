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

// ---- TODO find a better place for these definitions

///
/// Errors common for all subsystems.
///
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeError {
    OutOfNativeResources,
}
/// # Types of errors occurring in a subsystem
///
/// ## Semantics
/// - InterfaceError: Misuse of this subsystem; violation of the interface's
///   contract. Some other subsystem did not respect the protocol of interaction
///   with this subsystem. Originates on the subsystem boundary.
/// - InvariantViolation: There is a bug in this subsystem, independent of its usage.
/// - RuntimeError: Resources exhaustion, and errors common to all subsystems
/// - Wrapped: Inherits semantics from wrapped error
///
/// ## Stakeholders
/// - InterfaceError: developers of this subsystem and other subsystems (which triggered it).
/// - InvariantViolation: developers of this subsystem
/// - RuntimeError: developers of this subsystem and other subsystems, server and humans (indirectly)
/// - Wrapped: Inherits semantics from wrapped error
///
/// ## Error handling contract:
/// - InterfaceError: Transaction rejected, system restart required
/// - InvariantViolation: Transaction rejected, system restart required
/// - RuntimeError: Transaction rejected, system restart required
/// - PropagatedError: Inherits semantics from wrapped error
pub trait SubsystemErrorTypes {
    type Interface : Clone + core::fmt::Debug + Eq + Sized;

    /// Errors this subsystem can wrap from its children
    /// Implement a enum with a variant for each child subsystem
    type Wrapped: Clone + core::fmt::Debug + Eq + Sized;
}

#[derive(Debug)]
pub struct AsPublic<E>(pub E);

#[derive(Debug)]
pub struct AsInterface<E>(pub E);

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
    pub fn wrap<T:SubsystemErrorTypes<Wrapped : From<SubsystemError<F>>>>(self) -> SubsystemError<T>
    {
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

impl<S: SubsystemErrorTypes> From<AsInterface<S::Interface>> for SubsystemError<S> {
    fn from(v: AsInterface<S::Interface>) -> Self {
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
    ($msg:expr) => {
        // The concatenation happens at compile time.
        // The result is a single &'static str.
        SystemWideError::from(SubsystemError::InvariantViolation(
            concat!(file!(), ":", line!(), ": ", $msg)
        ))
    };
}

// ----
