use strum_macros::IntoStaticStr;

use super::location::{ErrorLocation, Localizable};
use super::metadata::Metadata;

#[cfg_attr(target_arch = "riscv32", derive(Copy))]
#[derive(Clone, Debug, PartialEq, Eq, IntoStaticStr)]
pub enum RuntimeError {
    OutOfNativeResources(Metadata),
    OutOfErgs(Metadata),
}

#[macro_export]
macro_rules! out_of_native_resources {
    () => {
        $crate::system::errors::runtime::RuntimeError::OutOfNativeResources(
            $crate::location!().into(),
        )
    };
}
impl Localizable for RuntimeError {
    fn get_location(&self) -> ErrorLocation {
        match self {
            RuntimeError::OutOfNativeResources(metadata) | RuntimeError::OutOfErgs(metadata) => {
                metadata.location
            }
        }
    }
}
