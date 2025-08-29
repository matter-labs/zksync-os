// based on https://github.com/RustCrypto/elliptic-curves/blob/master/p256/src/arithmetic/scalar/scalar64.rs
use crate::secp256r1::{u64_arithmatic::*, Secp256r1Err};

use super::{MODULUS, MU};

#[derive(Default, Clone, Copy, Debug)]
pub struct Scalar([u64; 4]);

impl Scalar {
    #[cfg(test)]
    pub(crate) const ZERO: Self = Self([0, 0, 0, 0]);
    pub(crate) const ONE: Self = Self([1, 0, 0, 0]);

    pub(crate) fn reduce_be_bytes(bytes: &[u8; 32]) -> Self {
        let mut val = Self::from_be_bytes_unchecked(bytes);
        val.subtract_modulus();
        val
    }

    pub(super) fn from_be_bytes_unchecked(bytes: &[u8; 32]) -> Self {
        Self([
            u64::from_le_bytes([
                bytes[31], bytes[30], bytes[29], bytes[28], bytes[27], bytes[26], bytes[25],
                bytes[24],
            ]),
            u64::from_le_bytes([
                bytes[23], bytes[22], bytes[21], bytes[20], bytes[19], bytes[18], bytes[17],
                bytes[16],
            ]),
            u64::from_le_bytes([
                bytes[15], bytes[14], bytes[13], bytes[12], bytes[11], bytes[10], bytes[9],
                bytes[8],
            ]),
            u64::from_le_bytes([
                bytes[7], bytes[6], bytes[5], bytes[4], bytes[3], bytes[2], bytes[1], bytes[0],
            ]),
        ])
    }

    pub(crate) fn from_be_bytes(bytes: &[u8; 32]) -> Result<Self, Secp256r1Err> {
        let val = Self::from_be_bytes_unchecked(bytes);
        if val.overflow() {
            Err(Secp256r1Err::InvalidSignature)
        } else {
            Ok(val)
        }
    }

    fn overflow(&self) -> bool {
        let (_, of) = overflowing_sub(&MODULUS, &self.0);
        of
    }

    #[cfg(test)]
    pub(crate) fn from_words(words: [u64; 4]) -> Self {
        Self(words)
    }

    pub(super) fn to_words(self) -> [u64; 4] {
        self.0
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.0 == [0; 4]
    }

    pub(super) fn neg_assign(&mut self) {
        if !self.is_zero() {
            self.0 = overflowing_sub(&MODULUS, &self.0).0;
        }
    }

    pub(super) fn eq_inner(&self, other: &Self) -> bool {
        self.0 == other.0
    }

    fn subtract_modulus(&mut self) {
        let borrow;
        (self.0, borrow) = overflowing_sub(&self.0, &MODULUS);

        if borrow {
            self.0 = overflowing_add(&self.0, &MODULUS).0;
        }
    }

    pub(super) fn square_assign(&mut self) {
        let rhs = *self;
        self.mul_assign(&rhs);
    }

    pub(super) fn mul_assign(&mut self, rhs: &Self) {
        let mut i = 0;
        let mut lo = [0u64; 4];
        let mut hi = [0u64; 4];

        // schoolbook algorithm
        while i < 4 {
            let mut j = 0;
            let mut carry = 0;

            while j < 4 {
                let k = i + j;

                if k >= 4 {
                    let (n, c) = mac(hi[k - 4], self.0[i], rhs.0[j], carry);
                    hi[k - 4] = n;
                    carry = c;
                } else {
                    let (n, c) = mac(lo[k], self.0[i], rhs.0[j], carry);
                    lo[k] = n;
                    carry = c;
                }

                j += 1;
            }

            if i + j >= 4 {
                hi[i + j - 4] = carry;
            } else {
                lo[i + j] = carry;
            }
            i += 1;
        }

        *self = Self(barrett_reduce(lo, hi));
    }
}

fn overflowing_sub(lhs: &[u64; 4], rhs: &[u64; 4]) -> ([u64; 4], bool) {
    let mut borrow = false;
    let mut i = 0;
    let mut res = [0; 4];

    while i < 4 {
        (res[i], borrow) = {
            let (result, borrow_1) = lhs[i].overflowing_sub(rhs[i]);
            let (result, borrow_2) = result.overflowing_sub(borrow as u64);
            (result, borrow_1 | borrow_2)
        };
        i += 1;
    }

    (res, borrow)
}

