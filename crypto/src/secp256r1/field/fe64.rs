// based on https://github.com/RustCrypto/elliptic-curves/blob/master/p256/src/arithmetic/field/field64.rs
use core::{
    mem::MaybeUninit,
    ops::{AddAssign, MulAssign, SubAssign},
};

use crate::secp256r1::u64_arithmatic::*;

use super::{MODULUS, R2};

#[derive(Clone, Copy, Default)]
pub(crate) struct FieldElement(pub [u64; 4]);

impl core::fmt::Debug for FieldElement {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("0x")?;
        let bytes = self.to_be_bytes();
        for b in bytes.as_slice().iter() {
            f.write_fmt(format_args!("{b:02x}"))?;
        }
        core::fmt::Result::Ok(())
    }
}

impl FieldElement {
    pub(crate) const ZERO: Self = Self::from_words_unchecked([0; 4]);
    // montgomerry form
    pub(crate) const ONE: Self =
        Self::from_words_unchecked([1, 18446744069414584320, 18446744073709551615, 4294967294]);

    pub(crate) fn to_integer(self) -> Self {
        FieldElement(montgomery_reduce(&[
            self.0[0], self.0[1], self.0[2], self.0[3], 0, 0, 0, 0,
        ]))
    }

    pub(super) const fn to_representation(self) -> Self {
        FieldElement(fe_mul(&self.0, &R2))
    }

