#![cfg(not(target_arch = "riscv32"))]

use core::fmt::Display;

use super::{
    element::{NamedContextElement, ValueVisibility},
    IErrorContext,
};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ErrorContext {
    values: Vec<NamedContextElement>,
}

impl IErrorContext for ErrorContext {
    #[inline(always)]
    fn get(&self, name: &str) -> Option<&String> {
        self.values
            .iter()
            .find(|e| (e.name == name))
            .map(|e| &e.value)
    }

    #[inline(always)]
    fn to_vec(&self) -> Option<Vec<NamedContextElement>> {
        Some(self.values.clone())
    }

    #[inline(always)]
    fn into_vec(self) -> Option<Vec<NamedContextElement>> {
        Some(self.values)
    }

    #[inline(always)]
    fn push(
        mut self,
        name: &'static str,
        value: impl ToString,
        visibility: ValueVisibility,
    ) -> Self {
        let mut perform_push_value = || {
            let value = value.to_string();
            self.values.push(NamedContextElement { name, value })
        };
        if cfg!(not(target_arch = "riscv32")) {
            match visibility {
                ValueVisibility::AnyForwardRun => perform_push_value(),
                ValueVisibility::DetailedOnly if cfg!(feature = "detailed_errors") => {
                    perform_push_value()
                }
                _ => {}
            }
        }
        self
    }

