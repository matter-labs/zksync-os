#![allow(dead_code)]
// most of the code in this file comes from https://github.com/RustCrypto/elliptic-curves/blob/master/k256/src/arithmetic/field/field_10x26.rs

use crate::k256::FieldBytes;

use super::mod_inv32::{ModInfo, Signed30};

const MOD_INFO: ModInfo = ModInfo::new([-0x3D1, -4, 0, 0, 0, 0, 0, 0, 65536], 0x2DDACACF);

#[derive(Clone, Copy)]
pub struct FieldElement10x26(pub(super) [u32; 10]);

impl core::fmt::Debug for FieldElement10x26 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("0x")?;
        let bytes = self.to_bytes();
        for b in bytes.as_slice().iter() {
            f.write_fmt(format_args!("{:02x}", b))?;
        }

        core::fmt::Result::Ok(())
    }
}

impl FieldElement10x26 {
    pub(super) const ZERO: Self = Self([0; 10]);
    pub(super) const ONE: Self = Self([1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    pub(super) const BETA: Self = Self::from_bytes_unchecked(&[
        0x7a, 0xe9, 0x6a, 0x2b, 0x65, 0x7c, 0x07, 0x10, 0x6e, 0x64, 0x47, 0x9e, 0xac, 0x34, 0x34,
        0xe9, 0x9c, 0xf0, 0x49, 0x75, 0x12, 0xf5, 0x89, 0x95, 0xc1, 0x39, 0x6c, 0x28, 0x71, 0x95,
        0x01, 0xee,
    ]);

    // 2^256 mod P
    pub(super) const TWO_POW_256: Self = Self::from_bytes_unchecked(&[
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0,
        0x03, 0xd1,
    ]);

    #[inline(always)]
    pub(super) const fn from_bytes_unchecked(bytes: &[u8; 32]) -> Self {
        let w0 = (bytes[31] as u32)
            | ((bytes[30] as u32) << 8)
            | ((bytes[29] as u32) << 16)
            | (((bytes[28] & 0x3) as u32) << 24);
        let w1 = (((bytes[28] >> 2) as u32) & 0x3f)
            | ((bytes[27] as u32) << 6)
            | ((bytes[26] as u32) << 14)
            | (((bytes[25] & 0xf) as u32) << 22);
        let w2 = (((bytes[25] >> 4) as u32) & 0xf)
            | ((bytes[24] as u32) << 4)
            | ((bytes[23] as u32) << 12)
            | (((bytes[22] & 0x3f) as u32) << 20);
        let w3 = (((bytes[22] >> 6) as u32) & 0x3)
            | ((bytes[21] as u32) << 2)
            | ((bytes[20] as u32) << 10)
            | ((bytes[19] as u32) << 18);
        let w4 = (bytes[18] as u32)
            | ((bytes[17] as u32) << 8)
            | ((bytes[16] as u32) << 16)
            | (((bytes[15] & 0x3) as u32) << 24);
        let w5 = (((bytes[15] >> 2) as u32) & 0x3f)
            | ((bytes[14] as u32) << 6)
            | ((bytes[13] as u32) << 14)
            | (((bytes[12] & 0xf) as u32) << 22);
        let w6 = (((bytes[12] >> 4) as u32) & 0xf)
            | ((bytes[11] as u32) << 4)
            | ((bytes[10] as u32) << 12)
            | (((bytes[9] & 0x3f) as u32) << 20);
        let w7 = (((bytes[9] >> 6) as u32) & 0x3)
            | ((bytes[8] as u32) << 2)
            | ((bytes[7] as u32) << 10)
            | ((bytes[6] as u32) << 18);
        let w8 = (bytes[5] as u32)
            | ((bytes[4] as u32) << 8)
            | ((bytes[3] as u32) << 16)
            | (((bytes[2] & 0x3) as u32) << 24);
        let w9 = (((bytes[2] >> 2) as u32) & 0x3f)
            | ((bytes[1] as u32) << 6)
            | ((bytes[0] as u32) << 14);

        Self([w0, w1, w2, w3, w4, w5, w6, w7, w8, w9])
    }

    #[inline(always)]
    pub(super) fn from_bytes(bytes: &[u8; 32]) -> Option<Self> {
        let val = Self::from_bytes_unchecked(bytes);
        if val.overflow() {
            None
        } else {
            Some(val)
        }
    }

    #[inline(always)]
    pub(super) const fn max_magnitude() -> u32 {
        31
    }

    #[inline(always)]
    const fn overflow(&self) -> bool {
        let m = self.0[2] & self.0[3] & self.0[4] & self.0[5] & self.0[6] & self.0[7] & self.0[8];
        (self.0[9] >> 22 != 0)
            | ((self.0[9] == 0x3FFFFFu32)
                & (m == 0x3FFFFFFu32)
                & ((self.0[1] + 0x40u32 + ((self.0[0] + 0x3D1u32) >> 26)) > 0x3FFFFFFu32))
    }

    #[inline(always)]
    pub(super) fn to_bytes(self) -> FieldBytes {
        let mut r = FieldBytes::default();
        r[0] = (self.0[9] >> 14) as u8;
        r[1] = (self.0[9] >> 6) as u8;
        r[2] = ((self.0[9] as u8 & 0x3Fu8) << 2) | ((self.0[8] >> 24) as u8 & 0x3);
        r[3] = (self.0[8] >> 16) as u8;
        r[4] = (self.0[8] >> 8) as u8;
        r[5] = self.0[8] as u8;
        r[6] = (self.0[7] >> 18) as u8;
        r[7] = (self.0[7] >> 10) as u8;
        r[8] = (self.0[7] >> 2) as u8;
        r[9] = ((self.0[7] as u8 & 0x3u8) << 6) | ((self.0[6] >> 20) as u8 & 0x3fu8);
        r[10] = (self.0[6] >> 12) as u8;
        r[11] = (self.0[6] >> 4) as u8;
        r[12] = ((self.0[6] as u8 & 0xfu8) << 4) | ((self.0[5] >> 22) as u8 & 0xfu8);
        r[13] = (self.0[5] >> 14) as u8;
        r[14] = (self.0[5] >> 6) as u8;
        r[15] = ((self.0[5] as u8 & 0x3fu8) << 2) | ((self.0[4] >> 24) as u8 & 0x3u8);
        r[16] = (self.0[4] >> 16) as u8;
        r[17] = (self.0[4] >> 8) as u8;
        r[18] = self.0[4] as u8;
        r[19] = (self.0[3] >> 18) as u8;
        r[20] = (self.0[3] >> 10) as u8;
        r[21] = (self.0[3] >> 2) as u8;
        r[22] = ((self.0[3] as u8 & 0x3u8) << 6) | ((self.0[2] >> 20) as u8 & 0x3fu8);
        r[23] = (self.0[2] >> 12) as u8;
        r[24] = (self.0[2] >> 4) as u8;
        r[25] = ((self.0[2] as u8 & 0xfu8) << 4) | ((self.0[1] >> 22) as u8 & 0xfu8);
        r[26] = (self.0[1] >> 14) as u8;
        r[27] = (self.0[1] >> 6) as u8;
        r[28] = ((self.0[1] as u8 & 0x3fu8) << 2) | ((self.0[0] >> 24) as u8 & 0x3u8);
        r[29] = (self.0[0] >> 16) as u8;
        r[30] = (self.0[0] >> 8) as u8;
        r[31] = self.0[0] as u8;
        r
    }

    #[inline(always)]
    pub(super) const fn mul(&self, rhs: &Self) -> Self {
        let mut ret = *self;
        ret.mul_in_place(rhs);
        ret
    }

    #[inline(always)]
    pub(super) const fn mul_in_place(&mut self, rhs: &Self) {
        let m = 0x3FFFFFFu64;
        let rr0 = 0x3D10u64;
        let rr1 = 0x400u64;

        let a0 = self.0[0] as u64;
        let a1 = self.0[1] as u64;
        let a2 = self.0[2] as u64;
        let a3 = self.0[3] as u64;
        let a4 = self.0[4] as u64;
        let a5 = self.0[5] as u64;
        let a6 = self.0[6] as u64;
        let a7 = self.0[7] as u64;
        let a8 = self.0[8] as u64;
        let a9 = self.0[9] as u64;

        let b0 = rhs.0[0] as u64;
        let b1 = rhs.0[1] as u64;
        let b2 = rhs.0[2] as u64;
        let b3 = rhs.0[3] as u64;
        let b4 = rhs.0[4] as u64;
        let b5 = rhs.0[5] as u64;
        let b6 = rhs.0[6] as u64;
        let b7 = rhs.0[7] as u64;
        let b8 = rhs.0[8] as u64;
        let b9 = rhs.0[9] as u64;

        // [... a b c] is a shorthand for ... + a<<52 + b<<26 + c<<0 mod n.
        // for 0 <= x <= 9, px is a shorthand for sum(a[i]*b[x-i], i=0..x).
        // for 9 <= x <= 18, px is a shorthand for sum(a[i]*b[x-i], i=(x-9)..9)
        // Note that [x 0 0 0 0 0 0 0 0 0 0] = [x*rr1 x*rr0].

        let mut c: u64;
        let mut d: u64;

        d = a0 * b9
            + a1 * b8
            + a2 * b7
            + a3 * b6
            + a4 * b5
            + a5 * b4
            + a6 * b3
            + a7 * b2
            + a8 * b1
            + a9 * b0;
        // [d 0 0 0 0 0 0 0 0 0] = [p9 0 0 0 0 0 0 0 0 0]
        let t9 = (d & m) as u32;
        d >>= 26;
        debug_assert!(t9 >> 26 == 0);
        debug_assert!(d >> 38 == 0);
        // [d t9 0 0 0 0 0 0 0 0 0] = [p9 0 0 0 0 0 0 0 0 0]

        c = a0 * b0;
        debug_assert!(c >> 60 == 0);
        // [d t9 0 0 0 0 0 0 0 0 c] = [p9 0 0 0 0 0 0 0 0 p0]
        d +=
            a1 * b9 + a2 * b8 + a3 * b7 + a4 * b6 + a5 * b5 + a6 * b4 + a7 * b3 + a8 * b2 + a9 * b1;
        debug_assert!(d >> 63 == 0);
        // [d t9 0 0 0 0 0 0 0 0 c] = [p10 p9 0 0 0 0 0 0 0 0 p0]
        let u0 = (d & m) as u32;
        d >>= 26;
        c += u0 as u64 * rr0;
        debug_assert!(u0 >> 26 == 0);
        debug_assert!(d >> 37 == 0);
        debug_assert!(c >> 61 == 0);
        // [d u0 t9 0 0 0 0 0 0 0 0 c-u0*rr0] = [p10 p9 0 0 0 0 0 0 0 0 p0]
        let t0 = (c & m) as u32;
        c >>= 26;
        c += u0 as u64 * rr1;
        debug_assert!(t0 >> 26 == 0);
        debug_assert!(c >> 37 == 0);
        // [d u0 t9 0 0 0 0 0 0 0 c-u0*rr1 t0-u0*rr0] = [p10 p9 0 0 0 0 0 0 0 0 p0]
        // [d 0 t9 0 0 0 0 0 0 0 c t0] = [p10 p9 0 0 0 0 0 0 0 0 p0]

        c += a0 * b1 + a1 * b0;
        debug_assert!(c >> 62 == 0);
        // [d 0 t9 0 0 0 0 0 0 0 c t0] = [p10 p9 0 0 0 0 0 0 0 p1 p0]
        d += a2 * b9 + a3 * b8 + a4 * b7 + a5 * b6 + a6 * b5 + a7 * b4 + a8 * b3 + a9 * b2;
        debug_assert!(d >> 63 == 0);
        // [d 0 t9 0 0 0 0 0 0 0 c t0] = [p11 p10 p9 0 0 0 0 0 0 0 p1 p0]
        let u1 = (d & m) as u32;
        d >>= 26;
        c += u1 as u64 * rr0;
        debug_assert!(u1 >> 26 == 0);
        debug_assert!(d >> 37 == 0);
        debug_assert!(c >> 63 == 0);
        // [d u1 0 t9 0 0 0 0 0 0 0 c-u1*rr0 t0] = [p11 p10 p9 0 0 0 0 0 0 0 p1 p0]
        let t1 = (c & m) as u32;
        c >>= 26;
        c += u1 as u64 * rr1;
        debug_assert!(t1 >> 26 == 0);
        debug_assert!(c >> 38 == 0);
        // [d u1 0 t9 0 0 0 0 0 0 c-u1*rr1 t1-u1*rr0 t0] = [p11 p10 p9 0 0 0 0 0 0 0 p1 p0]
        // [d 0 0 t9 0 0 0 0 0 0 c t1 t0] = [p11 p10 p9 0 0 0 0 0 0 0 p1 p0]

        c += a0 * b2 + a1 * b1 + a2 * b0;
        debug_assert!(c >> 62 == 0);
        // [d 0 0 t9 0 0 0 0 0 0 c t1 t0] = [p11 p10 p9 0 0 0 0 0 0 p2 p1 p0]
        d += a3 * b9 + a4 * b8 + a5 * b7 + a6 * b6 + a7 * b5 + a8 * b4 + a9 * b3;
        debug_assert!(d >> 63 == 0);
        // [d 0 0 t9 0 0 0 0 0 0 c t1 t0] = [p12 p11 p10 p9 0 0 0 0 0 0 p2 p1 p0]
        let u2 = (d & m) as u32;
        d >>= 26;
        c += u2 as u64 * rr0;
        debug_assert!(u2 >> 26 == 0);
        debug_assert!(d >> 37 == 0);
        debug_assert!(c >> 63 == 0);
        // [d u2 0 0 t9 0 0 0 0 0 0 c-u2*rr0 t1 t0] = [p12 p11 p10 p9 0 0 0 0 0 0 p2 p1 p0]
        let t2 = (c & m) as u32;
        c >>= 26;
        c += u2 as u64 * rr1;
        debug_assert!(t2 >> 26 == 0);
        debug_assert!(c >> 38 == 0);
        // [d u2 0 0 t9 0 0 0 0 0 c-u2*rr1 t2-u2*rr0 t1 t0] = [p12 p11 p10 p9 0 0 0 0 0 0 p2 p1 p0]
        // [d 0 0 0 t9 0 0 0 0 0 c t2 t1 t0] = [p12 p11 p10 p9 0 0 0 0 0 0 p2 p1 p0]

        c += a0 * b3 + a1 * b2 + a2 * b1 + a3 * b0;
        debug_assert!(c >> 63 == 0);
        // [d 0 0 0 t9 0 0 0 0 0 c t2 t1 t0] = [p12 p11 p10 p9 0 0 0 0 0 p3 p2 p1 p0]
        d += a4 * b9 + a5 * b8 + a6 * b7 + a7 * b6 + a8 * b5 + a9 * b4;
        debug_assert!(d >> 63 == 0);
        // [d 0 0 0 t9 0 0 0 0 0 c t2 t1 t0] = [p13 p12 p11 p10 p9 0 0 0 0 0 p3 p2 p1 p0]
        let u3 = (d & m) as u32;
        d >>= 26;
        c += u3 as u64 * rr0;
        debug_assert!(u3 >> 26 == 0);
        debug_assert!(d >> 37 == 0);
        // [d u3 0 0 0 t9 0 0 0 0 0 c-u3*rr0 t2 t1 t0] = [p13 p12 p11 p10 p9 0 0 0 0 0 p3 p2 p1 p0]
        let t3 = (c & m) as u32;
        c >>= 26;
        c += u3 as u64 * rr1;
        debug_assert!(t3 >> 26 == 0);
        debug_assert!(c >> 39 == 0);
        // [d u3 0 0 0 t9 0 0 0 0 c-u3*rr1 t3-u3*rr0 t2 t1 t0] = [p13 p12 p11 p10 p9 0 0 0 0 0 p3 p2 p1 p0]
        // [d 0 0 0 0 t9 0 0 0 0 c t3 t2 t1 t0] = [p13 p12 p11 p10 p9 0 0 0 0 0 p3 p2 p1 p0]

        c += a0 * b4 + a1 * b3 + a2 * b2 + a3 * b1 + a4 * b0;
        debug_assert!(c >> 63 == 0);
        // [d 0 0 0 0 t9 0 0 0 0 c t3 t2 t1 t0] = [p13 p12 p11 p10 p9 0 0 0 0 p4 p3 p2 p1 p0]
        d += a5 * b9 + a6 * b8 + a7 * b7 + a8 * b6 + a9 * b5;
        debug_assert!(d >> 62 == 0);
        // [d 0 0 0 0 t9 0 0 0 0 c t3 t2 t1 t0] = [p14 p13 p12 p11 p10 p9 0 0 0 0 p4 p3 p2 p1 p0]
        let u4 = (d & m) as u32;
        d >>= 26;
        c += u4 as u64 * rr0;
        debug_assert!(u4 >> 26 == 0);
        debug_assert!(d >> 36 == 0);
        // [d u4 0 0 0 0 t9 0 0 0 0 c-u4*rr0 t3 t2 t1 t0] = [p14 p13 p12 p11 p10 p9 0 0 0 0 p4 p3 p2 p1 p0]
        let t4 = (c & m) as u32;
        c >>= 26;
        c += u4 as u64 * rr1;
        debug_assert!(t4 >> 26 == 0);
        debug_assert!(c >> 39 == 0);
        // [d u4 0 0 0 0 t9 0 0 0 c-u4*rr1 t4-u4*rr0 t3 t2 t1 t0] = [p14 p13 p12 p11 p10 p9 0 0 0 0 p4 p3 p2 p1 p0]
        // [d 0 0 0 0 0 t9 0 0 0 c t4 t3 t2 t1 t0] = [p14 p13 p12 p11 p10 p9 0 0 0 0 p4 p3 p2 p1 p0]

        c += a0 * b5 + a1 * b4 + a2 * b3 + a3 * b2 + a4 * b1 + a5 * b0;
        debug_assert!(c >> 63 == 0);
        // [d 0 0 0 0 0 t9 0 0 0 c t4 t3 t2 t1 t0] = [p14 p13 p12 p11 p10 p9 0 0 0 p5 p4 p3 p2 p1 p0]
        d += a6 * b9 + a7 * b8 + a8 * b7 + a9 * b6;
        debug_assert!(d >> 62 == 0);
        // [d 0 0 0 0 0 t9 0 0 0 c t4 t3 t2 t1 t0] = [p15 p14 p13 p12 p11 p10 p9 0 0 0 p5 p4 p3 p2 p1 p0]
        let u5 = (d & m) as u32;
        d >>= 26;
        c += u5 as u64 * rr0;
        debug_assert!(u5 >> 26 == 0);
        debug_assert!(d >> 36 == 0);
        // [d u5 0 0 0 0 0 t9 0 0 0 c-u5*rr0 t4 t3 t2 t1 t0] = [p15 p14 p13 p12 p11 p10 p9 0 0 0 p5 p4 p3 p2 p1 p0]
        let t5 = (c & m) as u32;
        c >>= 26;
        c += u5 as u64 * rr1;
        debug_assert!(t5 >> 26 == 0);
        debug_assert!(c >> 39 == 0);
        // [d u5 0 0 0 0 0 t9 0 0 c-u5*rr1 t5-u5*rr0 t4 t3 t2 t1 t0] = [p15 p14 p13 p12 p11 p10 p9 0 0 0 p5 p4 p3 p2 p1 p0]
        // [d 0 0 0 0 0 0 t9 0 0 c t5 t4 t3 t2 t1 t0] = [p15 p14 p13 p12 p11 p10 p9 0 0 0 p5 p4 p3 p2 p1 p0]

        c += a0 * b6 + a1 * b5 + a2 * b4 + a3 * b3 + a4 * b2 + a5 * b1 + a6 * b0;
        debug_assert!(c >> 63 == 0);
        // [d 0 0 0 0 0 0 t9 0 0 c t5 t4 t3 t2 t1 t0] = [p15 p14 p13 p12 p11 p10 p9 0 0 p6 p5 p4 p3 p2 p1 p0]
        d += a7 * b9 + a8 * b8 + a9 * b7;
        debug_assert!(d >> 61 == 0);
        // [d 0 0 0 0 0 0 t9 0 0 c t5 t4 t3 t2 t1 t0] = [p16 p15 p14 p13 p12 p11 p10 p9 0 0 p6 p5 p4 p3 p2 p1 p0]
        let u6 = (d & m) as u32;
        d >>= 26;
        c += u6 as u64 * rr0;
        debug_assert!(u6 >> 26 == 0);
        debug_assert!(d >> 35 == 0);
        // [d u6 0 0 0 0 0 0 t9 0 0 c-u6*rr0 t5 t4 t3 t2 t1 t0] = [p16 p15 p14 p13 p12 p11 p10 p9 0 0 p6 p5 p4 p3 p2 p1 p0]
        let t6 = (c & m) as u32;
        c >>= 26;
        c += u6 as u64 * rr1;
        debug_assert!(t6 >> 26 == 0);
        debug_assert!(c >> 39 == 0);
        // [d u6 0 0 0 0 0 0 t9 0 c-u6*rr1 t6-u6*rr0 t5 t4 t3 t2 t1 t0] = [p16 p15 p14 p13 p12 p11 p10 p9 0 0 p6 p5 p4 p3 p2 p1 p0]
        // [d 0 0 0 0 0 0 0 t9 0 c t6 t5 t4 t3 t2 t1 t0] = [p16 p15 p14 p13 p12 p11 p10 p9 0 0 p6 p5 p4 p3 p2 p1 p0]

        c += a0 * b7 + a1 * b6 + a2 * b5 + a3 * b4 + a4 * b3 + a5 * b2 + a6 * b1 + a7 * b0;
        debug_assert!(c <= 0x8000007C00000007u64);
        // [d 0 0 0 0 0 0 0 t9 0 c t6 t5 t4 t3 t2 t1 t0] = [p16 p15 p14 p13 p12 p11 p10 p9 0 p7 p6 p5 p4 p3 p2 p1 p0]
        d += a8 * b9 + a9 * b8;
        debug_assert!(d >> 58 == 0);
        // [d 0 0 0 0 0 0 0 t9 0 c t6 t5 t4 t3 t2 t1 t0] = [p17 p16 p15 p14 p13 p12 p11 p10 p9 0 p7 p6 p5 p4 p3 p2 p1 p0]
        let u7 = (d & m) as u32;
        d >>= 26;
        c += u7 as u64 * rr0;
        debug_assert!(u7 >> 26 == 0);
        debug_assert!(d >> 32 == 0);
        let d32 = d as u32;
        debug_assert!(c <= 0x800001703FFFC2F7u64);
        // [d u7 0 0 0 0 0 0 0 t9 0 c-u7*rr0 t6 t5 t4 t3 t2 t1 t0] = [p17 p16 p15 p14 p13 p12 p11 p10 p9 0 p7 p6 p5 p4 p3 p2 p1 p0]
        let t7 = (c & m) as u32;
        c >>= 26;
        c += u7 as u64 * rr1;
        debug_assert!(t7 >> 26 == 0);
        debug_assert!(c >> 38 == 0);
        // [d u7 0 0 0 0 0 0 0 t9 c-u7*rr1 t7-u7*rr0 t6 t5 t4 t3 t2 t1 t0] = [p17 p16 p15 p14 p13 p12 p11 p10 p9 0 p7 p6 p5 p4 p3 p2 p1 p0]
        // [d 0 0 0 0 0 0 0 0 t9 c t7 t6 t5 t4 t3 t2 t1 t0] = [p17 p16 p15 p14 p13 p12 p11 p10 p9 0 p7 p6 p5 p4 p3 p2 p1 p0]

        c +=
            a0 * b8 + a1 * b7 + a2 * b6 + a3 * b5 + a4 * b4 + a5 * b3 + a6 * b2 + a7 * b1 + a8 * b0;
        debug_assert!(c <= 0x9000007B80000008u64);
        // [d 0 0 0 0 0 0 0 0 t9 c t7 t6 t5 t4 t3 t2 t1 t0] = [p17 p16 p15 p14 p13 p12 p11 p10 p9 p8 p7 p6 p5 p4 p3 p2 p1 p0]
        d = d32 as u64 + a9 * b9;
        debug_assert!(d >> 57 == 0);
        // [d 0 0 0 0 0 0 0 0 t9 c t7 t6 t5 t4 t3 t2 t1 t0] = [p18 p17 p16 p15 p14 p13 p12 p11 p10 p9 p8 p7 p6 p5 p4 p3 p2 p1 p0]
        let u8 = (d & m) as u32;
        d >>= 26;
        c += u8 as u64 * rr0;
        debug_assert!(u8 >> 26 == 0);
        debug_assert!(d >> 31 == 0);
        let d32 = d as u32;
        debug_assert!(c <= 0x9000016FBFFFC2F8u64);
        // [d u8 0 0 0 0 0 0 0 0 t9 c-u8*rr0 t7 t6 t5 t4 t3 t2 t1 t0] = [p18 p17 p16 p15 p14 p13 p12 p11 p10 p9 p8 p7 p6 p5 p4 p3 p2 p1 p0]

        let r3 = t3;
        debug_assert!(r3 >> 26 == 0);
        // [d u8 0 0 0 0 0 0 0 0 t9 c-u8*rr0 t7 t6 t5 t4 r3 t2 t1 t0] = [p18 p17 p16 p15 p14 p13 p12 p11 p10 p9 p8 p7 p6 p5 p4 p3 p2 p1 p0]
        let r4 = t4;
        debug_assert!(r4 >> 26 == 0);
        // [d u8 0 0 0 0 0 0 0 0 t9 c-u8*rr0 t7 t6 t5 r4 r3 t2 t1 t0] = [p18 p17 p16 p15 p14 p13 p12 p11 p10 p9 p8 p7 p6 p5 p4 p3 p2 p1 p0]
        let r5 = t5;
        debug_assert!(r5 >> 26 == 0);
        // [d u8 0 0 0 0 0 0 0 0 t9 c-u8*rr0 t7 t6 r5 r4 r3 t2 t1 t0] = [p18 p17 p16 p15 p14 p13 p12 p11 p10 p9 p8 p7 p6 p5 p4 p3 p2 p1 p0]
        let r6 = t6;
        debug_assert!(r6 >> 26 == 0);
        // [d u8 0 0 0 0 0 0 0 0 t9 c-u8*rr0 t7 r6 r5 r4 r3 t2 t1 t0] = [p18 p17 p16 p15 p14 p13 p12 p11 p10 p9 p8 p7 p6 p5 p4 p3 p2 p1 p0]
        let r7 = t7;
        debug_assert!(r7 >> 26 == 0);
        // [d u8 0 0 0 0 0 0 0 0 t9 c-u8*rr0 r7 r6 r5 r4 r3 t2 t1 t0] = [p18 p17 p16 p15 p14 p13 p12 p11 p10 p9 p8 p7 p6 p5 p4 p3 p2 p1 p0]

        let r8 = (c & m) as u32;
        c >>= 26;
        c += u8 as u64 * rr1;
        debug_assert!(r8 >> 26 == 0);
        debug_assert!(c >> 39 == 0);
        // [d u8 0 0 0 0 0 0 0 0 t9+c-u8*rr1 r8-u8*rr0 r7 r6 r5 r4 r3 t2 t1 t0] = [p18 p17 p16 p15 p14 p13 p12 p11 p10 p9 p8 p7 p6 p5 p4 p3 p2 p1 p0]
        // [d 0 0 0 0 0 0 0 0 0 t9+c r8 r7 r6 r5 r4 r3 t2 t1 t0] = [p18 p17 p16 p15 p14 p13 p12 p11 p10 p9 p8 p7 p6 p5 p4 p3 p2 p1 p0]
        c += d32 as u64 * rr0 + t9 as u64;
        debug_assert!(c >> 45 == 0);
        // [d 0 0 0 0 0 0 0 0 0 c-d*rr0 r8 r7 r6 r5 r4 r3 t2 t1 t0] = [p18 p17 p16 p15 p14 p13 p12 p11 p10 p9 p8 p7 p6 p5 p4 p3 p2 p1 p0]
        let r9 = (c & (m >> 4)) as u32;
        c >>= 22;
        c += d * (rr1 << 4);
        debug_assert!(r9 >> 22 == 0);
        debug_assert!(c >> 46 == 0);
        // [d 0 0 0 0 0 0 0 0 r9+((c-d*rr1<<4)<<22)-d*rr0 r8 r7 r6 r5 r4 r3 t2 t1 t0] = [p18 p17 p16 p15 p14 p13 p12 p11 p10 p9 p8 p7 p6 p5 p4 p3 p2 p1 p0]
        // [d 0 0 0 0 0 0 0 -d*rr1 r9+(c<<22)-d*rr0 r8 r7 r6 r5 r4 r3 t2 t1 t0] = [p18 p17 p16 p15 p14 p13 p12 p11 p10 p9 p8 p7 p6 p5 p4 p3 p2 p1 p0]
        // [r9+(c<<22) r8 r7 r6 r5 r4 r3 t2 t1 t0] = [p18 p17 p16 p15 p14 p13 p12 p11 p10 p9 p8 p7 p6 p5 p4 p3 p2 p1 p0]

        d = c * (rr0 >> 4) + t0 as u64;
        debug_assert!(d >> 56 == 0);
        // [r9+(c<<22) r8 r7 r6 r5 r4 r3 t2 t1 d-c*rr0>>4] = [p18 p17 p16 p15 p14 p13 p12 p11 p10 p9 p8 p7 p6 p5 p4 p3 p2 p1 p0]
        let r0 = (d & m) as u32;
        d >>= 26;
        debug_assert!(r0 >> 26 == 0);
        debug_assert!(d >> 30 == 0);
        let d32 = d as u32;
        // [r9+(c<<22) r8 r7 r6 r5 r4 r3 t2 t1+d r0-c*rr0>>4] = [p18 p17 p16 p15 p14 p13 p12 p11 p10 p9 p8 p7 p6 p5 p4 p3 p2 p1 p0]
        d = d32 as u64 + c * (rr1 >> 4) + t1 as u64;
        debug_assert!(d >> 53 == 0);
        debug_assert!(d <= 0x10000003FFFFBFu64);
        // [r9+(c<<22) r8 r7 r6 r5 r4 r3 t2 d-c*rr1>>4 r0-c*rr0>>4] = [p18 p17 p16 p15 p14 p13 p12 p11 p10 p9 p8 p7 p6 p5 p4 p3 p2 p1 p0]
        // [r9 r8 r7 r6 r5 r4 r3 t2 d r0] = [p18 p17 p16 p15 p14 p13 p12 p11 p10 p9 p8 p7 p6 p5 p4 p3 p2 p1 p0]
        let r1 = (d & m) as u32;
        d >>= 26;
        debug_assert!(r1 >> 26 == 0);
        debug_assert!(d >> 27 == 0);
        let d32 = d as u32;
        debug_assert!(d <= 0x4000000u64);
        // [r9 r8 r7 r6 r5 r4 r3 t2+d r1 r0] = [p18 p17 p16 p15 p14 p13 p12 p11 p10 p9 p8 p7 p6 p5 p4 p3 p2 p1 p0]
        d = d32 as u64 + t2 as u64;
        debug_assert!(d >> 27 == 0);
        // [r9 r8 r7 r6 r5 r4 r3 d r1 r0] = [p18 p17 p16 p15 p14 p13 p12 p11 p10 p9 p8 p7 p6 p5 p4 p3 p2 p1 p0]
        let r2 = d as u32;
        debug_assert!(r2 >> 27 == 0);
        // [r9 r8 r7 r6 r5 r4 r3 r2 r1 r0] = [p18 p17 p16 p15 p14 p13 p12 p11 p10 p9 p8 p7 p6 p5 p4 p3 p2 p1 p0]

        *self = Self([r0, r1, r2, r3, r4, r5, r6, r7, r8, r9]);
    }

    #[inline(always)]
    pub(super) const fn mul_int(&self, rhs: u32) -> Self {
        let mut ret = *self;
        ret.mul_int_in_place(rhs);
        ret
    }

    #[inline(always)]
    pub(super) const fn mul_int_in_place(&mut self, rhs: u32) {
        self.0[0] *= rhs;
        self.0[1] *= rhs;
        self.0[2] *= rhs;
        self.0[3] *= rhs;
        self.0[4] *= rhs;
        self.0[5] *= rhs;
        self.0[6] *= rhs;
        self.0[7] *= rhs;
        self.0[8] *= rhs;
        self.0[9] *= rhs;
    }

    #[inline(always)]
    pub(super) const fn square(&self) -> Self {
        let mut ret = *self;
        ret.mul_in_place(self);
        ret
    }

    #[inline(always)]
    pub(super) fn square_in_place(&mut self) {
        let rhs = *self;
        self.mul_in_place(&rhs);
    }

    #[inline(always)]
    pub(super) const fn add(&self, rhs: &Self) -> Self {
        let mut ret = *self;
        ret.add_in_place(rhs);
        ret
    }

    #[inline(always)]
    pub(super) const fn add_in_place(&mut self, rhs: &Self) {
        self.0[0] += rhs.0[0];
        self.0[1] += rhs.0[1];
        self.0[2] += rhs.0[2];
        self.0[3] += rhs.0[3];
        self.0[4] += rhs.0[4];
        self.0[5] += rhs.0[5];
        self.0[6] += rhs.0[6];
        self.0[7] += rhs.0[7];
        self.0[8] += rhs.0[8];
        self.0[9] += rhs.0[9];
    }

    #[inline(always)]
    pub(super) const fn double_in_place(&mut self) {
        let rhs = *self;
        self.add_in_place(&rhs);
    }
    #[inline(always)]
    pub(super) fn add_int_in_place(&mut self, rhs: u32) {
        self.0[0] += rhs;
    }

    #[inline(always)]
    pub(super) const fn negate(&self, magnitude: u32) -> Self {
        let mut ret = *self;
        ret.negate_in_place(magnitude);
        ret
    }

    #[inline(always)]
    pub(super) const fn negate_in_place(&mut self, magnitude: u32) {
        let m: u32 = magnitude + 1;
        self.0[0] = 0x3FFFC2Fu32 * 2 * m - self.0[0];
        self.0[1] = 0x3FFFFBFu32 * 2 * m - self.0[1];
        self.0[2] = 0x3FFFFFFu32 * 2 * m - self.0[2];
        self.0[3] = 0x3FFFFFFu32 * 2 * m - self.0[3];
        self.0[4] = 0x3FFFFFFu32 * 2 * m - self.0[4];
        self.0[5] = 0x3FFFFFFu32 * 2 * m - self.0[5];
        self.0[6] = 0x3FFFFFFu32 * 2 * m - self.0[6];
        self.0[7] = 0x3FFFFFFu32 * 2 * m - self.0[7];
        self.0[8] = 0x3FFFFFFu32 * 2 * m - self.0[8];
        self.0[9] = 0x03FFFFFu32 * 2 * m - self.0[9];
    }

    #[inline(always)]
    pub(super) fn sub_in_place(&mut self, rhs: &Self) {
        let mut rhs = *rhs;
        rhs.normalize_in_place();
        rhs.negate_in_place(1);
        self.add_in_place(&rhs);
    }

    #[inline(always)]
    pub(super) const fn normalize(&self) -> Self {
        let mut ret = *self;
        ret.normalize_in_place();
        ret
    }

    #[inline(always)]
    pub(super) const fn normalize_in_place(&mut self) {
        // Reduce self.0[9] at the start so there will be at most a single carry from the first pass
        let mut x = self.0[9] >> 22;
        self.0[9] &= 0x03FFFFF;

        // The first pass ensures the magnitude is 1, ...
        self.0[0] += x * 0x3D1;
        self.0[1] += x << 6;
        self.0[1] += self.0[0] >> 26;
        self.0[0] &= 0x3FFFFFF;
        self.0[2] += self.0[1] >> 26;
        self.0[1] &= 0x3FFFFFF;
        self.0[3] += self.0[2] >> 26;
        self.0[2] &= 0x3FFFFFF;
        let mut m = self.0[2];
        self.0[4] += self.0[3] >> 26;
        self.0[3] &= 0x3FFFFFF;
        m &= self.0[3];
        self.0[5] += self.0[4] >> 26;
        self.0[4] &= 0x3FFFFFF;
        m &= self.0[4];
        self.0[6] += self.0[5] >> 26;
        self.0[5] &= 0x3FFFFFF;
        m &= self.0[5];
        self.0[7] += self.0[6] >> 26;
        self.0[6] &= 0x3FFFFFF;
        m &= self.0[6];
        self.0[8] += self.0[7] >> 26;
        self.0[7] &= 0x3FFFFFF;
        m &= self.0[7];
        self.0[9] += self.0[8] >> 26;
        self.0[8] &= 0x3FFFFFF;
        m &= self.0[8];

        /* ... except for a possible carry at bit 22 of self.0[9] (i.e. bit 256 of the field element) */
        debug_assert!(self.0[9] >> 23 == 0);

        // At most a single final reduction is needed; check if the value is >= the field characteristic
        x = (self.0[9] >> 22)
            | ((self.0[9] == 0x03FFFFF)
                & (m == 0x3FFFFFF)
                & ((self.0[1] + 0x40 + ((self.0[0] + 0x3D1) >> 26)) > 0x3FFFFFF))
                as u32;

        if x != 0 {
            self.0[0] += 0x3D1;
            self.0[1] += x << 6;
            self.0[1] += self.0[0] >> 26;
            self.0[0] &= 0x3FFFFFF;
            self.0[2] += self.0[1] >> 26;
            self.0[1] &= 0x3FFFFFF;
            self.0[3] += self.0[2] >> 26;
            self.0[2] &= 0x3FFFFFF;
            self.0[4] += self.0[3] >> 26;
            self.0[3] &= 0x3FFFFFF;
            self.0[5] += self.0[4] >> 26;
            self.0[4] &= 0x3FFFFFF;
            self.0[6] += self.0[5] >> 26;
            self.0[5] &= 0x3FFFFFF;
            self.0[7] += self.0[6] >> 26;
            self.0[6] &= 0x3FFFFFF;
            self.0[8] += self.0[7] >> 26;
            self.0[7] &= 0x3FFFFFF;
            self.0[9] += self.0[8] >> 26;
            self.0[8] &= 0x3FFFFFF;

            // If self.0[9] didn't carry to bit 22 already, then it should have after any final reduction
            debug_assert!(self.0[9] >> 22 == x);

            // Mask off the possible multiple of 2^256 from the final reduction
            self.0[9] &= 0x03FFFFF;
        }
    }

    #[inline(always)]
    pub(super) const fn normalizes_to_zero(&self) -> bool {
        let mut t0 = self.0[0];
        let mut t9 = self.0[9];

        // Reduce t9 at the start so there will be at most a single carry from the first pass
        let x = t9 >> 22;

        // The first pass ensures the magnitude is 1, ...
        t0 += x * 0x3D1;

        // z0 tracks a possible raw value of 0, z1 tracks a possible raw value of P
        let mut z0 = t0 & 0x3FFFFFF;
        let mut z1 = z0 ^ 0x3D0;

        // Fast return path should catch the majority of cases
        if (z0 != 0) & (z1 != 0x3FFFFFF) {
            return false;
        }

        let mut t1 = self.0[1];
        let mut t2 = self.0[2];
        let mut t3 = self.0[3];
        let mut t4 = self.0[4];
        let mut t5 = self.0[5];
        let mut t6 = self.0[6];
        let mut t7 = self.0[7];
        let mut t8 = self.0[8];

        t9 &= 0x03FFFFF;
        t1 += x << 6;

        t1 += t0 >> 26;
        t2 += t1 >> 26;
        t1 &= 0x3FFFFFF;
        z0 |= t1;
        z1 &= t1 ^ 0x40;
        t3 += t2 >> 26;
        t2 &= 0x3FFFFFF;
        z0 |= t2;
        z1 &= t2;
        t4 += t3 >> 26;
        t3 &= 0x3FFFFFF;
        z0 |= t3;
        z1 &= t3;
        t5 += t4 >> 26;
        t4 &= 0x3FFFFFF;
        z0 |= t4;
        z1 &= t4;
        t6 += t5 >> 26;
        t5 &= 0x3FFFFFF;
        z0 |= t5;
        z1 &= t5;
        t7 += t6 >> 26;
        t6 &= 0x3FFFFFF;
        z0 |= t6;
        z1 &= t6;
        t8 += t7 >> 26;
        t7 &= 0x3FFFFFF;
        z0 |= t7;
        z1 &= t7;
        t9 += t8 >> 26;
        t8 &= 0x3FFFFFF;
        z0 |= t8;
        z1 &= t8;
        z0 |= t9;
        z1 &= t9 ^ 0x3C00000;

        // ... except for a possible carry at bit 22 of t9 (i.e. bit 256 of the field element)
        debug_assert!(t9 >> 23 == 0);

        (z0 == 0) | (z1 == 0x3FFFFFF)
    }

    #[inline(always)]
    pub(super) const fn is_odd(&self) -> bool {
        (self.0[0] as u8 & 1) != 0
    }

    #[inline(always)]
    pub(super) const fn to_storage(self) -> FieldStorage10x26 {
        FieldStorage10x26([
            self.0[0] | self.0[1] << 26,
            self.0[1] >> 6 | self.0[2] << 20,
            self.0[2] >> 12 | self.0[3] << 14,
            self.0[3] >> 18 | self.0[4] << 8,
            self.0[4] >> 24 | self.0[5] << 2 | self.0[6] << 28,
            self.0[6] >> 4 | self.0[7] << 22,
            self.0[7] >> 10 | self.0[8] << 16,
            self.0[8] >> 16 | self.0[9] << 10,
        ])
    }

    #[inline(always)]
    pub(crate) fn invert_in_place(&mut self) {
        *self = self
            .normalize()
            .to_signed30()
            .modinv32(&MOD_INFO)
            .to_field_elem();
    }

    #[inline(always)]
    fn to_signed30(self) -> Signed30 {
        const M30: u32 = u32::MAX >> 2;
        let [a0, a1, a2, a3, a4, a5, a6, a7, a8, a9] = self.0;
        Signed30([
            ((a0 | a1 << 26) & M30) as i32,
            ((a1 >> 4 | a2 << 22) & M30) as i32,
            ((a2 >> 8 | a3 << 18) & M30) as i32,
            ((a3 >> 12 | a4 << 14) & M30) as i32,
            ((a4 >> 16 | a5 << 10) & M30) as i32,
            ((a5 >> 20 | a6 << 6) & M30) as i32,
            ((a6 >> 24 | a7 << 2 | a8 << 28) & M30) as i32,
            ((a8 >> 2 | a9 << 24) & M30) as i32,
            (a9 >> 6) as i32,
        ])
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FieldStorage10x26(pub(super) [u32; 8]);

impl FieldStorage10x26 {
    pub(super) const DEFAULT: Self = Self([0; 8]);

    #[cfg(not(feature = "bigint_ops"))]
    #[inline(always)]
    pub(super) fn to_field_elem(self) -> FieldElement10x26 {
        FieldElement10x26([
            self.0[0] & 0x3FFFFFF,
            self.0[0] >> 26 | ((self.0[1] << 6) & 0x3FFFFFF),
            self.0[1] >> 20 | ((self.0[2] << 12) & 0x3FFFFFF),
            self.0[2] >> 14 | ((self.0[3] << 18) & 0x3FFFFFF),
            self.0[3] >> 8 | ((self.0[4] << 24) & 0x3FFFFFF),
            (self.0[4] >> 2) & 0x3FFFFFF,
            self.0[4] >> 28 | ((self.0[5] << 4) & 0x3FFFFFF),
            self.0[5] >> 22 | ((self.0[6] << 10) & 0x3FFFFFF),
            self.0[6] >> 16 | ((self.0[7] << 16) & 0x3FFFFFF),
            self.0[7] >> 10,
        ])
    }

    #[cfg(feature = "bigint_ops")]
    #[inline(always)]
    pub(super) fn to_field_elem(self) -> crate::secp256k1::field::field_8x32::FieldElement8x32 {
        let mut res = [0; 4];
        let words = self.0;
        let mut i = 0;
        while i < 4 {
            res[i] = words[2 * i] as u64 + ((words[2 * i + 1] as u64) << 32);
            i += 1;
        }
        crate::secp256k1::field::field_8x32::FieldElement8x32::from_words(res)
    }
}

#[cfg(test)]
impl PartialEq for FieldElement10x26 {
    fn eq(&self, other: &Self) -> bool {
        self.normalize().0 == other.normalize().0
    }
}

#[cfg(test)]
impl proptest::arbitrary::Arbitrary for FieldElement10x26 {
    type Parameters = ();

    fn arbitrary_with(_args: Self::Parameters) -> Self::Strategy {
        use proptest::prelude::{any, Strategy};

        any::<[u32; 10]>().prop_map(|limbs| Self(limbs).normalize())
    }

    type Strategy = proptest::arbitrary::Mapped<[u32; 10], Self>;
}

#[cfg(test)]
mod tests {
    use proptest::{prop_assert_eq, proptest};

    use super::FieldElement10x26;

    const ONE: FieldElement10x26 = FieldElement10x26([1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    const ZERO: FieldElement10x26 = FieldElement10x26([0; 10]);

    #[test]
    fn test_invert_var() {
        proptest!(|(x: FieldElement10x26)| {
            let mut a = x;
            a.invert_in_place();
            a.invert_in_place();
            prop_assert_eq!(a, x);

            a = x;
            a.invert_in_place();
            a.mul_in_place(&x);
            if x.normalizes_to_zero() {
                prop_assert_eq!(a, ZERO);
            } else {
                prop_assert_eq!(a, ONE);
            }
        })
    }

    #[test]
    fn test_mul() {
        proptest!(|(x: FieldElement10x26, y: FieldElement10x26, z: FieldElement10x26)| {
            prop_assert_eq!(x.mul(&y), y.mul(&x));
            prop_assert_eq!(x.mul(&y).mul(&z), x.mul(&y.mul(&z)));
            prop_assert_eq!(x.mul(&ONE), x);
            prop_assert_eq!(x.mul(&ZERO), ZERO);

            prop_assert_eq!(x.mul(&y.add(&z)), x.mul(&y).add(&x.mul(&z)));
        })
    }

    #[test]
    fn test_add() {
        proptest!(|(x: FieldElement10x26, y: FieldElement10x26, z: FieldElement10x26)| {
            prop_assert_eq!(x.add(&y), y.add(&x));
            prop_assert_eq!(x.add(&ZERO), x);
            prop_assert_eq!(x.add(&y).add(&z), x.add(&y.add(&z)));
            prop_assert_eq!(x.add(&x.negate(1)), ZERO);
        })
    }

    #[test]
    fn to_signed30_round() {
        proptest!(|(x: FieldElement10x26)| {
            prop_assert_eq!(x.to_signed30().to_field_elem(), x);
        })
    }

    // #[test]
    // fn to_storage_round() {
    //     proptest!(|(x: FieldElement10x26)| {
    //         let s = x.to_storage();
    //         prop_assert_eq!(s.to_field_elem(), x);
    //     })
    // }

    #[test]
    fn from_bytes_round() {
        proptest!(|(bytes: [u8; 32])| {
            prop_assert_eq!(&*FieldElement10x26::from_bytes_unchecked(&bytes).normalize().to_bytes(), &bytes);
        })
    }

    #[test]
    fn to_bytes_round() {
        proptest!(|(x: FieldElement10x26)| {
            let bytes = &*x.to_bytes();
            prop_assert_eq!(FieldElement10x26::from_bytes_unchecked(bytes.try_into().unwrap()), x);
        })
    }
}