    pub(crate) const fn from_be_bytes_unchecked(bytes: &[u8; 32]) -> Self {
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

    pub(crate) const fn from_words_unchecked(words: [u64; 4]) -> Self {
        Self(words)
    }

    pub(crate) const fn from_words(words: [u64; 4]) -> Self {
        Self::from_words_unchecked(words).to_representation()
    }

    pub(crate) fn to_be_bytes(mut self) -> [u8; 32] {
        self = self.to_integer();
        let mut r: [MaybeUninit<u8>; 32] = unsafe { MaybeUninit::uninit().assume_init() };

        r[0..8].copy_from_slice(&self.0[3].to_be_bytes().map(MaybeUninit::new));
        r[8..16].copy_from_slice(&self.0[2].to_be_bytes().map(MaybeUninit::new));
        r[16..24].copy_from_slice(&self.0[1].to_be_bytes().map(MaybeUninit::new));
        r[24..32].copy_from_slice(&self.0[0].to_be_bytes().map(MaybeUninit::new));

        unsafe { core::mem::transmute(r) }
    }

    pub(crate) const fn is_zero(&self) -> bool {
        self.0[0] == 0 && self.0[1] == 0 && self.0[2] == 0 && self.0[3] == 0
    }

    pub(crate) fn overflow(&self) -> bool {
        MODULUS <= self.0
    }

    pub(crate) const fn add(&self, other: &Self) -> Self {
        FieldElement(fe_add(&self.0, &other.0))
    }

    pub(crate) const fn sub(&self, other: &Self) -> Self {
        FieldElement(fe_sub(&self.0, &other.0))
    }

    pub(crate) const fn mul_int(&self, other: u32) -> Self {
        let b = Self::from_words([other as u64, 0, 0, 0]);
        FieldElement(fe_mul(&self.0, &b.0))
    }

    pub(crate) const fn mul(&self, other: &Self) -> Self {
        FieldElement(fe_mul(&self.0, &other.0))
    }

    pub(crate) const fn square(&self) -> Self {
        let other = *self;
        self.mul(&other)
    }

    pub(crate) fn square_assign(&mut self) {
        *self = self.square();
    }

    pub(crate) fn negate_assign(&mut self) {
        *self = Self(fe_sub(&MODULUS, &self.0));
    }

    pub(crate) fn double_assign(&mut self) {
        let other = *self;
        self.add_assign(&other);
    }

    /// Computes `self = other - self`
    pub(crate) fn sub_and_negate_assign(&mut self, other: &Self) {
        *self -= other;
        self.negate_assign();
    }
}

impl AddAssign<&Self> for FieldElement {
    fn add_assign(&mut self, rhs: &Self) {
        *self = self.add(rhs);
    }
}

impl SubAssign<&Self> for FieldElement {
    fn sub_assign(&mut self, rhs: &Self) {
        *self = self.sub(rhs);
    }
}

impl MulAssign<&Self> for FieldElement {
    fn mul_assign(&mut self, rhs: &Self) {
        *self = self.mul(rhs);
    }
}

impl MulAssign<u32> for FieldElement {
    fn mul_assign(&mut self, rhs: u32) {
        *self = self.mul_int(rhs);
    }
}

impl PartialEq for FieldElement {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

/// Returns `a + b mod p`.
const fn fe_add(a: &[u64; 4], b: &[u64; 4]) -> [u64; 4] {
    // Bit 256 of p is set, so addition can result in five words.
    let (w0, carry) = adc(a[0], b[0], 0);
    let (w1, carry) = adc(a[1], b[1], carry);
    let (w2, carry) = adc(a[2], b[2], carry);
    let (w3, w4) = adc(a[3], b[3], carry);

    // Attempt to subtract the modulus, to ensure the result is in the field.
    sub_inner(
        &[w0, w1, w2, w3, w4],
        &[MODULUS[0], MODULUS[1], MODULUS[2], MODULUS[3], 0],
    )
}

/// Returns `a - b mod p`.
const fn fe_sub(a: &[u64; 4], b: &[u64; 4]) -> [u64; 4] {
    sub_inner(&[a[0], a[1], a[2], a[3], 0], &[b[0], b[1], b[2], b[3], 0])
}

/// Returns `a * b mod p`.
const fn fe_mul(a: &[u64; 4], b: &[u64; 4]) -> [u64; 4] {
    let (w0, carry) = mac(0, a[0], b[0], 0);
    let (w1, carry) = mac(0, a[0], b[1], carry);
    let (w2, carry) = mac(0, a[0], b[2], carry);
    let (w3, w4) = mac(0, a[0], b[3], carry);

    let (w1, carry) = mac(w1, a[1], b[0], 0);
    let (w2, carry) = mac(w2, a[1], b[1], carry);
    let (w3, carry) = mac(w3, a[1], b[2], carry);
    let (w4, w5) = mac(w4, a[1], b[3], carry);

    let (w2, carry) = mac(w2, a[2], b[0], 0);
    let (w3, carry) = mac(w3, a[2], b[1], carry);
    let (w4, carry) = mac(w4, a[2], b[2], carry);
    let (w5, w6) = mac(w5, a[2], b[3], carry);

    let (w3, carry) = mac(w3, a[3], b[0], 0);
    let (w4, carry) = mac(w4, a[3], b[1], carry);
    let (w5, carry) = mac(w5, a[3], b[2], carry);
    let (w6, w7) = mac(w6, a[3], b[3], carry);

    montgomery_reduce(&[w0, w1, w2, w3, w4, w5, w6, w7])
}

/// Montgomery Reduction
///
/// The general algorithm is:
/// ```text
/// A <- input (2n b-limbs)
/// for i in 0..n {
///     k <- A[i] p' mod b
///     A <- A + k p b^i
/// }
/// A <- A / b^n
/// if A >= p {
///     A <- A - p
/// }
/// ```
///
/// For secp256r1, we have the following simplifications:
///
/// - `p'` is 1, so our multiplicand is simply the first limb of the intermediate A.
///
/// - The first limb of p is 2^64 - 1; multiplications by this limb can be simplified
///   to a shift and subtraction:
///   ```text
///       a_i * (2^64 - 1) = a_i * 2^64 - a_i = (a_i << 64) - a_i
///   ```
///   However, because `p' = 1`, the first limb of p is multiplied by limb i of the
///   intermediate A and then immediately added to that same limb, so we simply
///   initialize the carry to limb i of the intermediate.
///
/// - The third limb of p is zero, so we can ignore any multiplications by it and just
///   add the carry.
///
/// References:
/// - Handbook of Applied Cryptography, Chapter 14
///   Algorithm 14.32
///   http://cacr.uwaterloo.ca/hac/about/chap14.pdf
///
/// - Efficient and Secure Elliptic Curve Cryptography Implementation of Curve P-256
///   Algorithm 7) Montgomery Word-by-Word Reduction
///   https://csrc.nist.gov/csrc/media/events/workshop-on-elliptic-curve-cryptography-standards/documents/papers/session6-adalier-mehmet.pdf
#[inline]
#[allow(clippy::too_many_arguments)]
const fn montgomery_reduce(r: &[u64; 8]) -> [u64; 4] {
    let r0 = r[0];
    let r1 = r[1];
    let r2 = r[2];
    let r3 = r[3];
    let r4 = r[4];
    let r5 = r[5];
    let r6 = r[6];
    let r7 = r[7];

    let (r1, carry) = mac(r1, r0, MODULUS[1], r0);
    let (r2, carry) = adc(r2, 0, carry);
    let (r3, carry) = mac(r3, r0, MODULUS[3], carry);
    let (r4, carry2) = adc(r4, 0, carry);

    let (r2, carry) = mac(r2, r1, MODULUS[1], r1);
    let (r3, carry) = adc(r3, 0, carry);
    let (r4, carry) = mac(r4, r1, MODULUS[3], carry);
    let (r5, carry2) = adc(r5, carry2, carry);

    let (r3, carry) = mac(r3, r2, MODULUS[1], r2);
    let (r4, carry) = adc(r4, 0, carry);
    let (r5, carry) = mac(r5, r2, MODULUS[3], carry);
    let (r6, carry2) = adc(r6, carry2, carry);

    let (r4, carry) = mac(r4, r3, MODULUS[1], r3);
    let (r5, carry) = adc(r5, 0, carry);
    let (r6, carry) = mac(r6, r3, MODULUS[3], carry);
    let (r7, r8) = adc(r7, carry2, carry);

    // Result may be within MODULUS of the correct value
    sub_inner(
        &[r4, r5, r6, r7, r8],
        &[MODULUS[0], MODULUS[1], MODULUS[2], MODULUS[3], 0],
    )
}

#[inline]
#[allow(clippy::too_many_arguments)]
const fn sub_inner(l: &[u64; 5], r: &[u64; 5]) -> [u64; 4] {
    let (w0, borrow) = sbb(l[0], r[0], 0);
    let (w1, borrow) = sbb(l[1], r[1], borrow);
    let (w2, borrow) = sbb(l[2], r[2], borrow);
    let (w3, borrow) = sbb(l[3], r[3], borrow);
    let (_, borrow) = sbb(l[4], r[4], borrow);

    // If underflow occurred on the final limb, borrow = 0xfff...fff, otherwise
    // borrow = 0x000...000. Thus, we use it as a mask to conditionally add the
    // modulus.
    let (w0, carry) = adc(w0, MODULUS[0] & borrow, 0);
    let (w1, carry) = adc(w1, MODULUS[1] & borrow, carry);
    let (w2, carry) = adc(w2, MODULUS[2] & borrow, carry);
    let (w3, _) = adc(w3, MODULUS[3] & borrow, carry);

    [w0, w1, w2, w3]
}

#[cfg(test)]
mod tests {
    use super::*;

    impl proptest::arbitrary::Arbitrary for FieldElement {
        type Parameters = ();

        fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
            use proptest::prelude::{any, Strategy};

            any::<[u64; 4]>().prop_map(|limbs| {
                if limbs < MODULUS {
                    Self(limbs).to_representation()
                } else {
                    Self(sub_inner(
                        &[limbs[0], limbs[1], limbs[2], limbs[3], 0],
                        &[MODULUS[0], MODULUS[1], MODULUS[2], MODULUS[3], 0],
                    ))
                    .to_representation()
                }
            })
        }

        type Strategy = proptest::arbitrary::Mapped<[u64; 4], FieldElement>;
    }
}
