#![cfg_attr(not(feature = "testing"), no_std)]
#![feature(allocator_api)]
#![feature(iter_advance_by)]
#![allow(incomplete_features)]
#![feature(generic_const_exprs)]
#![feature(str_from_raw_parts)]
#![allow(clippy::bool_comparison)]
#![allow(clippy::type_complexity)]
#![allow(clippy::needless_borrow)]
#![allow(clippy::new_without_default)]
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::needless_borrows_for_generic_args)]

extern crate alloc;

// use ruint::aliases::{B160, U256};
// use zk_ee::system::reference_implementations::BaseResources;

// use zk_ee::system_trait::*;
// use zk_ee::system::types_config::*;

// pub mod deployment_artifacts;
// pub mod frame;
// pub mod host;
// pub mod host_ops;
// pub mod interpreter;
// pub mod memory_manager;

#[macro_export]
macro_rules! dev_format {
    ($($arg:tt)*) => {{
        #[cfg(feature = "testing")]
        {
            format!($($arg)*)
        }
        #[cfg(not(feature = "testing"))]
        {
            ""
        }
    }};
}
