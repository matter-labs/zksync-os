use zk_ee::system::logger::Logger;

use super::U256;

#[repr(transparent)]
#[derive(Debug)]
pub(crate) struct U512([U256; 2]);

impl From<&U256> for U512 {
    fn from(value: &U256) -> Self {
        Self([value.clone(), U256::ZERO])
    }
}

impl U512 {
    // pub(crate) const ONE: Self = Self([U256::ONE, U256::ZERO]);

    // pub(crate) fn low(&self) -> &U256 {
    //     &self.0[0]
    // }
    //
    // pub(crate) fn high(&self) -> &U256 {
    //     &self.0[1]
    // }

    pub(crate) fn into_words(self) -> (U256, U256) {
        match self.0 { [lo, hi] => (lo, hi) }
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
 
    pub(crate) fn from_narrow_mul<L: Logger>(logger: &mut L, lhs: &U256, rhs: &U256) -> Self {
        let mut r = Self([lhs.clone(), lhs.clone()]);

        let (r1, r2) = r.0.split_at_mut(1);

        r1[0].widening_mul_assign_into(&mut r2[0], rhs);

        r
    }
}
