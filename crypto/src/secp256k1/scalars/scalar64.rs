// modified version of https://github.com/RustCrypto/elliptic-curves/blob/master/k256/src/arithmetic/scalar/wide64.rs

use crate::k256::elliptic_curve::bigint::{Encoding, Limb, U256};

const _: () = const {
    assert!(core::mem::size_of::<crate::k256::Scalar>() == core::mem::size_of::<ScalarInner>());
    assert!(core::mem::align_of::<crate::k256::Scalar>() >= core::mem::align_of::<ScalarInner>());
};

#[derive(Clone, Copy)]
pub struct ScalarInner(U256);

impl core::fmt::Debug for ScalarInner {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("0x")?;
        let bytes = self.to_be_bytes();
        for b in bytes.as_slice().iter() {
            f.write_fmt(format_args!("{:02x}", b))?;
        }

        core::fmt::Result::Ok(())
    }
}

impl ScalarInner {
    pub(super) const ZERO: Self = Self(U256::ZERO);
    pub(super) const ONE: Self = Self(U256::ONE);
    pub(super) const ORDER: Self = Self::from_be_hex(super::ORDER_HEX);

    pub(super) const MINUS_LAMBDA: Self = Self::from_be_bytes_unchecked(&[
        0xac, 0x9c, 0x52, 0xb3, 0x3f, 0xa3, 0xcf, 0x1f, 0x5a, 0xd9, 0xe3, 0xfd, 0x77, 0xed, 0x9b,
        0xa4, 0xa8, 0x80, 0xb9, 0xfc, 0x8e, 0xc7, 0x39, 0xc2, 0xe0, 0xcf, 0xc8, 0x10, 0xb5, 0x12,
        0x83, 0xcf,
    ]);

    pub(super) const MINUS_B1: Self = Self::from_be_bytes_unchecked(&[
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0xe4, 0x43, 0x7e, 0xd6, 0x01, 0x0e, 0x88, 0x28, 0x6f, 0x54, 0x7f, 0xa9, 0x0a, 0xbf,
        0xe4, 0xc3,
    ]);

    pub(super) const MINUS_B2: Self = Self::from_be_bytes_unchecked(&[
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xfe, 0x8a, 0x28, 0x0a, 0xc5, 0x07, 0x74, 0x34, 0x6d, 0xd7, 0x65, 0xcd, 0xa8, 0x3d, 0xb1,
        0x56, 0x2c,
    ]);

    pub(super) const G1: Self = Self::from_be_bytes_unchecked(&[
        0x30, 0x86, 0xd2, 0x21, 0xa7, 0xd4, 0x6b, 0xcd, 0xe8, 0x6c, 0x90, 0xe4, 0x92, 0x84, 0xeb,
        0x15, 0x3d, 0xaa, 0x8a, 0x14, 0x71, 0xe8, 0xca, 0x7f, 0xe8, 0x93, 0x20, 0x9a, 0x45, 0xdb,
        0xb0, 0x31,
    ]);

    pub(super) const G2: Self = Self::from_be_bytes_unchecked(&[
        0xe4, 0x43, 0x7e, 0xd6, 0x01, 0x0e, 0x88, 0x28, 0x6f, 0x54, 0x7f, 0xa9, 0x0a, 0xbf, 0xe4,
        0xc4, 0x22, 0x12, 0x08, 0xac, 0x9d, 0xf5, 0x06, 0xc6, 0x15, 0x71, 0xb4, 0xae, 0x8a, 0xc4,
        0x7f, 0x71,
    ]);

    pub(super) const fn from_be_bytes_unchecked(bytes: &[u8; 32]) -> Self {
        Self(U256::from_be_slice(bytes))
    }

    #[cfg(test)]
    pub(super) fn from_be_bytes(bytes: &[u8; 32]) -> Self {
        Self(U256::from_be_slice(bytes))
    }

    #[allow(clippy::missing_transmute_annotations)]
    pub(crate) fn from_k256_scalar(s: crate::k256::Scalar) -> Self {
        unsafe { Self(core::mem::transmute(s)) }
    }

    #[cfg(test)]
    pub(super) const fn from_u128(n: u128) -> Self {
        Self(U256::from_u128(n))
    }

    pub(super) const fn from_be_hex(hex: &str) -> Self {
        Self(U256::from_be_hex(hex))
    }

    pub(super) fn to_be_bytes(self) -> [u8; 32] {
        self.0.to_be_bytes()
    }

    pub(super) fn bits(&self, offset: usize, count: usize) -> u32 {
        // check requested bits must be from the same limb
        debug_assert!((offset + count - 1) >> 6 == offset >> 6);
        let limbs = self.0.as_words();
        ((limbs[offset >> 6] >> (offset & 0x3F)) & ((1 << count) - 1)) as u32
    }

