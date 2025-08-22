use core::fmt::{self, Display, Formatter};

use crate::system::errors::{runtime::RuntimeError, subsystem::SubsystemError};

use super::{
    cascade::{CascadedError, ICascadedInner},
    interface::{InterfaceError, InterfaceErrorKind},
    internal::InternalError,
    root_cause::{ErrorInfo, RootCause},
    subsystem::Subsystem,
    system::SystemError,
};

impl<I> Display for CascadedError<I>
where
    I: ICascadedInner,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let Self(err, meta) = self;
        write!(f, "cascaded {meta}\n{err}")
    }
}

impl Display for InternalError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let Self(msg, meta) = self;
        write!(f, "{msg}\n{meta}")
    }
}
impl Display for RuntimeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::FatalRuntimeError(
                super::runtime::FatalRuntimeError::OutOfNativeResources(metadata),
            ) => {
                write!(f, "Out of native resources\n{metadata}")
            }
            RuntimeError::FatalRuntimeError(
                super::runtime::FatalRuntimeError::OutOfReturnMemory(metadata),
            ) => {
                write!(f, "Out of return memory\n{metadata}")
            }
            RuntimeError::OutOfErgs(metadata) => write!(f, "Out of ergs\n{metadata}"),
        }
    }
}

impl<I> Display for InterfaceError<I>
where
    I: InterfaceErrorKind,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let Self(err, meta) = self;
        let name = err.get_name();
        write!(f, "{name}\n{meta}")
    }
}
impl<S> Display for SubsystemError<S>
where
    S: Subsystem,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            SubsystemError::LeafUsage(e) => write!(f, "{e}"),
            SubsystemError::LeafDefect(e) => write!(f, "{e}"),
            SubsystemError::LeafRuntime(e) => write!(f, "{e}"),
            SubsystemError::Cascaded(e) => write!(f, "{e}"),
        }
    }
}

impl Display for SystemError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            SystemError::LeafDefect(internal_error) => write!(f, "{internal_error}"),
            SystemError::LeafRuntime(runtime_error) => write!(f, "{runtime_error}"),
        }
    }
}

impl Display for ErrorInfo<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let Self {
            subsystem,
            error,
            location,
        } = self;
        write!(f, "{location}::{subsystem}::{error}")
    }
}

impl Display for RootCause<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            RootCause::Runtime(runtime_error) => write!(f, "{runtime_error}"),
            RootCause::Internal(internal_error) => write!(f, "{internal_error}"),
            RootCause::Usage(error_info) => write!(f, "{error_info}"),
        }
    }
}
