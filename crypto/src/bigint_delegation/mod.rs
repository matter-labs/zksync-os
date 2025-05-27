use crate::ark_ff_delegation::BigInt;

mod delegation;
pub mod u256;
pub mod u512;

pub fn init() {
    u256::init();
    u512::init();
}

pub trait DelegatedModParams<const N: usize>: Default {
    /// Provides a reference to the modululs for delegation purposes
    /// # Safety
    /// The reference has to be to a value outside the ROM, i.e. a mutable static
    unsafe fn modulus() -> &'static BigInt<N>;
}

pub trait DelegatedMontParams<const N: usize>: DelegatedModParams<N> {
    /// Provides a reference to the reduction const (`-1/Self::modulus mod 2^256`) for Montgomerry reduction
    /// # Safety
    /// The reference has to be to a value outside the ROM, i.e. a mutable static
    unsafe fn reduction_const() -> &'static BigInt<4>;
}

pub trait DelegatedBarretParams<const N: usize>: DelegatedModParams<N> {
    /// Provides a reference to `-Self::modulus mod 2^256` for Barret reduction
    /// # Safety
    /// The reference has to be to a value outside the ROM, i.e. a mutable static
    unsafe fn neg_modulus() -> &'static BigInt<4>;
}
