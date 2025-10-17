use core::fmt::Display;

use alloc::string::String;

/// A context element with a name and value
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedContextElement {
    pub name: &'static str,
    pub value: String,
}

/// Describes which builds will include the context value.
/// RISC-V builds for proving purposes always omit context to be efficient.
pub enum ValueVisibility {
    /// Value is available only outside proving context and with
    /// `detailed_errors` feature enabled.
    ///
    DetailedOnly,
    /// Any target outside proving context will include the value in the
    /// context.
    AnyForwardRun,
}

impl Display for NamedContextElement {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let Self { name, value } = self;
        write!(f, "{name} => {value}")
    }
}
