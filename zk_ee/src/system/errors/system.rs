use super::{
    internal::InternalError,
    no_errors::NoErrors,
    runtime::RuntimeError,
    subsystem::{Subsystem, SubsystemError},
};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
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

impl<S> From<SubsystemError<S>> for SystemError
where
    S: Subsystem<Interface = NoErrors, Cascaded = NoErrors>,
{
    fn from(value: SubsystemError<S>) -> Self {
        match value {
            SubsystemError::LeafUsage(_) => unreachable!(),
            SubsystemError::LeafDefect(internal_error) => internal_error.into(),
            SubsystemError::LeafRuntime(runtime_error) => runtime_error.into(),
            SubsystemError::Cascaded(_) => unreachable!(),
        }
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
