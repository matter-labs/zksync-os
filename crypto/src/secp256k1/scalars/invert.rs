use super::Scalar;

impl Scalar {
    // using addition chain from
    // https://briansmith.org/ecc-inversion-addition-chains-01#secp256k1_scalar_inversion
    pub(crate) fn invert_in_place(&mut self) {
        let x_1 = self.clone();
        self.pow2k_in_place(1);
        let x_10 = self.clone();
        let mut x_11 = x_10.clone();
        x_11 *= &x_1;
        let mut x_101 = x_10.clone();
        x_101 *= &x_11;
        let mut x_111 = x_10.clone();
        x_111 *= &x_101;
        let mut x_1001 = x_10.clone();
        x_1001 *= &x_111;
        let mut x_1011 = x_10;
        x_1011 *= &x_1001;
        *self *= &x_1011;
        let x_1101 = self.clone();
        self.pow2k_in_place(2);
        *self *= &x_1011;
        let x6 = self.clone();
        self.pow2k_in_place(2);
        *self *= &x_11;
        let x8 = self.clone();
        self.pow2k_in_place(6);
        *self *= &x6;
        let x14 = self.clone();
        self.pow2k_in_place(14);
        *self *= &x14;
        let x28 = self.clone();
        self.pow2k_in_place(28);
        *self *= &x28;
        let x56 = self.clone();
        self.pow2k_in_place(56);
        *self *= &x56;
        self.pow2k_in_place(14);
        *self *= &x14;
        self.pow2k_in_place(3);
        *self *= &x_101;
        self.pow2k_in_place(4);
        *self *= &x_111;
        self.pow2k_in_place(4);
        *self *= &x_101;
        self.pow2k_in_place(5);
        *self *= &x_1011;
        self.pow2k_in_place(4);
        *self *= &x_1011;
        self.pow2k_in_place(4);
        *self *= &x_111;
        self.pow2k_in_place(5);
        *self *= &x_111;
        self.pow2k_in_place(6);
        *self *= &x_1101;
        self.pow2k_in_place(4);
        *self *= &x_101;
        self.pow2k_in_place(3);
        *self *= &x_111;
        self.pow2k_in_place(5);
        *self *= &x_1001;
        self.pow2k_in_place(6);
        *self *= &x_101;
        self.pow2k_in_place(10);
        *self *= &x_111;
        self.pow2k_in_place(4);
        *self *= &x_111;
        self.pow2k_in_place(9);
        *self *= &x8;
        self.pow2k_in_place(5);
        *self *= &x_1001;
        self.pow2k_in_place(6);
        *self *= &x_1011;
        self.pow2k_in_place(4);
        *self *= &x_1101;
        self.pow2k_in_place(5);
        *self *= &x_11;
        self.pow2k_in_place(6);
        *self *= &x_1101;
        self.pow2k_in_place(10);
        *self *= &x_1101;
        self.pow2k_in_place(4);
        *self *= &x_1001;
        self.pow2k_in_place(6);
        *self *= &x_1;
        self.pow2k_in_place(8);
        *self *= &x6;
    }

    #[inline(always)]
    fn pow2k_in_place(&mut self, k: usize) {
        for _ in 0..k {
            self.0.square_in_place();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Scalar;
    use proptest::{prop_assert_eq, proptest};
    #[test]
    fn test_invert() {
        #[cfg(feature = "bigint_ops")]
        crate::secp256k1::init();

        proptest!(|(x: Scalar)| {
            let mut a = x.clone();
            a.invert_in_place();
            a.invert_in_place();
            prop_assert_eq!(a, x.clone());

            a = x.clone();
            a.invert_in_place();
            a *= &x;

            if x.is_zero() {
                prop_assert_eq!(a, Scalar::ZERO);
            } else {
                prop_assert_eq!(a, Scalar::ONE);
            }
        })
    }
}
