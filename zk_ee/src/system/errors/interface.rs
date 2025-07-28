use super::{
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
        $crate::system::errors::interface::InterfaceError($instance, $crate::location!().into())
            .into()
    };
}

#[derive(Debug)]
pub struct AsInterfaceError<E>(pub E);

impl<T: InterfaceErrorKind> Localizable for InterfaceError<T> {
    fn get_location(&self) -> ErrorLocation {
        let InterfaceError(_, meta) = self;
        meta.location
    }
}