fn overflowing_add(lhs: &[u64; 4], rhs: &[u64; 4]) -> ([u64; 4], bool) {
    let mut carry = false;
    let mut i = 0;
    let mut res = [0; 4];
    while i < 4 {
        (res[i], carry) = {
            let (result, carry_1) = lhs[i].overflowing_add(rhs[i]);
            let (result, carry_2) = result.overflowing_add(carry as u64);
            (result, carry_1 | carry_2)
        };
        i += 1;
    }

    (res, carry)
}

/// Barrett Reduction
///
/// The general algorithm is:
/// ```text
/// p = n = order of group
/// b = 2^64 = 64bit machine word
/// k = 4
/// a \in [0, 2^512]
/// mu := floor(b^{2k} / p)
/// q1 := floor(a / b^{k - 1})
/// q2 := q1 * mu
/// q3 := <- floor(a / b^{k - 1})
/// r1 := a mod b^{k + 1}
/// r2 := q3 * m mod b^{k + 1}
/// r := r1 - r2
///
/// if r < 0: r := r + b^{k + 1}
/// while r >= p: do r := r - p (at most twice)
/// ```
///
/// References:
/// - Handbook of Applied Cryptography, Chapter 14
///   Algorithm 14.42
///   http://cacr.uwaterloo.ca/hac/about/chap14.pdf
///
/// - Efficient and Secure Elliptic Curve Cryptography Implementation of Curve P-256
///   Algorithm 6) Barrett Reduction modulo p
///   https://csrc.nist.gov/csrc/media/events/workshop-on-elliptic-curve-cryptography-standards/documents/papers/session6-adalier-mehmet.pdf
#[inline]
#[allow(clippy::too_many_arguments)]
const fn barrett_reduce(lo: [u64; 4], hi: [u64; 4]) -> [u64; 4] {
    let a0 = lo[0];
    let a1 = lo[1];
    let a2 = lo[2];
    let a3 = lo[3];
    let a4 = hi[0];
    let a5 = hi[1];
    let a6 = hi[2];
    let a7 = hi[3];
    let q1: [u64; 5] = [a3, a4, a5, a6, a7];
    let q3 = q1_times_mu_shift_five(&q1);

    let r1: [u64; 5] = [a0, a1, a2, a3, a4];
    let r2: [u64; 5] = q3_times_n_keep_five(&q3);
    let r: [u64; 5] = sub_inner_five(r1, r2);

    // Result is in range (0, 3*n - 1),
    // and 90% of the time, no subtraction will be needed.
    let r = subtract_n_if_necessary(r[0], r[1], r[2], r[3], r[4]);
    let r = subtract_n_if_necessary(r[0], r[1], r[2], r[3], r[4]);
    [r[0], r[1], r[2], r[3]]
}

const fn q1_times_mu_shift_five(q1: &[u64; 5]) -> [u64; 5] {
    // Schoolbook multiplication.

    let (_w0, carry) = mac(0, q1[0], MU[0], 0);
    let (w1, carry) = mac(0, q1[0], MU[1], carry);
    let (w2, carry) = mac(0, q1[0], MU[2], carry);
    let (w3, carry) = mac(0, q1[0], MU[3], carry);
    let (w4, w5) = mac(0, q1[0], MU[4], carry);

    let (_w1, carry) = mac(w1, q1[1], MU[0], 0);
    let (w2, carry) = mac(w2, q1[1], MU[1], carry);
    let (w3, carry) = mac(w3, q1[1], MU[2], carry);
    let (w4, carry) = mac(w4, q1[1], MU[3], carry);
    let (w5, w6) = mac(w5, q1[1], MU[4], carry);

    let (_w2, carry) = mac(w2, q1[2], MU[0], 0);
    let (w3, carry) = mac(w3, q1[2], MU[1], carry);
    let (w4, carry) = mac(w4, q1[2], MU[2], carry);
    let (w5, carry) = mac(w5, q1[2], MU[3], carry);
    let (w6, w7) = mac(w6, q1[2], MU[4], carry);

    let (_w3, carry) = mac(w3, q1[3], MU[0], 0);
    let (w4, carry) = mac(w4, q1[3], MU[1], carry);
    let (w5, carry) = mac(w5, q1[3], MU[2], carry);
    let (w6, carry) = mac(w6, q1[3], MU[3], carry);
    let (w7, w8) = mac(w7, q1[3], MU[4], carry);

    let (_w4, carry) = mac(w4, q1[4], MU[0], 0);
    let (w5, carry) = mac(w5, q1[4], MU[1], carry);
    let (w6, carry) = mac(w6, q1[4], MU[2], carry);
    let (w7, carry) = mac(w7, q1[4], MU[3], carry);
    let (w8, w9) = mac(w8, q1[4], MU[4], carry);

    // let q2 = [_w0, _w1, _w2, _w3, _w4, w5, w6, w7, w8, w9];
    [w5, w6, w7, w8, w9]
}

