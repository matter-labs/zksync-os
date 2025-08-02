pub mod contextualized;
pub mod element;
pub mod empty;
pub mod nonempty;
#[cfg(test)]
pub mod tests;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;
use core::option::Option;
use element::ValueVisibility;

use element::NamedContextElement;

/// Context is a map from names (`&'static str`) to `String` values.
/// When we throw an error, we may chose to add additional information for an
/// easier debugging.
/// On different targets, this information is then preserved or ignored:
///
/// - On RISC-V proving system, the context is ignored.
/// - In production, only the basic context will be emitted.
/// - For debug purposes (e.g. in `anvil-zksync`), the full context should be emitted.
///
/// Contexts are constructed using [`error_ctx`] macro.
pub trait IErrorContext {
    fn get(&self, name: &str) -> Option<&String>;
    fn push(self, name: &'static str, value: impl ToString, visibility: ValueVisibility) -> Self;

    /// Push a context entry with lazy evaluation. The closure is only called
    /// when the entry should be included based on the visibility and feature flags.
    fn push_lazy<F>(self, name: &'static str, f: F, visibility: ValueVisibility) -> Self
    where
        F: FnOnce() -> String;

    fn to_vec(&self) -> Option<Vec<NamedContextElement>>;
    fn into_vec(self) -> Option<Vec<NamedContextElement>>;
}

#[cfg(not(target_arch = "riscv32"))]
pub type ErrorContext = nonempty::ErrorContext;
#[cfg(target_arch = "riscv32")]
pub type ErrorContext = empty::ErrorContext;
