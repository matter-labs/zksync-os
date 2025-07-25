use strum_macros::IntoStaticStr;

use super::location::ErrorLocation;

#[derive(Copy, Clone, Debug, PartialEq, Eq, IntoStaticStr)]
pub enum RuntimeError {
    OutOfNativeResources(ErrorLocation),
    OutOfErgs(ErrorLocation),
}

#[macro_export]
macro_rules! out_of_native_resources {
    () => {
        $crate::system::errors::runtime::RuntimeError::OutOfNativeResources($crate::location!())
    };
}