    #[inline(always)]
    fn push_lazy<F>(mut self, name: &'static str, f: F, visibility: ValueVisibility) -> Self
    where
        F: FnOnce() -> String,
    {
        let should_include = match visibility {
            ValueVisibility::AnyForwardRun => true,
            ValueVisibility::DetailedOnly => cfg!(feature = "detailed_errors"),
        };

        if should_include {
            let value = f();
            self.values.push(NamedContextElement { name, value });
        }
        self
    }
}

/// Constructs an error context. Works for forward runs, ignored in the proving
/// context.
///
/// # `detailed` attribute
///
/// The `#[detailed]` attribute uses lazy evaluation via closures to ensure expressions
/// are only evaluated when the `detailed_errors` feature is enabled:
/// - Expensive computations are lazily evaluated and eliminated when `detailed_errors` is disabled
/// - Side effects in detailed expressions won't occur in production builds
/// - The lazy closure approach provides better compiler optimization opportunities
///
/// # debug_format
///
/// When defining context, the function `debug_format` is available to transform
/// any value implementing `Debug` into its debug representation.
/// For example:
/// ```rust,ignore
/// error_ctx! {
/// "target" => debug_format(target),
/// }
/// ```
///
/// # Examples
/// ```rust
/// extern crate alloc;
/// use zk_ee::error_ctx;
/// fn test_valid_usages_still_work() {
///    // Empty context
///    let _ctx1 = error_ctx! {};
///
///    // Simple key value pair.
///    let _ctx2 = error_ctx! {
///        "key" => "some_value"
///    };
///
///    // A shorthand for `"var" => var`
///    let var = "test_value";
///    let _ctx3 = error_ctx! {
///        var
///    };
///
///    let _ctx4 = error_ctx! {
///        #[detailed] "debug" => "info",  // Only evaluated if `detailed_errors` feature is enabled
///        "public" => "data"              // Always evaluated (but not on RISC-V)
///    };
///
///    let _ctx5 = error_ctx! {
///        #[detailed] var,                // Only evaluated if `detailed_errors` feature is enabled
///        "other" => 42
///    };
///
/// // debug_format transforms an object implementing Debug into its string
/// // representation:
///    let _ctx6 = error_ctx! {
///        "test" => "value",
///        "var_debug_repr" => debug_format(var),
///    };
///
///    let _ctx7 = error_ctx! {
///        "test" => "value",
///        var,
///        // `debug_format` is lazily called via closure when `detailed_errors` feature is enabled
///        #[detailed] "debug" => debug_format(var),
///    };
///}
/// ```
#[macro_export]
macro_rules! error_ctx {
    (@entries $ctx:ident, $(,)*) => {};

    // ` "name" => expr `, only if the feature "detailed_errors" is enabled
    // Expression is lazily evaluated via closure - only called when needed
    (@entries $ctx:ident, #[detailed] $name:literal => $value:expr $(, $($rest:tt)*)?) => {
        $ctx = $ctx.push_lazy($name, || ($value).to_string(),
                  $crate::system::errors::context::element::ValueVisibility::DetailedOnly);
        $($crate::error_ctx!(@entries $ctx, $($rest)*);)?
    };
    // ` "name" => expr `, always included
    (@entries $ctx:ident, $name:literal => $value:expr $(, $($rest:tt)*)?) => {
         $ctx = $ctx.push($name, $value,
                   $crate::system::errors::context::element::ValueVisibility::AnyForwardRun,
         );
        $($crate::error_ctx!(@entries $ctx, $($rest)*);)?
    };
    // ident, only if the feature "detailed_errors" is enabled
    // `#[detailed] var` is an alias to `#[detailed] "var" => var`
    // Variable is lazily evaluated via closure - only accessed when needed
    (@entries $ctx:ident, #[detailed] $name:ident $(, $($rest:tt)*)?) => {
        $ctx = $ctx.push_lazy(stringify!($name), || $name.to_string(),
                  $crate::system::errors::context::element::ValueVisibility::DetailedOnly);
        $($crate::error_ctx!(@entries $ctx, $($rest)*);)?
    };
    // identifier, always included
    // `var` is an alias to `"var" => var`
    (@entries $ctx:ident, $name:ident $(, $($rest:tt)*)?) => {
         $ctx = $ctx.push(stringify!($name), $name,
                   $crate::system::errors::context::element::ValueVisibility::AnyForwardRun,
         );
        $($crate::error_ctx!(@entries $ctx, $($rest)*);)?
    };

    // Error patterns - these must come after valid patterns to catch invalid usage

    // Catch malformed attribute usage - #[detailed] on non-literal, non-ident
    (@entries $ctx:ident, #[detailed] $invalid:tt $($rest:tt)*) => {
        compile_error!(concat!(
            "Invalid #[detailed] attribute usage. ",
            "#[detailed] can only be used with identifiers or string literals. ",
            "Valid examples: #[detailed] variable, #[detailed] \"key\" => value"
        ));
    };

    // Catch invalid arrow usage - identifier followed by something other than comma or end
    (@entries $ctx:ident, $name:ident => $($rest:tt)*) => {
        compile_error!(concat!(
            "Invalid syntax: identifier '", stringify!($name), "' followed by '=>'. ",
            "Use either '", stringify!($name), "' (shorthand for \"", stringify!($name), "\" => ", stringify!($name), ") ",
            "or \"", stringify!($name), "\" => value (explicit key-value pair)"
        ));
    };

    // Catch missing arrow in key-value pairs - key and value with no =>
    (@entries $ctx:ident, $key:literal $value:tt $($rest:tt)*) => {
        compile_error!(concat!(
            "Missing '=>' between key and value. ",
            "Use: ", stringify!($key), " => <value>"
        ));
    };

    // Catch double attributes
    (@entries $ctx:ident, #[detailed] #[detailed] $($rest:tt)*) => {
        compile_error!("Duplicate #[detailed] attribute. Use only one #[detailed] per entry.");
    };

    // Catch unknown attributes
    (@entries $ctx:ident, #[$attr:ident] $($rest:tt)*) => {
        compile_error!(concat!(
            "Unknown attribute: #[", stringify!($attr), "]. ",
            "Only #[detailed] is supported."
        ));
    };

    // Catch empty string keys
    (@entries $ctx:ident, "" => $value:expr $(, $($rest:tt)*)?) => {
        compile_error!("Empty string keys are not allowed. Please provide a meaningful key name.");
    };

    (@entries $ctx:ident, #[detailed] "" => $value:expr $(, $($rest:tt)*)?) => {
        compile_error!("Empty string keys are not allowed. Please provide a meaningful key name.");
    };

    // Catch any other invalid patterns that don't match above
    (@entries $ctx:ident, $invalid:tt $($rest:tt)*) => {
        compile_error!(concat!(
            "Invalid syntax in error_ctx!. ",
            "Valid patterns are: ",
            "'identifier', ",
            "\"key\" => value, ",
            "#[detailed] identifier, ",
            "#[detailed] \"key\" => value"
        ));
    };

    // Entry point; creates a new context
    { $($tt:tt)* } => {{

        #[doc=r#"
        When defining context, this function is available to transform any value
        implementing `Debug` into its debug representation.
        For example:
        ```rust,ignore
        error_ctx! {
        "target" => debug_format(target),
        }
        ```
        "#]
        #[allow(dead_code)]
        fn debug_format<T:core::fmt::Debug>(val: T) -> alloc::string::String {
            alloc::format!("{:#?}", val)
        }
        #[allow(unused_imports)]
        use $crate::system::errors::context::IErrorContext;
        #[allow(unused_mut)]
        let mut tmp_ctx_instance = $crate::system::errors::context::nonempty::ErrorContext::default();
        $crate::error_ctx!(@entries tmp_ctx_instance, $($tt)*);
        tmp_ctx_instance
    }};
}

impl Display for ErrorContext {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for element in &self.values {
            write!(f, "{element}")?;
        }

        Ok(())
    }
}
