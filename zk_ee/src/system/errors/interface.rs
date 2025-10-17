use super::{
    context::{contextualized::Contextualized, ErrorContext},
    location::{ErrorLocation, Localizable},
    metadata::Metadata,
};

pub trait InterfaceErrorKind: Clone + core::fmt::Debug + Eq + Sized + Into<&'static str> {
    fn get_name(&self) -> &'static str;
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct InterfaceError<T: InterfaceErrorKind>(pub T, pub Metadata);

#[macro_export]
macro_rules! interface_error {
    ($instance:expr) => {
        $crate::system::errors::subsystem::SubsystemError::LeafUsage(
            $crate::system::errors::interface::InterfaceError(
                $instance,
                $crate::location!().into(),
            ),
        )
    };
}

impl<T: InterfaceErrorKind> Localizable for InterfaceError<T> {
    fn get_location(&self) -> ErrorLocation {
        let InterfaceError(_, meta) = self;
        meta.location
    }
}

impl<T: InterfaceErrorKind> Contextualized<InterfaceError<T>> for InterfaceError<T> {
    fn with_context_inner<F>(self, f: F) -> InterfaceError<T>
    where
        F: FnOnce() -> ErrorContext,
    {
        let Self(e, meta) = self;
        Self(e, meta.replace_context(f()))
    }
}