const fn q3_times_n_keep_five(q3: &[u64; 5]) -> [u64; 5] {
    // Schoolbook multiplication.
    let (w0, carry) = mac(0, q3[0], MODULUS[0], 0);
    let (w1, carry) = mac(0, q3[0], MODULUS[1], carry);
    let (w2, carry) = mac(0, q3[0], MODULUS[2], carry);
    let (w3, carry) = mac(0, q3[0], MODULUS[3], carry);
    let (w4, _) = mac(0, q3[0], 0, carry);

    let (w1, carry) = mac(w1, q3[1], MODULUS[0], 0);
    let (w2, carry) = mac(w2, q3[1], MODULUS[1], carry);
    let (w3, carry) = mac(w3, q3[1], MODULUS[2], carry);
    let (w4, _) = mac(w4, q3[1], MODULUS[3], carry);

    let (w2, carry) = mac(w2, q3[2], MODULUS[0], 0);
    let (w3, carry) = mac(w3, q3[2], MODULUS[1], carry);
    let (w4, _) = mac(w4, q3[2], MODULUS[2], carry);

    let (w3, carry) = mac(w3, q3[3], MODULUS[0], 0);
    let (w4, _) = mac(w4, q3[3], MODULUS[1], carry);

    let (w4, _) = mac(w4, q3[4], MODULUS[0], 0);

    [w0, w1, w2, w3, w4]
}

#[inline]
#[allow(clippy::too_many_arguments)]
const fn sub_inner_five(l: [u64; 5], r: [u64; 5]) -> [u64; 5] {
    let (w0, borrow) = sbb(l[0], r[0], 0);
    let (w1, borrow) = sbb(l[1], r[1], borrow);
    let (w2, borrow) = sbb(l[2], r[2], borrow);
    let (w3, borrow) = sbb(l[3], r[3], borrow);
    let (w4, _borrow) = sbb(l[4], r[4], borrow);

    // If underflow occurred on the final limb - don't care (= add b^{k+1}).
    [w0, w1, w2, w3, w4]
}

#[inline]
#[allow(clippy::too_many_arguments)]
const fn subtract_n_if_necessary(r0: u64, r1: u64, r2: u64, r3: u64, r4: u64) -> [u64; 5] {
    let (w0, borrow) = sbb(r0, MODULUS[0], 0);
    let (w1, borrow) = sbb(r1, MODULUS[1], borrow);
    let (w2, borrow) = sbb(r2, MODULUS[2], borrow);
    let (w3, borrow) = sbb(r3, MODULUS[3], borrow);
    let (w4, borrow) = sbb(r4, 0, borrow);

    // If underflow occurred on the final limb, borrow = 0xfff...fff, otherwise
    // borrow = 0x000...000. Thus, we use it as a mask to conditionally add the
    // MODULUS.
    let (w0, carry) = adc(w0, MODULUS[0] & borrow, 0);
    let (w1, carry) = adc(w1, MODULUS[1] & borrow, carry);
    let (w2, carry) = adc(w2, MODULUS[2] & borrow, carry);
    let (w3, carry) = adc(w3, MODULUS[3] & borrow, carry);
    let (w4, _carry) = adc(w4, 0, carry);

    [w0, w1, w2, w3, w4]
}

#[cfg(test)]
mod tests {
    use super::Scalar;

    impl proptest::arbitrary::Arbitrary for Scalar {
        type Parameters = ();

        fn arbitrary_with(_args: Self::Parameters) -> Self::Strategy {
            use proptest::prelude::{any, Strategy};

            any::<[u64; 4]>().prop_map(|x| {
                let mut res = Self(x);
                res.subtract_modulus();
                res
            })
        }

        type Strategy = proptest::arbitrary::Mapped<[u64; 4], Scalar>;
    }
}
