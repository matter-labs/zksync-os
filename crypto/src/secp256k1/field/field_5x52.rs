// most of the code in this file comes from https://github.com/RustCrypto/elliptic-curves/blob/master/k256/src/arithmetic/field/field_5x52.rs
use crate::k256::FieldBytes;

use super::mod_inv64::{ModInfo, Signed62};

const MOD_INFO: ModInfo = ModInfo::new([-0x1000003D1, 0, 0, 0, 256], 0x27C7F6E22DDACACF);

#[derive(Clone, Copy)]
pub struct FieldElement5x52(pub(super) [u64; 5]);

impl core::fmt::Debug for FieldElement5x52 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("0x")?;
        let bytes = self.to_bytes();
        #[allow(deprecated)]
        for b in bytes.as_slice().iter() {
            f.write_fmt(format_args!("{b:02x}"))?;
        }

        core::fmt::Result::Ok(())
    }
}

impl FieldElement5x52 {
    pub(super) const ZERO: Self = Self([0; 5]);
    pub(super) const ONE: Self = Self([1, 0, 0, 0, 0]);
    pub(super) const BETA: Self = Self::from_bytes_unchecked(&[
        0x7a, 0xe9, 0x6a, 0x2b, 0x65, 0x7c, 0x07, 0x10, 0x6e, 0x64, 0x47, 0x9e, 0xac, 0x34, 0x34,
        0xe9, 0x9c, 0xf0, 0x49, 0x75, 0x12, 0xf5, 0x89, 0x95, 0xc1, 0x39, 0x6c, 0x28, 0x71, 0x95,
        0x01, 0xee,
    ]);

