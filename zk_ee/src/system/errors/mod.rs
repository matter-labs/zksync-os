pub mod cascade;
pub mod interface;
pub mod internal;
pub mod location;
pub mod no_errors;
pub mod root_cause;
pub mod runtime;
pub mod subsystem;
pub mod system;

use internal::InternalError;
use location::{ErrorLocation, Localizable};
use system::SystemError;

//TODO remove in favor of subsystem errors
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
