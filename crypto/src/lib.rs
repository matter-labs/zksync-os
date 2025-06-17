#![cfg_attr(not(test), no_std)]
#![feature(array_chunks)]
#![allow(static_mut_refs)]
#![feature(ptr_as_ref_unchecked)]
#![allow(clippy::uninit_assumed_init)]
#![allow(clippy::new_without_default)]

#[cfg(any(all(target_arch = "riscv32", feature = "bigint_ops"), test))]
mod ark_ff_delegation;
#[allow(unused_imports)]
#[cfg(any(all(target_arch = "riscv32", feature = "bigint_ops"), test))]
mod bigint_delegation;
#[allow(unexpected_cfgs)]
pub mod blake2s;
#[allow(clippy::all)]
pub mod bls12_381;
#[allow(clippy::all)]
pub mod bn254;
pub mod k256;
pub mod modexp;
pub mod p256;
pub mod ripemd160;
pub mod secp256k1;
pub mod sha256;
pub mod sha3;

pub use blake2 as blake2_ext;

pub use ark_ec;
pub use ark_ff;
pub use ark_serialize;

pub fn init_lib() {
    #[cfg(any(all(target_arch = "riscv32", feature = "bigint_ops"), test))]
    {
        secp256k1::init();
    }
    #[cfg(any(all(target_arch = "riscv32", feature = "bigint_ops"), test))]
    {
        bigint_riscv::init();

        bn254::fields::init();
        bls12_381::fields::init();
        bigint_delegation::init();
    }
    #[cfg(feature = "bigint_ops")]
    {
        // #[cfg(all(target_arch = "riscv32", feature = "bigint_ops"))]
        bigint_riscv::init();
        secp256k1::init();
    }
}

pub trait MiniDigest: Sized {
    type HashOutput;

    fn new() -> Self;
    fn digest(input: impl AsRef<[u8]>) -> Self::HashOutput;
    fn update(&mut self, input: impl AsRef<[u8]>);
    fn finalize(self) -> Self::HashOutput;
}