    #[inline(always)]
    pub(super) const fn from_bytes_unchecked(bytes: &[u8; 32]) -> Self {
        let w0 = (bytes[31] as u64)
            | ((bytes[30] as u64) << 8)
            | ((bytes[29] as u64) << 16)
            | ((bytes[28] as u64) << 24)
            | ((bytes[27] as u64) << 32)
            | ((bytes[26] as u64) << 40)
            | (((bytes[25] & 0xFu8) as u64) << 48);

        let w1 = ((bytes[25] >> 4) as u64)
            | ((bytes[24] as u64) << 4)
            | ((bytes[23] as u64) << 12)
            | ((bytes[22] as u64) << 20)
            | ((bytes[21] as u64) << 28)
            | ((bytes[20] as u64) << 36)
            | ((bytes[19] as u64) << 44);

        let w2 = (bytes[18] as u64)
            | ((bytes[17] as u64) << 8)
            | ((bytes[16] as u64) << 16)
            | ((bytes[15] as u64) << 24)
            | ((bytes[14] as u64) << 32)
            | ((bytes[13] as u64) << 40)
            | (((bytes[12] & 0xFu8) as u64) << 48);

        let w3 = ((bytes[12] >> 4) as u64)
            | ((bytes[11] as u64) << 4)
            | ((bytes[10] as u64) << 12)
            | ((bytes[9] as u64) << 20)
            | ((bytes[8] as u64) << 28)
            | ((bytes[7] as u64) << 36)
            | ((bytes[6] as u64) << 44);

        let w4 = (bytes[5] as u64)
            | ((bytes[4] as u64) << 8)
            | ((bytes[3] as u64) << 16)
            | ((bytes[2] as u64) << 24)
            | ((bytes[1] as u64) << 32)
            | ((bytes[0] as u64) << 40);

        Self([w0, w1, w2, w3, w4])
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
    #[cfg(debug_assertions)]
    pub(super) const fn max_magnitude() -> u32 {
        2047u32
    }

    #[inline(always)]
    pub(super) const fn overflow(&self) -> bool {
        let m = self.0[1] & self.0[2] & self.0[3];
        (self.0[4] >> 48 != 0)
            | ((self.0[4] == 0x0FFFFFFFFFFFFu64)
                & (m == 0xFFFFFFFFFFFFFu64)
                & (self.0[0] >= 0xFFFFEFFFFFC2Fu64))
    }

    #[inline(always)]
    pub(super) fn to_bytes(self) -> FieldBytes {
        let mut ret = FieldBytes::default();
        ret[0] = (self.0[4] >> 40) as u8;
        ret[1] = (self.0[4] >> 32) as u8;
        ret[2] = (self.0[4] >> 24) as u8;
        ret[3] = (self.0[4] >> 16) as u8;
        ret[4] = (self.0[4] >> 8) as u8;
        ret[5] = self.0[4] as u8;
        ret[6] = (self.0[3] >> 44) as u8;
        ret[7] = (self.0[3] >> 36) as u8;
        ret[8] = (self.0[3] >> 28) as u8;
        ret[9] = (self.0[3] >> 20) as u8;
        ret[10] = (self.0[3] >> 12) as u8;
        ret[11] = (self.0[3] >> 4) as u8;
        ret[12] = ((self.0[2] >> 48) as u8 & 0xFu8) | ((self.0[3] as u8 & 0xFu8) << 4);
        ret[13] = (self.0[2] >> 40) as u8;
        ret[14] = (self.0[2] >> 32) as u8;
        ret[15] = (self.0[2] >> 24) as u8;
        ret[16] = (self.0[2] >> 16) as u8;
        ret[17] = (self.0[2] >> 8) as u8;
        ret[18] = self.0[2] as u8;
        ret[19] = (self.0[1] >> 44) as u8;
        ret[20] = (self.0[1] >> 36) as u8;
        ret[21] = (self.0[1] >> 28) as u8;
        ret[22] = (self.0[1] >> 20) as u8;
        ret[23] = (self.0[1] >> 12) as u8;
        ret[24] = (self.0[1] >> 4) as u8;
        ret[25] = ((self.0[0] >> 48) as u8 & 0xFu8) | ((self.0[1] as u8 & 0xFu8) << 4);
        ret[26] = (self.0[0] >> 40) as u8;
        ret[27] = (self.0[0] >> 32) as u8;
        ret[28] = (self.0[0] >> 24) as u8;
        ret[29] = (self.0[0] >> 16) as u8;
        ret[30] = (self.0[0] >> 8) as u8;
        ret[31] = self.0[0] as u8;
        ret
    }

    #[inline(always)]
    pub(super) const fn mul(&self, rhs: &Self) -> Self {
        let mut ret = *self;
        ret.mul_in_place(rhs);
        ret
    }

    #[inline(always)]
    pub(super) const fn mul_in_place(&mut self, rhs: &Self) {
        let a0 = self.0[0] as u128;
        let a1 = self.0[1] as u128;
        let a2 = self.0[2] as u128;
        let a3 = self.0[3] as u128;
        let a4 = self.0[4] as u128;
        let b0 = rhs.0[0] as u128;
        let b1 = rhs.0[1] as u128;
        let b2 = rhs.0[2] as u128;
        let b3 = rhs.0[3] as u128;
        let b4 = rhs.0[4] as u128;
        let m = 0xFFFFFFFFFFFFFu128;
        let r = 0x1000003D10u128;

        debug_assert!(a0 >> 56 == 0);
        debug_assert!(a1 >> 56 == 0);
        debug_assert!(a2 >> 56 == 0);
        debug_assert!(a3 >> 56 == 0);
        debug_assert!(a4 >> 52 == 0);

        debug_assert!(b0 >> 56 == 0);
        debug_assert!(b1 >> 56 == 0);
        debug_assert!(b2 >> 56 == 0);
        debug_assert!(b3 >> 56 == 0);
        debug_assert!(b4 >> 52 == 0);

        // [... a b c] is a shorthand for ... + a<<104 + b<<52 + c<<0 mod n.
        // for 0 <= x <= 4, px is a shorthand for sum(a[i]*b[x-i], i=0..x).
        // for 4 <= x <= 8, px is a shorthand for sum(a[i]*b[x-i], i=(x-4)..4)
        // Note that [x 0 0 0 0 0] = [x*r].

        let mut d = a0 * b3 + a1 * b2 + a2 * b1 + a3 * b0;
        debug_assert!(d >> 114 == 0);
        // [d 0 0 0] = [p3 0 0 0]
        let mut c = a4 * b4;
        debug_assert!(c >> 112 == 0);
        // [c 0 0 0 0 d 0 0 0] = [p8 0 0 0 0 p3 0 0 0]
        d += (c & m) * r;
        c >>= 52;
        debug_assert!(d >> 115 == 0);
        debug_assert!(c >> 60 == 0);
        let c64 = c as u64;
        // [c 0 0 0 0 0 d 0 0 0] = [p8 0 0 0 0 p3 0 0 0]
        let t3 = (d & m) as u64;
        d >>= 52;
        debug_assert!(t3 >> 52 == 0);
        debug_assert!(d >> 63 == 0);
        let d64 = d as u64;
        // [c 0 0 0 0 d t3 0 0 0] = [p8 0 0 0 0 p3 0 0 0]

        d = d64 as u128 + a0 * b4 + a1 * b3 + a2 * b2 + a3 * b1 + a4 * b0;
        debug_assert!(d >> 115 == 0);
        // [c 0 0 0 0 d t3 0 0 0] = [p8 0 0 0 p4 p3 0 0 0]
        d += c64 as u128 * r;
        debug_assert!(d >> 116 == 0);
        // [d t3 0 0 0] = [p8 0 0 0 p4 p3 0 0 0]
        let t4 = (d & m) as u64;
        d >>= 52;
        debug_assert!(t4 >> 52 == 0);
        debug_assert!(d >> 64 == 0);
        let d64 = d as u64;
        // [d t4 t3 0 0 0] = [p8 0 0 0 p4 p3 0 0 0]
        let tx = t4 >> 48;
        let t4 = t4 & ((m as u64) >> 4);
        debug_assert!(tx >> 4 == 0);
        debug_assert!(t4 >> 48 == 0);
        // [d t4+(tx<<48) t3 0 0 0] = [p8 0 0 0 p4 p3 0 0 0]

        c = a0 * b0;
        debug_assert!(c >> 112 == 0);
        // [d t4+(tx<<48) t3 0 0 c] = [p8 0 0 0 p4 p3 0 0 p0]
        d = d64 as u128 + a1 * b4 + a2 * b3 + a3 * b2 + a4 * b1;
        debug_assert!(d >> 115 == 0);
        // [d t4+(tx<<48) t3 0 0 c] = [p8 0 0 p5 p4 p3 0 0 p0]
        let u0 = (d & m) as u64;
        d >>= 52;
        debug_assert!(u0 >> 52 == 0);
        debug_assert!(d >> 63 == 0);
        let d64 = d as u64;
        // [d u0 t4+(tx<<48) t3 0 0 c] = [p8 0 0 p5 p4 p3 0 0 p0]
        // [d 0 t4+(tx<<48)+(u0<<52) t3 0 0 c] = [p8 0 0 p5 p4 p3 0 0 p0]
        let u0 = (u0 << 4) | tx;
        debug_assert!(u0 >> 56 == 0);
        // [d 0 t4+(u0<<48) t3 0 0 c] = [p8 0 0 p5 p4 p3 0 0 p0]
        c += u0 as u128 * ((r as u64) >> 4) as u128;
        debug_assert!(c >> 115 == 0);
        // [d 0 t4 t3 0 0 c] = [p8 0 0 p5 p4 p3 0 0 p0]
        let r0 = (c & m) as u64;
        c >>= 52;
        debug_assert!(r0 >> 52 == 0);
        debug_assert!(c >> 61 == 0);
        let c64 = c as u64;
        // [d 0 t4 t3 0 c r0] = [p8 0 0 p5 p4 p3 0 0 p0]

        c = c64 as u128 + a0 * b1 + a1 * b0;
        debug_assert!(c >> 114 == 0);
        // [d 0 t4 t3 0 c r0] = [p8 0 0 p5 p4 p3 0 p1 p0]
        d = d64 as u128 + a2 * b4 + a3 * b3 + a4 * b2;
        debug_assert!(d >> 114 == 0);
        // [d 0 t4 t3 0 c r0] = [p8 0 p6 p5 p4 p3 0 p1 p0]
        c += (d & m) * r;
        d >>= 52;
        debug_assert!(c >> 115 == 0);
        debug_assert!(d >> 62 == 0);
        let d64 = d as u64;
        // [d 0 0 t4 t3 0 c r0] = [p8 0 p6 p5 p4 p3 0 p1 p0]
        let r1 = (c & m) as u64;
        c >>= 52;
        debug_assert!(r1 >> 52 == 0);
        debug_assert!(c >> 63 == 0);
        let c64 = c as u64;
        // [d 0 0 t4 t3 c r1 r0] = [p8 0 p6 p5 p4 p3 0 p1 p0]

        c = c64 as u128 + a0 * b2 + a1 * b1 + a2 * b0;
        debug_assert!(c >> 114 == 0);
        // [d 0 0 t4 t3 c r1 r0] = [p8 0 p6 p5 p4 p3 p2 p1 p0]
        d = d64 as u128 + a3 * b4 + a4 * b3;
        debug_assert!(d >> 114 == 0);
        // [d 0 0 t4 t3 c t1 r0] = [p8 p7 p6 p5 p4 p3 p2 p1 p0]
        c += (d & m) * r;
        d >>= 52;
        debug_assert!(c >> 115 == 0);
        debug_assert!(d >> 62 == 0);
        let d64 = d as u64;
        // [d 0 0 0 t4 t3 c r1 r0] = [p8 p7 p6 p5 p4 p3 p2 p1 p0]

        // [d 0 0 0 t4 t3 c r1 r0] = [p8 p7 p6 p5 p4 p3 p2 p1 p0]
        let r2 = (c & m) as u64;
        c >>= 52;
        debug_assert!(r2 >> 52 == 0);
        debug_assert!(c >> 63 == 0);
        let c64 = c as u64;
        // [d 0 0 0 t4 t3+c r2 r1 r0] = [p8 p7 p6 p5 p4 p3 p2 p1 p0]
        c = c64 as u128 + (d64 as u128) * r + t3 as u128;
        debug_assert!(c >> 100 == 0);
        // [t4 c r2 r1 r0] = [p8 p7 p6 p5 p4 p3 p2 p1 p0]
        let r3 = (c & m) as u64;
        c >>= 52;
        debug_assert!(r3 >> 52 == 0);
        debug_assert!(c >> 48 == 0);
        let c64 = c as u64;
        // [t4+c r3 r2 r1 r0] = [p8 p7 p6 p5 p4 p3 p2 p1 p0]
        c = c64 as u128 + t4 as u128;
        debug_assert!(c >> 49 == 0);
        // [c r3 r2 r1 r0] = [p8 p7 p6 p5 p4 p3 p2 p1 p0]
        let r4 = c as u64;
        debug_assert!(r4 >> 49 == 0);
        // [r4 r3 r2 r1 r0] = [p8 p7 p6 p5 p4 p3 p2 p1 p0]

        *self = Self([r0, r1, r2, r3, r4]);
    }

    #[inline(always)]
    pub(super) const fn mul_int(&self, rhs: u32) -> Self {
        let mut ret = *self;
        ret.mul_int_in_place(rhs);
        ret
    }

    #[inline(always)]
    pub(super) const fn mul_int_in_place(&mut self, rhs: u32) {
        let rhs_u64 = rhs as u64;

        self.0[0] *= rhs_u64;
        self.0[1] *= rhs_u64;
        self.0[2] *= rhs_u64;
        self.0[3] *= rhs_u64;
        self.0[4] *= rhs_u64;
    }

    #[inline(always)]
    pub(super) const fn square(&self) -> Self {
        let mut ret = *self;
        ret.mul_in_place(self);
        ret
    }

    #[inline(always)]
    pub(super) const fn square_in_place(&mut self) {
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
    }

    #[inline(always)]
    pub(super) const fn double_in_place(&mut self) {
        let rhs = *self;
        self.add_in_place(&rhs);
    }

    #[inline(always)]
    pub(super) fn add_int_in_place(&mut self, rhs: u32) {
        self.0[0] += rhs as u64;
    }

    #[inline(always)]
    pub(super) const fn negate(&self, magnitude: u32) -> Self {
        let mut ret = *self;
        ret.negate_in_place(magnitude);
        ret
    }

    #[inline(always)]
    pub(super) fn sub_in_place(&mut self, rhs: &Self) {
        let mut rhs = *rhs;
        rhs.normalize_in_place();
        rhs.negate_in_place(1);
        self.add_in_place(&rhs);
    }

    #[inline(always)]
    pub(super) const fn negate_in_place(&mut self, magnitude: u32) {
        let m = (magnitude + 1) as u64;
        self.0[0] = 0xFFFFEFFFFFC2Fu64 * 2 * m - self.0[0];
        self.0[1] = 0xFFFFFFFFFFFFFu64 * 2 * m - self.0[1];
        self.0[2] = 0xFFFFFFFFFFFFFu64 * 2 * m - self.0[2];
        self.0[3] = 0xFFFFFFFFFFFFFu64 * 2 * m - self.0[3];
        self.0[4] = 0x0FFFFFFFFFFFFu64 * 2 * m - self.0[4];
    }

    #[inline(always)]
    pub(super) const fn normalize(&self) -> Self {
        let mut ret = *self;
        ret.normalize_in_place();
        ret
    }

    #[inline(always)]
    pub(super) const fn normalize_in_place(&mut self) {
        let mut x = self.0[4] >> 48;
        self.0[4] &= 0x0FFFFFFFFFFFF;

        /* The first pass ensures the magnitude is 1, ... */
        self.0[0] += x * 0x1000003D1;
        self.0[1] += self.0[0] >> 52;
        self.0[0] &= 0xFFFFFFFFFFFFF;
        self.0[2] += self.0[1] >> 52;
        self.0[1] &= 0xFFFFFFFFFFFFF;
        let mut m = self.0[1];
        self.0[3] += self.0[2] >> 52;
        self.0[2] &= 0xFFFFFFFFFFFFF;
        m &= self.0[2];
        self.0[4] += self.0[3] >> 52;
        self.0[3] &= 0xFFFFFFFFFFFFF;
        m &= self.0[3];

        //... except for a possible carry at bit 48 of self.0[4] (i.e. bit 256 of the field element)
        debug_assert!(self.0[4] >> 49 == 0);

        // At most a single final reduction is needed; check if the value is >= the field characteristic
        x = (self.0[4] >> 48)
            | ((self.0[4] == 0x0FFFFFFFFFFFF)
                & (m == 0xFFFFFFFFFFFFF)
                & (self.0[0] >= 0xFFFFEFFFFFC2F)) as u64;

        if x != 0 {
            self.0[0] += 0x1000003D1;
            self.0[1] += self.0[0] >> 52;
            self.0[0] &= 0xFFFFFFFFFFFFF;
            self.0[2] += self.0[1] >> 52;
            self.0[1] &= 0xFFFFFFFFFFFFF;
            self.0[3] += self.0[2] >> 52;
            self.0[2] &= 0xFFFFFFFFFFFFF;
            self.0[4] += self.0[3] >> 52;
            self.0[3] &= 0xFFFFFFFFFFFFF;

            // If self.0[4] didn't carry to bit 48 already, then it should have after any final reduction
            debug_assert!(self.0[4] >> 48 == x);

            // Mask off the possible multiple of 2^256 from the final reduction
            self.0[4] &= 0x0FFFFFFFFFFFF;
        }
    }

    #[inline(always)]
    pub(super) const fn normalizes_to_zero(&self) -> bool {
        let mut t0 = self.0[0];
        let mut t4 = self.0[4];

        // Reduce t4 at the start so there will be at most a single carry from the first pass
        let x = t4 >> 48;

        // The first pass ensures the magnitude is 1
        t0 += x * 0x1000003D1;

        // z0 tracks a possible raw value of 0, z1 tracks a possible raw value of P
        let mut z0 = t0 & 0xFFFFFFFFFFFFF;
        let mut z1 = z0 ^ 0x1000003D0;

        // Fast return path, should catch majority of cases
        let var_name = (z0 != 0) & (z1 != 0xFFFFFFFFFFFFF);
        if var_name {
            return false;
        }

        let mut t1 = self.0[1];
        let mut t2 = self.0[2];
        let mut t3 = self.0[3];

        t4 &= 0x0FFFFFFFFFFFF;

        t1 += t0 >> 52;
        t2 += t1 >> 52;
        t1 &= 0xFFFFFFFFFFFFF;
        z0 |= t1;
        z1 &= t1;
        t3 += t2 >> 52;
        t2 &= 0xFFFFFFFFFFFFF;
        z0 |= t2;
        z1 &= t2;
        t4 += t3 >> 52;
        t3 &= 0xFFFFFFFFFFFFF;
        z0 |= t3;
        z1 &= t3;
        z0 |= t4;
        z1 &= t4 ^ 0xF000000000000;

        debug_assert!(t4 >> 49 == 0);

        (z0 == 0) | (z1 == 0xFFFFFFFFFFFFF)
    }

    #[inline(always)]
    pub(super) const fn is_odd(&self) -> bool {
        (self.0[0] as u8 & 1) != 0
    }

    #[inline(always)]
    pub(super) const fn to_storage(self) -> FieldStorage5x52 {
        FieldStorage5x52([
            self.0[0] | self.0[1] << 52,
            self.0[1] >> 12 | self.0[2] << 40,
            self.0[2] >> 24 | self.0[3] << 28,
            self.0[3] >> 36 | self.0[4] << 16,
        ])
    }

    #[inline(always)]
    pub(crate) fn invert_in_place(&mut self) {
        *self = self
            .normalize()
            .to_signed62()
            .modinv64(&MOD_INFO)
            .to_field_elem();
    }

    #[inline(always)]
    const fn to_signed62(self) -> Signed62 {
        let m62 = u64::MAX >> 2;
        let a0 = self.0[0];
        let a1 = self.0[1];
        let a2 = self.0[2];
        let a3 = self.0[3];
        let a4 = self.0[4];

        Signed62([
            ((a0 | a1 << 52) & m62) as i64,
            ((a1 >> 10 | a2 << 42) & m62) as i64,
            ((a2 >> 20 | a3 << 32) & m62) as i64,
            ((a3 >> 30 | a4 << 22) & m62) as i64,
            (a4 >> 40) as i64,
        ])
    }
}

#[derive(Debug, Clone, Copy)]

pub(super) struct FieldStorage5x52([u64; 4]);

impl FieldStorage5x52 {
    pub(super) const DEFAULT: Self = Self([0; 4]);

