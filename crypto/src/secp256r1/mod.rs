#[allow(long_running_const_eval)]
mod context;
mod field;
mod points;
mod scalar;
mod u64_arithmatic;
mod verify;
mod wnaf;

#[cfg(test)]
mod test_vectors;

use core::fmt::Debug;
use core::fmt::Display;

pub(crate) const WINDOW_A: usize = 5;

pub(crate) const WINDOW_G: usize = 10;

pub(crate) const ECMULT_TABLE_SIZE_A: usize = 1 << (WINDOW_A - 2);
pub(crate) const ECMULT_TABLE_SIZE_G: usize = 1 << (WINDOW_G - 2);
pub(crate) const WNAF_BITS: usize = 256;

#[cfg(any(all(target_arch = "riscv32", feature = "bigint_ops"), test))]
pub fn init() {
    field::init();
    scalar::init();
}

pub use verify::verify;

#[derive(Debug)]
pub enum Secp256r1Err {
    InvalidSignature,
    InvalidCoordinates,
    InvalidFieldBytes,
    RecoveredInfinity,
}

impl Display for Secp256r1Err {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Secp256r1Err::InvalidSignature => {
                write!(f, "secp256r1: Could not recover signature from bytes")
            }
            Secp256r1Err::InvalidCoordinates => write!(
                f,
                "secp256r1: Could not recover curve point from coordinates"
            ),
            Secp256r1Err::RecoveredInfinity => {
                write!(f, "secp256r1: Recieved coordinates of point at infinity")
            }
            Secp256r1Err::InvalidFieldBytes => write!(f, "secp256r1: Field bytes out of range"),
        }
    }
}
