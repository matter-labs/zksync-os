use strum_macros::IntoStaticStr;

use super::location::ErrorLocation;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeError(pub RuntimeErrorKind, pub ErrorLocation);

#[derive(Clone, Debug, PartialEq, Eq, IntoStaticStr)]
pub enum RuntimeErrorKind {
    OutOfNativeResources,
}

#[macro_export]
macro_rules! out_of_native_resources {
    () => {
        $crate::system::errors::runtime::RuntimeError(
            $crate::system::errors::runtime::RuntimeErrorKind::OutOfNativeResources,
            $crate::system::errors::location::ErrorLocation::new(file!(), line!()),
        )
    };
}
