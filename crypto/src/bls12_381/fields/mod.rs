// #[cfg(any(all(target_arch = "riscv32", feature = "bigint_ops"), test))]
// pub mod fq;
// #[cfg(any(all(target_arch = "riscv32", feature = "bigint_ops"), test))]
// pub use self::fq::{init, Fq};

#[cfg(any(all(target_arch = "riscv32", feature = "bigint_ops"), test))]
pub mod fq;
#[cfg(any(all(target_arch = "riscv32", feature = "bigint_ops"), test))]
pub use self::fq::Fq;

#[cfg(not(any(all(target_arch = "riscv32", feature = "bigint_ops"), test)))]
pub use ark_bls12_381::Fq;

#[cfg(not(any(all(target_arch = "riscv32", feature = "bigint_ops"), test)))]
pub use ark_bls12_381::Fr;

#[cfg(any(all(target_arch = "riscv32", feature = "bigint_ops"), test))]
pub mod fr;
#[cfg(any(all(target_arch = "riscv32", feature = "bigint_ops"), test))]
pub use self::fr::Fr;

pub mod fq2;
pub use self::fq2::*;

pub mod fq6;
pub use self::fq6::*;

pub mod fq12;
pub use self::fq12::*;
