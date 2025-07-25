use super::{
    cascade::CascadedError,
    internal::InternalError,
    location::{ErrorLocation, Localizable},
    runtime::RuntimeError,
    subsystem::{Subsystem, SubsystemError},
};

#[derive(Clone, Copy)]
pub struct ErrorInfo<'a> {
    pub subsystem: &'static str,
    pub location: ErrorLocation,
    pub error: &'a dyn core::fmt::Display,
}

#[derive(Clone, Copy, Debug)]
pub enum RootCause<'a> {
    Runtime(&'a RuntimeError),
    Internal(&'a InternalError),
    Usage(ErrorInfo<'a>),
}

pub trait GetRootCause {
    fn root_cause(&self) -> RootCause;
}

impl<S> GetRootCause for SubsystemError<S>
where
    S: Subsystem,
{
    fn root_cause(&self) -> RootCause {
        match self {
            SubsystemError::Cascaded(CascadedError(inner, _)) => inner.root_cause(),
            SubsystemError::LeafRuntime(e) => RootCause::Runtime(&e),
            SubsystemError::LeafDefect(e) => RootCause::Internal(&e),
            SubsystemError::LeafUsage(e) => RootCause::Usage(ErrorInfo {
                subsystem: S::SUBSYSTEM_NAME,
                location: e.get_location(),
                error: e,
            }),
        }
    }
}

impl<'a> core::fmt::Debug for ErrorInfo<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ErrorInfo")
            .field("subsystem", &self.subsystem)
            .field("location", &self.location)
            .field("error", &format_args!("{}", self.error))
            .finish()
    }
}
