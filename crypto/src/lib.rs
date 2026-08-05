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
pub mod ark_ff_delegation;
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
    test
))]
pub use self::ark_ff_delegation::{BigInt, BigInteger};

#[cfg(not(any(
    all(target_arch = "riscv32", feature = "bigint_ops"),
    feature = "proving",
    test
)))]
pub use self::ark_ff::{BigInt, BigInteger};

#[cfg(any(
    all(target_arch = "riscv32", feature = "bigint_ops"),
    feature = "proving",
    test
))]
pub use crate::ark_ff_delegation::Fp;

#[cfg(not(any(
    all(target_arch = "riscv32", feature = "bigint_ops"),
    feature = "proving",
    test
)))]
pub use ark_ff::Fp;

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

///
/// Parse the byte array as a BE 32-byte BigInt.
/// If length is less than 32 bytes, it will be left-padded (most significant bytes) with zeroes.
///
pub fn parse_u256_be<const N: usize>(input: &[u8; N]) -> BigInt<4> {
    assert!(N <= 32);
    // Arkworks has strange format for integer serialization, so we do manually
    let mut repr = [0u64; 4];
    let mut repr_iter = repr.iter_mut();
    let (remainder, chunks) = input.as_rchunks::<8>();
    for chunk in chunks.iter().rev() {
        *repr_iter.next().unwrap() = u64::from_be_bytes(*chunk);
    }
    if !remainder.is_empty() {
        let mut buff = [0u8; 8];
        buff[8 - remainder.len()..].copy_from_slice(remainder);
        *repr_iter.next().unwrap() = u64::from_be_bytes(buff);
    }
    BigInt::new(repr)
}
