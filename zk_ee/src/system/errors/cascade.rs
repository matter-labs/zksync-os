use super::{
    context::{contextualized::Contextualized, ErrorContext},
    location::{ErrorLocation, Localizable},
    metadata::Metadata,
    root_cause::GetRootCause,
};

pub trait ICascadedInner:
    core::fmt::Debug + Clone + Eq + Sized + GetRootCause + core::fmt::Display
{
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CascadedError<T: ICascadedInner>(pub T, pub Metadata);

impl<T: ICascadedInner> Localizable for CascadedError<T> {
    fn get_location(&self) -> ErrorLocation {
        let CascadedError(_, meta) = self;
        meta.location
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

impl<T> Contextualized<CascadedError<T>> for CascadedError<T>
where
    T: ICascadedInner,
{
    fn with_context_inner<F>(self, f: F) -> CascadedError<T>
    where
        F: FnOnce() -> ErrorContext,
    {
        let Self(e, metadata) = self;
        Self(e, metadata.replace_context(f()))
    }
}
