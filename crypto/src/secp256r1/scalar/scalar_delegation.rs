use core::mem::MaybeUninit;

use crate::ark_ff_delegation::BigInt;
use crate::bigint_delegation::{u256, DelegatedModParams, DelegatedMontParams};
use crate::secp256r1::Secp256r1Err;

static mut MODULUS: MaybeUninit<BigInt<4>> = MaybeUninit::uninit();
static mut REDUCTION_CONST: MaybeUninit<BigInt<4>> = MaybeUninit::uninit();
static mut R2: MaybeUninit<BigInt<4>> = MaybeUninit::uninit(); 

pub(crate) fn init() {
    unsafe {
        MODULUS.write(BigInt::<4>(super::MODULUS));
        REDUCTION_CONST.write(BigInt::<4>(super::REDUCTION_CONST));
        R2.write(BigInt::<4>(super::R2));
    }
}

#[derive(Default)]
struct ScalarParams;

impl DelegatedModParams<4> for ScalarParams {
    unsafe fn modulus() -> &'static BigInt<4> {
        MODULUS.assume_init_ref()
    }
}

impl DelegatedMontParams<4> for ScalarParams {
    unsafe fn reduction_const() -> &'static BigInt<4> {
        REDUCTION_CONST.assume_init_ref()
    }
}

pub(crate) struct Scalar(BigInt<4>);

impl Scalar {
    pub(super) fn to_repressentation(mut self) -> Self {
        unsafe {
            u256::mul_assign_montgomery::<ScalarParams>(&mut self.0, R2.assume_init_ref());
        }
        self
    }

    pub(super) fn to_integer(mut self) -> Self {
        unsafe {
            u256::mul_assign_montgomery::<ScalarParams>(&mut self.0, &BigInt::one());
        }
        self
    }

    pub(super) fn reduce_be_bytes(bytes: &[u8; 32]) -> Self {
        Self::from_be_bytes_unchecked(bytes).to_repressentation()
    }

    pub(super) fn from_be_bytes_unchecked(bytes: &[u8; 32]) -> Self {
        Self(u256::from_bytes_unchecked(bytes))
    }

    pub(super) fn from_be_bytes(bytes: &[u8; 32]) -> Result<Self, Secp256r1Err> {
        let val = Self::from_be_bytes_unchecked(bytes);
        let modulus = unsafe { ScalarParams::modulus() };

        if u256::lt(&val.0, modulus) {
            Ok(val.to_repressentation())
        } else {
            Err(Secp256r1Err::InvalidFieldBytes)
        }
    }

    pub(super) fn to_words(self) -> [u64; 4] {
        self.to_integer().0.0
    }

    pub(crate) fn is_zero(&self) -> bool {
        u256::is_zero(&self.0)
    }

    pub(super) fn square_assign(&mut self) {
        unsafe {
            u256::square_assign_montgomery::<ScalarParams>(&mut self.0);
        }
    }

    pub(super) fn mul_assign(&mut self, rhs: &Self) {
        unsafe {
            u256::mul_assign_montgomery::<ScalarParams>(&mut self.0, &rhs.0);
        }
    }

    pub(super) fn neg_assign(&mut self) {
        unsafe {
            u256::neg_mod_assign::<ScalarParams>(&mut self.0);
        }
    }

    pub(super) fn eq_inner(&self, other: &Self) -> bool {
        u256::eq(&self.0, &other.0)
    }
}

