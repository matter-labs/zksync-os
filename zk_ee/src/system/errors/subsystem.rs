use super::{
    cascade::{CascadedError, ICascadedInner},
    context::{contextualized::Contextualized, ErrorContext},
    interface::{InterfaceError, InterfaceErrorKind},
    internal::InternalError,
    location::ErrorLocation,
    no_errors::NoErrors,
    runtime::RuntimeError,
};

pub trait Subsystem: core::fmt::Debug {
    const SUBSYSTEM_NAME: &'static str;
    type Interface: InterfaceErrorKind = NoErrors;
    type Cascaded: ICascadedInner = NoErrors;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubsystemError<S: Subsystem> {
    LeafUsage(InterfaceError<S::Interface>),
    LeafDefect(InternalError),
    LeafRuntime(RuntimeError),
    Cascaded(CascadedError<S::Cascaded>),
}

impl<S: Subsystem> From<InterfaceError<S::Interface>> for SubsystemError<S> {
    fn from(v: InterfaceError<S::Interface>) -> Self {
        SubsystemError::LeafUsage(v)
    }
}

impl<F: Subsystem> SubsystemError<F> {
    pub fn wrap<T: Subsystem<Cascaded: From<SubsystemError<F>>>>(
        self,
        loc: ErrorLocation,
    ) -> SubsystemError<T> {
        SubsystemError::Cascaded(CascadedError(self.into(), loc.into()))
    }
}

#[macro_export]
macro_rules! define_interface_error {
    (
        $vis:vis $name:ident {
            $(
                $variant:ident $( { $($field:ident : $ty:ty),* $(,)? } )?
            ),* $(,)?
        }
    ) => {
        #[derive(Clone, Debug, Eq, PartialEq, ::strum_macros::IntoStaticStr)]
        $vis enum $name {
            $(
                $variant $( { $($field : $ty),* } )?,
            )*
        }

        impl $crate::system::errors::interface::InterfaceErrorKind for $name {
            fn get_name(&self) -> &'static str {
                self.into()
            }
        }
    };
}

#[macro_export]
macro_rules! define_cascade_error {
    (
        $vis:vis $name:ident {
            $($variant:ident($inner:ty)),* $(,)?
        }
    ) => {
        #[derive(Clone, Debug, Eq, PartialEq)]
        $vis enum $name {
            $(
                $variant($inner),
            )*
        }

        $(
            impl From<$inner> for $name {
                fn from(v: $inner) -> Self {
                    Self::$variant(v)
                }
            }
        )*

        impl $crate::system::errors::root_cause::GetRootCause for $name {
            fn root_cause(&self) -> $crate::system::errors::root_cause::RootCause {
                match self {
                    $(
                        Self::$variant(e) => e.root_cause(),
                    )*
                }
            }
        }
        impl core::fmt::Display for $name {

            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                match self {
                    $(
                        Self::$variant(e) => write!(f, "{e}"),
                    )*
                }
            }
        }

        impl $crate::system::errors::cascade::ICascadedInner for $name {}
    };
}

#[macro_export]
macro_rules! define_subsystem {
    (@type_alias $prefix:ident) => {
        paste::paste! {
            pub type [< $prefix SubsystemError >] = $crate::system::errors::subsystem::SubsystemError< [< $prefix Errors >] >  ;
        }
    };
    (@implement_trait $prefix:ident, $interface:ty, $wrapper:ty) => {
        paste::paste! {
            #[derive(Clone, Debug, Eq, PartialEq)]
            pub struct [< $prefix Errors >] ;

            impl $crate::system::errors::subsystem::Subsystem for [< $prefix Errors >] {
                const SUBSYSTEM_NAME: &'static str = stringify!($prefix);
                type Interface = $interface;
                type Cascaded = $wrapper;
            }
        }
    };
    // Both interface and wrapper
    (
        $prefix:ident
            , interface $interface_name:ident {
                $(
                    $interface_variant:ident $( { $($interface_field:ident : $interface_ty:ty),* $(,)? } )?
                ),* $(,)?
            }
            , cascade $wrapper_name:ident {
                $(
                    $wrapper_variant:ident($wrapped_ty:ty)
                ),* $(,)?
            }
            $(,)?
    ) => {
        $crate::define_interface_error! {
            pub $interface_name {
                $(
                    $interface_variant $( { $($interface_field : $interface_ty),* } )?
                ),*
            }
        }
        $crate::define_cascade_error! {
            pub $wrapper_name {
                $(
                    $wrapper_variant($wrapped_ty)
                ),*
            }
        }

        $crate::define_subsystem!(@implement_trait $prefix, $interface_name, $wrapper_name);
        $crate::define_subsystem!(@type_alias $prefix);
    };

    // Just wrapped errors
    (
       $prefix:ident
            , cascade $wrapper_name:ident {
                $(
                    $wrapper_variant:ident($wrapped_ty:ty)
                ),* $(,)?
            }
            $(,)?
    ) => {
        $crate::define_cascade_error! {
            pub $wrapper_name {
                $(
                    $wrapper_variant($wrapped_ty)
                ),*
            }
        }


        $crate::define_subsystem!(@implement_trait $prefix, $crate::system::errors::no_errors::NoErrors, $wrapper_name);
        $crate::define_subsystem!(@type_alias $prefix);
    };
    // Just interface
    (
        $prefix:ident
            , interface $interface_name:ident {
                $(
                    $interface_variant:ident $( { $($interface_field:ident : $interface_ty:ty),* $(,)? } )?
                ),* $(,)?
            }
            $(,)?
    ) => {
        $crate::define_interface_error! {
            pub $interface_name {
                $(
                    $interface_variant $( { $($interface_field : $interface_ty),* } )?
                ),*
            }
        }
        $crate::define_subsystem!(@implement_trait $prefix, $interface_name, $crate::system::errors::no_errors::NoErrors);
        $crate::define_subsystem!(@type_alias $prefix);
    };
    // No interface, no wrapped errors
    (
        $prefix:ident
              ) => {
        $crate::define_subsystem!(@implement_trait $prefix, $crate::system::errors::no_errors::NoErrors, $crate::system::errors::no_errors::NoErrors);
        $crate::define_subsystem!(@type_alias $prefix);
    };


}

impl<S: Subsystem> From<RuntimeError> for SubsystemError<S> {
    fn from(v: RuntimeError) -> Self {
        SubsystemError::LeafRuntime(v)
    }
}

impl<S: Subsystem> From<InternalError> for SubsystemError<S> {
    fn from(v: InternalError) -> Self {
        SubsystemError::LeafDefect(v)
    }
}

impl<S: Subsystem, E> Contextualized<SubsystemError<S>> for E
where
    E: Into<SubsystemError<S>>,
{
    fn with_context_inner<F>(self, f: F) -> SubsystemError<S>
    where
        F: FnOnce() -> ErrorContext,
    {
        match Into::into(self) {
            SubsystemError::Cascaded(e) => SubsystemError::Cascaded(e.with_context(f)),
            SubsystemError::LeafUsage(interface_error) => {
                SubsystemError::LeafUsage(interface_error.with_context(f))
            }
            SubsystemError::LeafDefect(internal_error) => {
                SubsystemError::LeafDefect(internal_error.with_context(f))
            }
            SubsystemError::LeafRuntime(runtime_error) => {
                SubsystemError::LeafRuntime(runtime_error.with_context(f))
            }
        }
    }
}
