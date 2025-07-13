use super::{location::Localizable, ErrorLocation, UpdateQueryError};

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

//TODO migrate away
impl From<InternalError> for UpdateQueryError {
    fn from(e: InternalError) -> Self {
        UpdateQueryError::System(e.into())
    }
}
