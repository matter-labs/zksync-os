// Taken from https://github.com/aurora-is-near/aurora-engine, with changes
// to explicitly pass around allocator.
// Also, some parts of code were rewritten to avoid `grow` usage,
// marked with "rewritten without grow usage on alloc for ZKsync OS" comments

// All `as` conversions in this code base have been carefully reviewed
// and are safe.
#![no_std]
#![allow(clippy::as_conversions)]
#![feature(allocator_api)]
extern crate alloc;
use alloc::vec::Vec;
use core::alloc::Allocator;

mod arith;
mod mpnat;

#[macro_export]
macro_rules! vec_in {
    // Repetition form: vec_in!(alloc; elem; n)
    // Creates a vector with `n` copies of `elem`.
    ($alloc:expr; $elem:expr; $n:expr) => {{
        let mut v = Vec::with_capacity_in($n, $alloc);
        v.resize($n, $elem);
        v
    }};

    // List form: vec_in!(alloc; val1, val2, val3, ...)
    // Creates a vector containing the listed values.
    // rewritten without grow usage on alloc for ZKsync OS
    // Only supports 1 element, but it's enough for current impl
    // ($alloc:expr; $( $x:expr ),* $(,)?) => {{
    ($alloc:expr; $x:expr) => {{
        let mut v = Vec::with_capacity_in(1, $alloc);
        v.push($x);
        v
    }};
}

/// Computes `(base ^ exp) % modulus`, where all values are given as big-endian
/// encoded bytes.
pub fn modexp<A: Allocator + Clone>(
    base: &[u8],
    exp: &[u8],
    modulus: &[u8],
    allocator: A,
) -> Vec<u8, A> {
    let m = mpnat::MPNat::from_big_endian(modulus, allocator.clone());
    if m.digits.len() == 1 && m.digits[0] == 0 {
        return Vec::new_in(allocator);
    }
    let mut x = mpnat::MPNat::from_big_endian(base, allocator.clone());
    let result = x.modpow(exp, &m, allocator.clone());
    result.to_big_endian(allocator)
}
