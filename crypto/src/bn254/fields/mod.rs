#[cfg(any(all(target_arch = "riscv32", feature = "bigint_ops"), test))]
mod fq;

#[cfg(any(all(target_arch = "riscv32", feature = "bigint_ops"), test))]
pub use self::fq::{init, Fq};

#[cfg(not(any(all(target_arch = "riscv32", feature = "bigint_ops"), test)))]
pub use ark_bn254::Fq;

// Scalar field is default impl for now
pub use ark_bn254::Fr;

pub mod fq2;
pub use self::fq2::*;

pub mod fq6;
pub use self::fq6::*;

pub mod fq12;
pub use self::fq12::*;
