use super::{
    location::{ErrorLocation, Localizable},
    root_cause::GetRootCause,
};

pub trait ICascadedInner:
    core::fmt::Debug + Clone + Eq + Sized + GetRootCause + core::fmt::Display
{
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CascadedError<T: ICascadedInner>(pub T, pub ErrorLocation);

impl<T: ICascadedInner> Localizable for CascadedError<T> {
    fn get_location(&self) -> ErrorLocation {
        let CascadedError(_, meta) = self;
        *meta
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