    #[inline(always)]
    pub(super) const fn to_field_elem(self) -> FieldElement5x52 {
        FieldElement5x52([
            self.0[0] & 0xFFFFFFFFFFFFF,
            self.0[0] >> 52 | ((self.0[1] << 12) & 0xFFFFFFFFFFFFF),
            self.0[1] >> 40 | ((self.0[2] << 24) & 0xFFFFFFFFFFFFF),
            self.0[2] >> 28 | ((self.0[3] << 36) & 0xFFFFFFFFFFFFF),
            self.0[3] >> 16,
        ])
    }
}

#[cfg(test)]
impl PartialEq for FieldElement5x52 {
    fn eq(&self, other: &Self) -> bool {
        self.normalize().0 == other.normalize().0
    }
}

#[cfg(test)]
impl proptest::arbitrary::Arbitrary for FieldElement5x52 {
    type Parameters = ();

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        use proptest::prelude::{any, Strategy};

        any::<[u64; 5]>().prop_map(|limbs| Self(limbs).normalize())
    }

    type Strategy = proptest::arbitrary::Mapped<[u64; 5], Self>;
}

#[cfg(test)]
mod tests {
    use proptest::{prop_assert_eq, proptest};

    use super::FieldElement5x52;

    const ONE: FieldElement5x52 = FieldElement5x52([1, 0, 0, 0, 0]);
    const ZERO: FieldElement5x52 = FieldElement5x52([0; 5]);

