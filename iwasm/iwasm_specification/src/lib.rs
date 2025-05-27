#![allow(incomplete_features)]
#![feature(generic_const_exprs)]
#![cfg_attr(not(test), no_std)]

#[macro_use]
extern crate num_derive;

pub use num_traits;

pub mod host_ops;
pub mod intx;
pub mod sys;
