use strum_macros::IntoStaticStr;

use super::{
    context::{contextualized::Contextualized, ErrorContext},
    location::{ErrorLocation, Localizable},
    metadata::Metadata,
};

#[cfg_attr(target_arch = "riscv32", derive(Copy))]
#[derive(Clone, Debug, PartialEq, Eq, IntoStaticStr)]
pub enum RuntimeError {
    OutOfNativeResources(Metadata),
    OutOfErgs(Metadata),
}

#[macro_export]
macro_rules! out_of_native_resources {
    () => {
        $crate::system::errors::runtime::RuntimeError::OutOfNativeResources(
            $crate::location!().into(),
        )
    };
}

impl Localizable for RuntimeError {
    fn get_location(&self) -> ErrorLocation {
        match self {
            RuntimeError::OutOfNativeResources(metadata) | RuntimeError::OutOfErgs(metadata) => {
                metadata.location
            }
        }
    }
}

impl Contextualized<RuntimeError> for RuntimeError {
    fn with_context_inner<F>(self, f: F) -> RuntimeError
    where
        F: FnOnce() -> ErrorContext,
    {
        match self {
            RuntimeError::OutOfNativeResources(metadata) => {
                RuntimeError::OutOfNativeResources(metadata.replace_context(f()))
            }
            RuntimeError::OutOfErgs(metadata) => {
                RuntimeError::OutOfErgs(metadata.replace_context(f()))
            }
        }
    }
}