    pub(super) fn bits_var(&self, offset: usize, count: usize) -> u32 {
        debug_assert!(count <= 32);
        debug_assert!(offset + count <= 256);
        // if all the requested bits are in the same limb
        if (offset + count - 1) >> 6 == offset >> 6 {
            self.bits(offset, count)
        } else {
            debug_assert!((offset >> 6) + 1 < 4);
            let limbs = self.0.as_words();
            (((limbs[offset >> 6] >> (offset & 0x3F))
                | (limbs[(offset >> 6) + 1] << (64 - (offset & 0x3F))))
                & ((1 << count) - 1)) as u32
        }
    }

    pub(super) fn decompose_128(&self) -> (Self, Self) {
        let words = self.0.as_words();

        let r1 = U256::from_words([words[0], words[1], 0, 0]);
        let r2 = U256::from_words([words[2], words[3], 0, 0]);

        (Self(r1), Self(r2))
    }

    pub(super) fn decompose(self) -> (Self, Self) {
        let mut c1 = self;
        c1.mul_shift_384_vartime(&Self::G1);
        let mut c2 = self;
        c2.mul_shift_384_vartime(&Self::G2);

        c1.mul_in_place(&Self::MINUS_B1);
        c2.mul_in_place(&Self::MINUS_B2);

        c1.add_in_place(&c2);

        let mut r1 = c1;
        r1.mul_in_place(&Self::MINUS_LAMBDA);
        r1.add_in_place(&self);

        (r1, c1)
    }

    pub(super) fn eq_mod(&self, other: &Self, modulus: &Self) -> bool {
        let (order, _) =
            crate::k256::elliptic_curve::bigint::NonZero::<crate::k256::U256>::const_new(modulus.0);
        let a = self.0.rem(&order);
        let b = other.0.rem(&order);

        a == b
    }

    pub(super) fn mul_shift_384_vartime(&mut self, b: &Self) {
        let mut b = *b;
        self.mul_wide(&mut b);

        let words = b.0.as_words();
        let l = words[1];
        self.0 = U256::from_words([words[2], words[3], 0, 0]);

        if (l >> 63) & 1 != 0 {
            self.add_in_place(&Self::ONE);
        }
    }

    pub(super) fn mul_in_place(&mut self, rhs: &Self) {
        let rhs = *rhs;
        self.mul_inner(rhs);
    }

    pub(super) fn square_in_place(&mut self) {
        let rhs = *self;
        self.mul_inner(rhs);
    }

    pub(super) fn add_in_place(&mut self, rhs: &Self) {
        self.0 = self.0.add_mod(&rhs.0, &Self::ORDER.0);
    }

    pub(super) fn negate_in_place(&mut self) {
        self.0 = self.0.neg_mod(&Self::ORDER.0);
    }

    pub(super) fn is_zero(&self) -> bool {
        self.eq_mod(&ScalarInner::ZERO, &ScalarInner::ORDER)
    }

