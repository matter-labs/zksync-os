// adapted from https://github.com/bitcoin-core/secp256k1/blob/master/src/modinv32_impl.h

#![allow(dead_code)]
// adapted from https://github.com/bitcoin-core/secp256k1/blob/master/src/modinv32_impl.h
use core::{
    cmp::Ordering,
    num::Wrapping,
    ops::{Mul, Neg},
};

use super::field_10x26::FieldElement10x26;

const MODINV32_INV256: [u8; 128] = [
    0xFF, 0x55, 0x33, 0x49, 0xC7, 0x5D, 0x3B, 0x11, 0x0F, 0xE5, 0xC3, 0x59, 0xD7, 0xED, 0xCB, 0x21,
    0x1F, 0x75, 0x53, 0x69, 0xE7, 0x7D, 0x5B, 0x31, 0x2F, 0x05, 0xE3, 0x79, 0xF7, 0x0D, 0xEB, 0x41,
    0x3F, 0x95, 0x73, 0x89, 0x07, 0x9D, 0x7B, 0x51, 0x4F, 0x25, 0x03, 0x99, 0x17, 0x2D, 0x0B, 0x61,
    0x5F, 0xB5, 0x93, 0xA9, 0x27, 0xBD, 0x9B, 0x71, 0x6F, 0x45, 0x23, 0xB9, 0x37, 0x4D, 0x2B, 0x81,
    0x7F, 0xD5, 0xB3, 0xC9, 0x47, 0xDD, 0xBB, 0x91, 0x8F, 0x65, 0x43, 0xD9, 0x57, 0x6D, 0x4B, 0xA1,
    0x9F, 0xF5, 0xD3, 0xE9, 0x67, 0xFD, 0xDB, 0xB1, 0xAF, 0x85, 0x63, 0xF9, 0x77, 0x8D, 0x6B, 0xC1,
    0xBF, 0x15, 0xF3, 0x09, 0x87, 0x1D, 0xFB, 0xD1, 0xCF, 0xA5, 0x83, 0x19, 0x97, 0xAD, 0x8B, 0xE1,
    0xDF, 0x35, 0x13, 0x29, 0xA7, 0x3D, 0x1B, 0xF1, 0xEF, 0xC5, 0xA3, 0x39, 0xB7, 0xCD, 0xAB, 0x01,
];

/// Transition matrix
/// Can hold 62 division steps
#[derive(Debug)]
struct TransitionMatrix {
    u: i32,
    v: i32,
    q: i32,
    r: i32,
}

impl TransitionMatrix {
    fn det(&self) -> i64 {
        self.u as i64 * self.r as i64 - self.v as i64 * self.q as i64
    }

    fn divsteps30(eta: &mut i32, f0: Wrapping<u32>, g0: Wrapping<u32>) -> Self {
        let mut u = Wrapping(1u32);
        let mut v = Wrapping(0u32);
        let mut q = Wrapping(0u32);
        let mut r = Wrapping(1u32);

        let mut f = f0;
        let mut g = g0;

        let mut i = 30;

        loop {
            let zeros = (g.0 | u32::MAX << i).trailing_zeros() as usize;

            g >>= zeros;
            u <<= zeros;
            v <<= zeros;
            *eta -= zeros as i32;
            i -= zeros as i32;

            if i == 0 {
                break;
            }
            debug_assert!(f.0 & 1 == 1);
            debug_assert!(g.0 & 1 == 1);
            debug_assert!(u * f0 + v * g0 == f << (30 - i) as usize);
            debug_assert!(q * f0 + r * g0 == g << (30 - i) as usize);
            debug_assert!(*eta >= -751 && *eta <= 751);

            if *eta < 0 {
                let mut tmp = f;
                *eta = -*eta;

                f = g;
                g = -tmp;

                tmp = u;
                u = q;
                q = -tmp;

                tmp = v;
                v = r;
                r = -tmp;
            }

            let limit = if *eta as i16 as i32 + 1 > i {
                i
            } else {
                *eta as i16 as i32 + 1
            };
            debug_assert!(limit > 0 && limit <= 30);
            let m = Wrapping((u32::MAX >> (32 - limit)) & 255);

            let w = Wrapping(
                ((g * Wrapping(MODINV32_INV256[(f.0 >> 1) as usize & 127] as u32)) & m).0 as u16,
            );

            let w = Wrapping(w.0 as u32);

            g += f * w;
            q += u * w;
            r += v * w;

            debug_assert!(g & m == Wrapping(0));
        }

        let t = TransitionMatrix {
            u: u.0 as i32,
            v: v.0 as i32,
            q: q.0 as i32,
            r: r.0 as i32,
        };

        debug_assert!(t.det().unsigned_abs().is_power_of_two());

        t
    }

