use super::location::{ErrorLocation, Localizable};

///
/// Internal error, should not be triggered by user input.
/// Do not construct it explicitly; instead, use the macro [`internal_error`].
///
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct InternalError(pub &'static str, pub ErrorLocation);

#[macro_export]
macro_rules! internal_error {
    ($msg:expr $(,)?) => {
        $crate::system::errors::internal::InternalError($msg, $crate::location!())
    };
}

impl Localizable for InternalError {
    fn get_location(&self) -> ErrorLocation {
        let InternalError(_, location) = self;
        *location
    }
}