    fn mul_inner(&mut self, mut rhs: Self) {
        const MODULUS: [u64; 4] = ScalarInner::ORDER.0.to_words();
        const NEG_MODULUS: [u64; 4] = [!MODULUS[0] + 1, !MODULUS[1], !MODULUS[2], !MODULUS[3]];

        self.mul_wide(&mut rhs);

        let w = self.0.as_words();
        let n = rhs.0.as_words();
        let n0 = n[0];
        let n1 = n[1];
        let n2 = n[2];
        let n3 = n[3];

        // Reduce 512 bits into 385.
        // m[0..6] = self[0..3] + n[0..3] * neg_modulus.
        let c0 = w[0];
        let c1 = 0;
        let c2 = 0;
        let (c0, c1) = muladd_fast(n0, NEG_MODULUS[0], c0, c1);
        let (m0, c0, c1) = (c0, c1, 0);
        let (c0, c1) = sumadd_fast(w[1], c0, c1);
        let (c0, c1, c2) = muladd(n1, NEG_MODULUS[0], c0, c1, c2);
        let (c0, c1, c2) = muladd(n0, NEG_MODULUS[1], c0, c1, c2);
        let (m1, c0, c1, c2) = (c0, c1, c2, 0);
        let (c0, c1, c2) = sumadd(w[2], c0, c1, c2);
        let (c0, c1, c2) = muladd(n2, NEG_MODULUS[0], c0, c1, c2);
        let (c0, c1, c2) = muladd(n1, NEG_MODULUS[1], c0, c1, c2);
        let (c0, c1, c2) = sumadd(n0, c0, c1, c2);
        let (m2, c0, c1, c2) = (c0, c1, c2, 0);
        let (c0, c1, c2) = sumadd(w[3], c0, c1, c2);
        let (c0, c1, c2) = muladd(n3, NEG_MODULUS[0], c0, c1, c2);
        let (c0, c1, c2) = muladd(n2, NEG_MODULUS[1], c0, c1, c2);
        let (c0, c1, c2) = sumadd(n1, c0, c1, c2);
        let (m3, c0, c1, c2) = (c0, c1, c2, 0);
        let (c0, c1, c2) = muladd(n3, NEG_MODULUS[1], c0, c1, c2);
        let (c0, c1, c2) = sumadd(n2, c0, c1, c2);
        let (m4, c0, c1, _c2) = (c0, c1, c2, 0);
        let (c0, c1) = sumadd_fast(n3, c0, c1);
        let (m5, c0, _c1) = (c0, c1, 0);
        debug_assert!(c0 <= 1);
        let m6 = c0;

        // Reduce 385 bits into 258.
        // p[0..4] = m[0..3] + m[4..6] * neg_modulus.
        let c0 = m0;
        let c1 = 0;
        let c2 = 0;
        let (c0, c1) = muladd_fast(m4, NEG_MODULUS[0], c0, c1);
        let (p0, c0, c1) = (c0, c1, 0);
        let (c0, c1) = sumadd_fast(m1, c0, c1);
        let (c0, c1, c2) = muladd(m5, NEG_MODULUS[0], c0, c1, c2);
        let (c0, c1, c2) = muladd(m4, NEG_MODULUS[1], c0, c1, c2);
        let (p1, c0, c1) = (c0, c1, 0);
        let (c0, c1, c2) = sumadd(m2, c0, c1, c2);
        let (c0, c1, c2) = muladd(m6, NEG_MODULUS[0], c0, c1, c2);
        let (c0, c1, c2) = muladd(m5, NEG_MODULUS[1], c0, c1, c2);
        let (c0, c1, c2) = sumadd(m4, c0, c1, c2);
        let (p2, c0, c1, _c2) = (c0, c1, c2, 0);
        let (c0, c1) = sumadd_fast(m3, c0, c1);
        let (c0, c1) = muladd_fast(m6, NEG_MODULUS[1], c0, c1);
        let (c0, c1) = sumadd_fast(m5, c0, c1);
        let (p3, c0, _c1) = (c0, c1, 0);
        let p4 = c0 + m6;
        debug_assert!(p4 <= 2);

        // Reduce 258 bits into 256.
        // r[0..3] = p[0..3] + p[4] * neg_modulus.
        let mut c = (p0 as u128) + (NEG_MODULUS[0] as u128) * (p4 as u128);
        let r0 = (c & 0xFFFFFFFFFFFFFFFFu128) as u64;
        c >>= 64;
        c += (p1 as u128) + (NEG_MODULUS[1] as u128) * (p4 as u128);
        let r1 = (c & 0xFFFFFFFFFFFFFFFFu128) as u64;
        c >>= 64;
        c += (p2 as u128) + (p4 as u128);
        let r2 = (c & 0xFFFFFFFFFFFFFFFFu128) as u64;
        c >>= 64;
        c += p3 as u128;
        let r3 = (c & 0xFFFFFFFFFFFFFFFFu128) as u64;
        c >>= 64;

        let r = U256::from_words([r0, r1, r2, r3]);
        let (r2, underflow) = r.sbb(&ScalarInner::ORDER.0, Limb::ZERO);
        let high_bit = c as u8;
        let underflow = (underflow.0 >> 63) as u8;

        match (1u8 & !underflow) | high_bit {
            0 => self.0 = r,
            1 => self.0 = r2,
            _ => panic!(),
        }
    }