    #[test]
    fn test_invert() {
        proptest!(|(x: FieldElement5x52)| {
            let mut a = x;
            a.invert_in_place();
            a.invert_in_place();
            prop_assert_eq!(a, x);

            a = x;
            a.invert_in_place();
            a.mul_in_place(&x);
            if x.normalizes_to_zero() {
                prop_assert_eq!(a, ZERO)
            } else {
                prop_assert_eq!(a, ONE);
            }
        })
    }

    #[test]
    fn test_mul() {
        proptest!(|(x: FieldElement5x52, y: FieldElement5x52, z: FieldElement5x52)| {
            prop_assert_eq!(x.mul(&y), y.mul(&x));
            prop_assert_eq!(x.mul(&y).mul(&z), x.mul(&y.mul(&z)));
            prop_assert_eq!(x.mul(&ONE), x);
            prop_assert_eq!(x.mul(&ZERO), ZERO);

            prop_assert_eq!(x.mul(&y.add(&z)), x.mul(&y).add(&x.mul(&z)));
        })
    }

    #[test]
    fn test_add() {
        proptest!(|(x: FieldElement5x52, y: FieldElement5x52, z: FieldElement5x52)| {
            prop_assert_eq!(x.add(&y), y.add(&x));
            prop_assert_eq!(x.add(&ZERO), x);
            prop_assert_eq!(x.add(&y).add(&z), x.add(&y.add(&z)));
            prop_assert_eq!(x.add(&x.negate(1)), ZERO);
        })
    }

    #[test]
    fn to_signed62_round() {
        proptest!(|(x: FieldElement5x52)| {
            prop_assert_eq!(x.to_signed62().to_field_elem(), x);
        })
    }

    #[test]
    fn to_storage_round() {
        proptest!(|(x: FieldElement5x52)| {
            let s = x.to_storage();
            prop_assert_eq!(s.to_field_elem(), x);
        })
    }

    #[test]
    fn from_bytes_round() {
        proptest!(|(bytes: [u8; 32])| {
            prop_assert_eq!(&*FieldElement5x52::from_bytes_unchecked(&bytes).normalize().to_bytes(), &bytes);
        })
    }

    #[test]
    fn to_bytes_round() {
        proptest!(|(x: FieldElement5x52)| {
            let bytes = &*x.to_bytes();
            prop_assert_eq!(FieldElement5x52::from_bytes_unchecked(bytes.try_into().unwrap()), x);
        })
    }
}
