use super::{
    context::{contextualized::Contextualized, ErrorContext},
    location::{ErrorLocation, Localizable},
    metadata::Metadata,
};

///
/// Internal error, should not be triggered by user input.
/// Do not construct it explicitly; instead, use the macro [`internal_error`].
///
#[cfg_attr(target_arch = "riscv32", derive(Copy))]
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct InternalError(pub &'static str, pub Metadata);

#[macro_export]
macro_rules! internal_error {
    ($msg:expr $(,)?) => {
        $crate::system::errors::internal::InternalError($msg, $crate::location!().into())
    };
}

impl Localizable for InternalError {
    fn get_location(&self) -> ErrorLocation {
        let InternalError(_, meta) = self;
        meta.location
    }
}
impl Contextualized<InternalError> for InternalError {
    fn with_context_inner<F>(self, f: F) -> InternalError
    where
        F: FnOnce() -> ErrorContext,
    {
        let Self(e, meta) = self;
        Self(e, meta.replace_context(f()))
    }
}