    fn update_fg30(&self, len: usize, f: &Signed30, g: &Signed30) -> (Signed30, Signed30) {
        const M30: i32 = (u32::MAX >> 2) as i32;
        let u = self.u as i64;
        let v = self.v as i64;
        let q = self.q as i64;
        let r = self.r as i64;

        let mut fi = f.0[0] as i64;
        let mut gi = g.0[0] as i64;

        let mut f_out = Signed30::ZERO;
        let mut g_out = Signed30::ZERO;

        let mut cf = u * fi + v * gi;
        let mut cg = q * fi + r * gi;

        debug_assert!(cf as i32 & M30 == 0);
        debug_assert!(cg as i32 & M30 == 0);

        cf >>= 30;
        cg >>= 30;

        for i in 1..len {
            fi = f.0[i] as i64;
            gi = g.0[i] as i64;

            cf += u * fi + v * gi;
            cg += q * fi + r * gi;

            f_out.0[i - 1] = cf as i32 & M30;
            g_out.0[i - 1] = cg as i32 & M30;

            cf >>= 30;
            cg >>= 30;
        }

        f_out.0[len - 1] = cf as i32;
        g_out.0[len - 1] = cg as i32;

        (f_out, g_out)
    }

    fn update_de30(&self, d: &Signed30, e: &Signed30, mod_info: &ModInfo) -> (Signed30, Signed30) {
        debug_assert!(*d > -mod_info.modulus * 2);
        debug_assert!(*d < mod_info.modulus);
        debug_assert!(*e > -mod_info.modulus * 2);
        debug_assert!(*e < mod_info.modulus);

        let mut d_out = Signed30::ZERO;
        let mut e_out = Signed30::ZERO;

        const M30: i32 = (u32::MAX >> 2) as i32;
        let u = self.u as i64;
        let v = self.v as i64;
        let q = self.q as i64;
        let r = self.r as i64;

        let sd = d.0[8] >> 31;
        let se = e.0[8] >> 31;

        let mut md = (self.u & sd) + (self.v & se);
        let mut me = (self.q & sd) + (self.r & se);

        let mut di = d.0[0] as i64;
        let mut ei = e.0[0] as i64;
        let mut cd = u * di + v * ei;
        let mut ce = q * di + r * ei;

        md -= (mod_info.modulus_inv30.wrapping_mul(cd as u32) as i32).wrapping_add(md) & M30;
        me -= (mod_info.modulus_inv30.wrapping_mul(ce as u32) as i32).wrapping_add(me) & M30;

        cd += mod_info.modulus.0[0] as i64 * md as i64;
        ce += mod_info.modulus.0[0] as i64 * me as i64;

        debug_assert!(cd as i32 & M30 == 0);
        debug_assert!(ce as i32 & M30 == 0);

        cd >>= 30;
        ce >>= 30;

        for i in 1..9 {
            di = d.0[i] as i64;
            ei = e.0[i] as i64;

            cd += u * di + v * ei;
            ce += q * di + r * ei;
            cd += mod_info.modulus.0[i] as i64 * md as i64;
            ce += mod_info.modulus.0[i] as i64 * me as i64;

            d_out.0[i - 1] = cd as i32 & M30;
            e_out.0[i - 1] = ce as i32 & M30;

            cd >>= 30;
            ce >>= 30;
        }

        d_out.0[8] = cd as i32;
        e_out.0[8] = ce as i32;

        debug_assert!(d_out > -mod_info.modulus * 2 && d_out < mod_info.modulus);
        debug_assert!(e_out > -mod_info.modulus * 2 && e_out < mod_info.modulus);

        (d_out, e_out)
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Signed30(pub(super) [i32; 9]);

impl Signed30 {
    const ZERO: Self = Self([0; 9]);
    const ONE: Self = Self([1, 0, 0, 0, 0, 0, 0, 0, 0]);

    pub(super) fn modinv32(&self, mod_info: &ModInfo) -> Self {
        let mut d = Self::ZERO;
        let mut e = Self::ONE;
        let mut f = mod_info.modulus;
        let mut g = *self;
        let mut eta = -1;

        let mut len = 9;

        let mut i = 0;

        loop {
            let t = TransitionMatrix::divsteps30(
                &mut eta,
                Wrapping(f.0[0] as u32),
                Wrapping(g.0[0] as u32),
            );

            (d, e) = t.update_de30(&d, &e, mod_info);

            debug_assert!(f > -mod_info.modulus);
            debug_assert!(f <= mod_info.modulus);
            debug_assert!(g > -mod_info.modulus);
            debug_assert!(g < mod_info.modulus);

            (f, g) = t.update_fg30(len, &f, &g);

            if g.is_zero() {
                break;
            }

            let fi = f.0[len - 1];
            let gi = g.0[len - 1];

            let mut cond = (len as i32 - 2) >> 31;
            cond |= fi ^ (fi >> 31);
            cond |= gi ^ (gi >> 31);

            if cond == 0 {
                f.0[len - 2] |= fi << 30;
                g.0[len - 2] |= gi << 30;
                len -= 1;
            }

            debug_assert!({
                i += 1;
                i < 25
            });
            debug_assert!(f > -mod_info.modulus);
            debug_assert!(f <= mod_info.modulus);
            debug_assert!(g > -mod_info.modulus);
            debug_assert!(g < mod_info.modulus);
        }

        debug_assert!(g == Self::ZERO);
        debug_assert!(
            (f == Self::ONE || f == -Self::ONE)
                || (*self == Self::ZERO
                    && d == Self::ZERO
                    && (f == mod_info.modulus || f == -mod_info.modulus))
        );

        d.normalize(f.0[len - 1], mod_info)
    }

    pub(super) fn to_field_elem(self) -> FieldElement10x26 {
        const M26: u32 = u32::MAX >> 6;
        let [a0, a1, a2, a3, a4, a5, a6, a7, a8] =
            unsafe { core::mem::transmute::<[i32; 9], [u32; 9]>(self.0) };

        debug_assert!(a0 >> 30 == 0);
        debug_assert!(a1 >> 30 == 0);
        debug_assert!(a2 >> 30 == 0);
        debug_assert!(a3 >> 30 == 0);
        debug_assert!(a4 >> 30 == 0);
        debug_assert!(a5 >> 30 == 0);
        debug_assert!(a6 >> 30 == 0);
        debug_assert!(a7 >> 30 == 0);
        debug_assert!(a8 >> 16 == 0);

        FieldElement10x26([
            a0 & M26,
            (a0 >> 26 | a1 << 4) & M26,
            (a1 >> 22 | a2 << 8) & M26,
            (a2 >> 18 | a3 << 12) & M26,
            (a3 >> 14 | a4 << 16) & M26,
            (a4 >> 10 | a5 << 20) & M26,
            (a5 >> 6 | a6 << 24) & M26,
            (a6 >> 2) & M26,
            (a6 >> 28 | a7 << 2) & M26,
            (a7 >> 24 | a8 << 6),
        ])
    }

    fn normalize(&self, sign: i32, mod_info: &ModInfo) -> Self {
        const M30: i32 = (u32::MAX >> 2) as i32;
        let [mut r0, mut r1, mut r2, mut r3, mut r4, mut r5, mut r6, mut r7, mut r8] = self.0;

        let mut cond_add = r8 >> 31;
        r0 += mod_info.modulus.0[0] & cond_add;
        r1 += mod_info.modulus.0[1] & cond_add;
        r2 += mod_info.modulus.0[2] & cond_add;
        r3 += mod_info.modulus.0[3] & cond_add;
        r4 += mod_info.modulus.0[4] & cond_add;
        r5 += mod_info.modulus.0[5] & cond_add;
        r6 += mod_info.modulus.0[6] & cond_add;
        r7 += mod_info.modulus.0[7] & cond_add;
        r8 += mod_info.modulus.0[8] & cond_add;

        let cond_negate = sign >> 31;
        r0 = (r0 ^ cond_negate) - cond_negate;
        r1 = (r1 ^ cond_negate) - cond_negate;
        r2 = (r2 ^ cond_negate) - cond_negate;
        r3 = (r3 ^ cond_negate) - cond_negate;
        r4 = (r4 ^ cond_negate) - cond_negate;
        r5 = (r5 ^ cond_negate) - cond_negate;
        r6 = (r6 ^ cond_negate) - cond_negate;
        r7 = (r7 ^ cond_negate) - cond_negate;
        r8 = (r8 ^ cond_negate) - cond_negate;

        r1 += r0 >> 30;
        r0 &= M30;
        r2 += r1 >> 30;
        r1 &= M30;
        r3 += r2 >> 30;
        r2 &= M30;
        r4 += r3 >> 30;
        r3 &= M30;
        r5 += r4 >> 30;
        r4 &= M30;
        r6 += r5 >> 30;
        r5 &= M30;
        r7 += r6 >> 30;
        r6 &= M30;
        r8 += r7 >> 30;
        r7 &= M30;

        cond_add = r8 >> 31;
        r0 += mod_info.modulus.0[0] & cond_add;
        r1 += mod_info.modulus.0[1] & cond_add;
        r2 += mod_info.modulus.0[2] & cond_add;
        r3 += mod_info.modulus.0[3] & cond_add;
        r4 += mod_info.modulus.0[4] & cond_add;
        r5 += mod_info.modulus.0[5] & cond_add;
        r6 += mod_info.modulus.0[6] & cond_add;
        r7 += mod_info.modulus.0[7] & cond_add;
        r8 += mod_info.modulus.0[8] & cond_add;

        r1 += r0 >> 30;
        r0 &= M30;
        r2 += r1 >> 30;
        r1 &= M30;
        r3 += r2 >> 30;
        r2 &= M30;
        r4 += r3 >> 30;
        r3 &= M30;
        r5 += r4 >> 30;
        r4 &= M30;
        r6 += r5 >> 30;
        r5 &= M30;
        r7 += r6 >> 30;
        r6 &= M30;
        r8 += r7 >> 30;
        r7 &= M30;

        debug_assert!(r0 >> 30 == 0);
        debug_assert!(r1 >> 30 == 0);
        debug_assert!(r2 >> 30 == 0);
        debug_assert!(r3 >> 30 == 0);
        debug_assert!(r4 >> 30 == 0);
        debug_assert!(r5 >> 30 == 0);
        debug_assert!(r6 >> 30 == 0);
        debug_assert!(r7 >> 30 == 0);

        let r = Self([r0, r1, r2, r3, r4, r5, r6, r7, r8]);

        debug_assert!(r >= Signed30::ZERO && r < mod_info.modulus);
        r
    }

    #[inline(always)]
    fn is_zero(&self) -> bool {
        self.0.iter().all(|&x| x == 0)
    }
}

impl PartialEq for Signed30 {
    fn eq(&self, other: &Self) -> bool {
        // weak normalize
        let a = self * 1;
        let b = other * 1;

        debug_assert!(a.0[..8].iter().all(|&x| x >> 30 == 0));
        debug_assert!(b.0[..8].iter().all(|&x| x >> 30 == 0));

        a.0.iter().zip(b.0.iter()).all(|(ai, bi)| ai == bi)
    }
}

impl PartialOrd for Signed30 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // weak normalize
        let a = self * 1;
        let b = other * 1;

        debug_assert!(a.0[..8].iter().all(|&x| x >> 30 == 0));
        debug_assert!(b.0[..8].iter().all(|&x| x >> 30 == 0));

        for i in (0..9).rev() {
            if a.0[i] != b.0[i] {
                return a.0[i].partial_cmp(&b.0[i]);
            }
        }
        Some(Ordering::Equal)
    }
}