    // stores lower bits in self, higher in rhs
    fn mul_wide(&mut self, rhs: &mut Self) {
        let a = self.0.as_words();
        let b = rhs.0.as_words();

        // 160 bit accumulator.
        let c0 = 0;
        let c1 = 0;
        let c2 = 0;

        // l[0..7] = a[0..3] * b[0..3].
        let (c0, c1) = muladd_fast(a[0], b[0], c0, c1);
        let (l0, c0, c1) = (c0, c1, 0);
        let (c0, c1, c2) = muladd(a[0], b[1], c0, c1, c2);
        let (c0, c1, c2) = muladd(a[1], b[0], c0, c1, c2);
        let (l1, c0, c1, c2) = (c0, c1, c2, 0);
        let (c0, c1, c2) = muladd(a[0], b[2], c0, c1, c2);
        let (c0, c1, c2) = muladd(a[1], b[1], c0, c1, c2);
        let (c0, c1, c2) = muladd(a[2], b[0], c0, c1, c2);
        let (l2, c0, c1, c2) = (c0, c1, c2, 0);
        let (c0, c1, c2) = muladd(a[0], b[3], c0, c1, c2);
        let (c0, c1, c2) = muladd(a[1], b[2], c0, c1, c2);
        let (c0, c1, c2) = muladd(a[2], b[1], c0, c1, c2);
        let (c0, c1, c2) = muladd(a[3], b[0], c0, c1, c2);
        let (l3, c0, c1, c2) = (c0, c1, c2, 0);
        let (c0, c1, c2) = muladd(a[1], b[3], c0, c1, c2);
        let (c0, c1, c2) = muladd(a[2], b[2], c0, c1, c2);
        let (c0, c1, c2) = muladd(a[3], b[1], c0, c1, c2);
        let (l4, c0, c1, c2) = (c0, c1, c2, 0);
        let (c0, c1, c2) = muladd(a[2], b[3], c0, c1, c2);
        let (c0, c1, c2) = muladd(a[3], b[2], c0, c1, c2);
        let (l5, c0, c1, _c2) = (c0, c1, c2, 0);
        let (c0, c1) = muladd_fast(a[3], b[3], c0, c1);
        let (l6, c0, _c1) = (c0, c1, 0);
        let l7 = c0;

        self.0 = U256::from_words([l0, l1, l2, l3]);
        rhs.0 = U256::from_words([l4, l5, l6, l7]);
    }
}

/// Add a*b to the number defined by (c0,c1,c2). c2 must never overflow.
#[inline(always)]
fn muladd(a: u64, b: u64, c0: u64, c1: u64, c2: u64) -> (u64, u64, u64) {
    let t = (a as u128) * (b as u128);
    let th = (t >> 64) as u64; // at most 0xFFFFFFFFFFFFFFFE
    let tl = t as u64;

    let (new_c0, carry0) = c0.overflowing_add(tl);
    let new_th = th.wrapping_add(carry0 as u64); // at most 0xFFFFFFFFFFFFFFFF
    let (new_c1, carry1) = c1.overflowing_add(new_th);
    let new_c2 = c2 + (carry1 as u64);

    (new_c0, new_c1, new_c2)
}

/// Add a*b to the number defined by (c0,c1). c1 must never overflow.
#[inline(always)]
fn muladd_fast(a: u64, b: u64, c0: u64, c1: u64) -> (u64, u64) {
    let t = (a as u128) * (b as u128);
    let th = (t >> 64) as u64; // at most 0xFFFFFFFFFFFFFFFE
    let tl = t as u64;

    let (new_c0, carry0) = c0.overflowing_add(tl);
    let new_th = th.wrapping_add(carry0 as u64); // at most 0xFFFFFFFFFFFFFFFF
    let new_c1 = c1 + new_th;

    (new_c0, new_c1)
}

/// Add a to the number defined by (c0,c1). c1 must never overflow.
fn sumadd_fast(a: u64, c0: u64, c1: u64) -> (u64, u64) {
    let (new_c0, carry0) = c0.overflowing_add(a);
    let new_c1 = c1 + (carry0 as u64);
    (new_c0, new_c1)
}

/// Add a to the number defined by (c0,c1,c2). c2 must never overflow.
fn sumadd(a: u64, c0: u64, c1: u64, c2: u64) -> (u64, u64, u64) {
    let (new_c0, carry0) = c0.overflowing_add(a);
    let (new_c1, carry1) = c1.overflowing_add(carry0 as u64);
    let new_c2 = c2 + (carry1 as u64);
    (new_c0, new_c1, new_c2)
}

#[cfg(test)]
impl proptest::arbitrary::Arbitrary for ScalarInner {
    type Parameters = ();

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        use proptest::prelude::{any, Strategy};

        any::<[u64; 4]>().prop_map(|words| {
            let (order, _) =
                crate::k256::elliptic_curve::bigint::NonZero::<U256>::const_new(Self::ORDER.0);
            let u256 = U256::from_words(words).rem(&order);
            Self(u256)
        })
    }

    type Strategy = proptest::arbitrary::Mapped<[u64; 4], Self>;
}

#[cfg(test)]
impl PartialEq for ScalarInner {
    fn eq(&self, other: &Self) -> bool {
        self.eq_mod(other, &Self::ORDER)
    }
}

#[cfg(test)]
impl PartialOrd for ScalarInner {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.0.partial_cmp(&other.0)
    }
}
