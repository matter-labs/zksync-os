use super::{
    cascade::ICascadedInner,
    interface::InterfaceErrorKind,
    location::{ErrorLocation, Localizable},
    root_cause::{GetRootCause, RootCause},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoErrors {}

impl Localizable for NoErrors {
    fn get_location(&self) -> ErrorLocation {
        unreachable!()
    }
}
impl InterfaceErrorKind for NoErrors {
    fn get_name(&self) -> &'static str {
        unreachable!()
    }
}
impl GetRootCause for NoErrors {
    fn root_cause(&self) -> RootCause {
        unreachable!()
    }
}
impl ICascadedInner for NoErrors {}

impl From<NoErrors> for &'static str {
    fn from(val: NoErrors) -> Self {
        match val {}
    }
}