impl Mul<i32> for &Signed30 {
    type Output = Signed30;

    fn mul(self, rhs: i32) -> Self::Output {
        *self * rhs
    }
}

impl Mul<i32> for Signed30 {
    type Output = Signed30;

    fn mul(self, rhs: i32) -> Self::Output {
        const M30: u32 = u32::MAX >> 2;

        let mut c = 0i64;
        let mut r = Signed30::ZERO;
        for i in 0..8 {
            c += self.0[i] as i64 * rhs as i64;
            r.0[i] = (c as u32 & M30) as i32;
            c >>= 30;
        }

        c += self.0[8] as i64 * rhs as i64;

        debug_assert_eq!(c, c as i32 as i64);

        r.0[8] = c as i32;

        r
    }
}

impl Neg for Signed30 {
    type Output = Self;

    fn neg(self) -> Self::Output {
        self * (-1)
    }
}

pub(super) struct ModInfo {
    modulus: Signed30,
    modulus_inv30: u32,
}

impl ModInfo {
    pub(super) const fn new(modulus: [i32; 9], modulus_inv30: u32) -> Self {
        Self {
            modulus: Signed30(modulus),
            modulus_inv30,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ModInfo, Signed30};

    const MOD_INFO: ModInfo = ModInfo::new([-0x3D1, -4, 0, 0, 0, 0, 0, 0, 65536], 0x2DDACACF);

    #[allow(clippy::erasing_op)] // multiplication by 0 is intentional
    #[test]
    fn signed30_arithmetic() {
        assert!(Signed30::ONE > Signed30::ZERO);
        assert!(-Signed30::ONE < Signed30::ZERO);
        assert_eq!(-(-Signed30::ONE), Signed30::ONE);
        assert_eq!(-Signed30::ZERO, Signed30::ZERO);
        assert_eq!(Signed30::ONE * 0, Signed30::ZERO);
        assert_eq!(Signed30::ONE * 1, Signed30::ONE);
        assert_eq!(Signed30::ONE * (-1), -Signed30::ONE);
    }

    #[test]
    fn invert() {
        assert_eq!(Signed30::ONE.modinv32(&MOD_INFO), Signed30::ONE);
    }
}
