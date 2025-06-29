use super::{
    internal::InternalError,
    runtime::RuntimeError,
    subsystem::{Subsystem, SubsystemError},
    FatalError,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SystemError {
    LeafDefect(InternalError),
    LeafRuntime(RuntimeError),
}

impl<T: Subsystem> From<SystemError> for SubsystemError<T> {
    fn from(value: SystemError) -> Self {
        match value {
            SystemError::LeafRuntime(runtime_error) => runtime_error.into(),
            SystemError::LeafDefect(internal_error) => internal_error.into(),
        }
    }
}

impl From<InternalError> for SystemError {
    fn from(e: InternalError) -> Self {
        SystemError::LeafDefect(e)
    }
}

impl From<RuntimeError> for SystemError {
    fn from(v: RuntimeError) -> Self {
        Self::LeafRuntime(v)
    }
}

#[macro_export]
macro_rules! out_of_ergs_error {
    () => {
        $crate::system::errors::system::SystemError::LeafRuntime(
            $crate::system::errors::runtime::RuntimeError::OutOfErgs(
                $crate::system::errors::location::ErrorLocation::new(file!(), line!()),
            ),
        )
    };
}

impl SystemError {
    //TODO migrate away
    pub fn into_fatal(self) -> FatalError {
        match self {
            SystemError::LeafDefect(e) => FatalError::Internal(e),
            SystemError::LeafRuntime(RuntimeError::OutOfNativeResources(loc)) => {
                FatalError::OutOfNativeResources(loc)
            }
            SystemError::LeafRuntime(_) => unreachable!(),
        }
    }
}
