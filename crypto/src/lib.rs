#![cfg_attr(not(test), no_std)]
#![feature(array_chunks)]
#![allow(static_mut_refs)]
#![allow(clippy::uninit_assumed_init)]
#![allow(clippy::new_without_default)]
#![feature(allocator_api)]

#[allow(clippy::all)]
#[allow(unused_imports, dead_code)]
#[cfg(any(
    all(target_arch = "riscv32", feature = "bigint_ops"),
    feature = "proving",
    feature = "testing",
    test
))]
mod ark_ff_delegation;
#[allow(clippy::all)]
#[allow(unused_imports, dead_code)]
#[cfg(any(
    all(target_arch = "riscv32", feature = "bigint_ops"),
    feature = "proving",
    feature = "testing",
    test
))]
mod bigint_delegation;
#[allow(unexpected_cfgs)]
pub mod blake2s;
#[allow(clippy::all)]
pub mod bls12_381;
#[allow(clippy::all)]
pub mod bn254;
mod glv_decomposition;
pub mod k256;
pub mod p256;
pub mod ripemd160;
pub mod secp256k1;
pub mod secp256r1;
pub mod sha256;
pub mod sha3;

#[cfg(any(
    all(target_arch = "riscv32", feature = "bigint_ops"),
    feature = "proving",
    feature = "testing",
    test
))]
mod raw_delegation_interface;

pub use blake2 as blake2_ext;

pub use ark_ec;
pub use ark_ff;
pub use ark_serialize;

#[cfg(any(
    all(target_arch = "riscv32", feature = "bigint_ops"),
    feature = "proving",
    feature = "testing",
    test
))]
pub use self::raw_delegation_interface::{
    bigint_op_delegation_raw, bigint_op_delegation_with_carry_bit_raw,
};

pub fn init_lib() {
    #[cfg(any(all(target_arch = "riscv32", feature = "bigint_ops"), test))]
    {
        bn254::fields::init();
        bls12_381::fields::init();
        secp256k1::init();
        bigint_delegation::init();
        secp256r1::init();
    }
}

pub enum BigIntOps {
    Add = 0,
    Sub = 1,
    SubAndNegate = 2,
    MulLow = 3,
    MulHigh = 4,
    Eq = 5,
    MemCpy = 7,
}

pub trait MiniDigest: Sized {
    type HashOutput;

    fn new() -> Self;
    fn digest(input: impl AsRef<[u8]>) -> Self::HashOutput;
    fn update(&mut self, input: impl AsRef<[u8]>);
    fn finalize(self) -> Self::HashOutput;
    fn finalize_reset(&mut self) -> Self::HashOutput;
}
