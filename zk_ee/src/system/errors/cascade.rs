use super::{location::ErrorLocation, root_cause::GetRootCause, Localizable};

pub trait ICascadedInner: core::fmt::Debug + Clone + Eq + Sized + GetRootCause {}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CascadedError<T: ICascadedInner>(pub T, pub ErrorLocation);

impl<T: ICascadedInner> Localizable for CascadedError<T> {
    fn get_location(&self) -> ErrorLocation {
        let CascadedError(_, location) = self;
        *location
    }
}

#[macro_export]
macro_rules! wrap_error {
    ($e:expr) => {
        $e.wrap($crate::location!())
    };
    () => {
        |e| e.wrap($crate::location!())
    };
}
