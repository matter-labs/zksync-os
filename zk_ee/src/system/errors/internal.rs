use super::{
    context::{contextualized::Contextualized, ErrorContext},
    location::{ErrorLocation, Localizable},
    metadata::Metadata,
};

///
/// Internal error, should not be triggered by user input.
/// Do not construct it explicitly; instead, use the macro [`internal_error`].
///
/// The message is only kept with the `error_origins` feature (like the location in
/// [Metadata]): without it the error is a zero-sized marker on the proving target,
/// which keeps every `Result` carrying it small.
///
#[cfg_attr(target_arch = "riscv32", derive(Copy))]
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct InternalError {
    #[cfg(feature = "error_origins")]
    pub message: &'static str,
    pub metadata: Metadata,
}

impl InternalError {
    #[cfg_attr(not(feature = "error_origins"), allow(unused_variables))]
    #[inline(always)]
    pub fn new(message: &'static str, location: ErrorLocation) -> Self {
        Self {
            #[cfg(feature = "error_origins")]
            message,
            metadata: location.into(),
        }
    }

    /// The message, when it is kept (`error_origins`).
    pub fn message(&self) -> Option<&'static str> {
        #[cfg(feature = "error_origins")]
        {
            Some(self.message)
        }
        #[cfg(not(feature = "error_origins"))]
        {
            None
        }
    }
}

#[macro_export]
macro_rules! internal_error {
    ($msg:expr $(,)?) => {
        $crate::system::errors::internal::InternalError::new($msg, $crate::location!())
    };
}

impl Localizable for InternalError {
    fn get_location(&self) -> ErrorLocation {
        self.metadata.location
    }
}
impl Contextualized<InternalError> for InternalError {
    fn with_context_inner<F>(self, f: F) -> InternalError
    where
        F: FnOnce() -> ErrorContext,
    {
        Self {
            #[cfg(feature = "error_origins")]
            message: self.message,
            metadata: self.metadata.replace_context(f()),
        }
    }
}
