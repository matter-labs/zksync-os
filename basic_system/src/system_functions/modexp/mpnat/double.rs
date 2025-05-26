use core::mem::MaybeUninit;

use zk_ee::system::logger::Logger;

use super::U256;

#[repr(transparent)]
#[derive(Debug)]
pub(crate) struct U512(pub(crate) [U256; 2]);

impl From<&U256> for U512 {

    fn from(value: &U256) -> Self {
        Self([value.clone(), U256::ZERO])
    }
}

impl U512 {
    pub(crate) fn zero() -> Self {
        Self([U256::zero(), U256::zero()])
    }

    pub(crate) const fn zero_const() -> Self {
        Self([U256::ZERO, U256::ZERO])
    }

    // pub(crate) const ONE: Self = Self([U256::ONE, U256::ZERO]);

    pub(crate) fn low(&self) -> &U256 {
        &self.0[0]
    }

    pub(crate) fn high(&self) -> &U256 {
        &self.0[1]
    }

    pub(crate) fn to_words(&self) -> (U256, U256) {
        match self.0 { [ref lo, ref hi] => (lo.clone(), hi.clone()) }
    }

    // pub(crate) fn from_words(lo: U256, hi: U256) -> Self {
    //     Self([lo, hi])
    // }
    //
    // pub(crate) fn add_assign(&mut self, rhs: &Self) -> bool {
    //     let mut carry = false;
    //
    //     for i in 0..2 {
    //         carry = self.0[i].overflowing_add_assign_with_carry_propagation(&rhs.0[i], carry);
    //     }
    //
    //     carry
    // }

    pub(crate) fn add_assign_narrow(&mut self, rhs: &U256) -> bool {
        let carry = self.0[0].overflowing_add_assign(rhs);

        match carry {
            true => self.0[1].overflowing_add_assign(&U256::ONE),
            false => false
        }
    }
 
    pub(crate) fn from_narrow_mul_into<L: Logger>(logger: &mut L, lhs: &U256, rhs: &U256, out: &mut [MaybeUninit<U256>; 2]) {
        lhs.clone_into_unchecked(&mut out[0]);
        lhs.clone_into_unchecked(&mut out[1]);

        let out = unsafe { core::mem::transmute::<&mut [MaybeUninit<U256>; 2], &mut [U256; 2]>(out) };

        let r = out;

        let (r1, r2) = r.split_at_mut(1);

        r1[0].widening_mul_assign_into(&mut r2[0], rhs);
    }
}
