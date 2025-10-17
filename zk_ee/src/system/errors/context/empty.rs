#![cfg(target_arch = "riscv32")]

use super::{
    element::{NamedContextElement, ValueVisibility},
    IErrorContext,
};

use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;

#[derive(Copy, Debug, Default, Clone, PartialEq, Eq)]
pub struct ErrorContext {}

impl core::fmt::Display for ErrorContext {
    fn fmt(&self, _: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Ok(())
    }
}

impl IErrorContext for ErrorContext {
    #[inline(always)]
    fn get(&self, _: &str) -> Option<&String> {
        None
    }

    #[inline(always)]
    fn push(self, _: &'static str, _: impl ToString, _: ValueVisibility) -> Self {
        self
    }

    #[inline(always)]
    fn push_lazy<F>(self, _name: &'static str, _f: F, _visibility: ValueVisibility) -> Self
    where
        F: FnOnce() -> String,
    {
        self
    }

    #[inline(always)]
    fn to_vec(&self) -> Option<Vec<NamedContextElement>> {
        None
    }

    #[inline(always)]
    fn into_vec(self) -> Option<Vec<NamedContextElement>> {
        None
    }
}

/// On RISC-V, this macro ignores all the context elements, guaranteeing that
/// the context will not be constructed and all the expressions used to
/// construct it will be ignored.
#[macro_export]
macro_rules! error_ctx {
    { $($tt:tt)* } => {{
        $crate::system::errors::context::empty::ErrorContext::default()
    }};
}
