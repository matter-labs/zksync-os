// adapted from https://github.com/bitcoin-core/secp256k1/blob/master/src/modinv64_impl.h

use core::{
    cmp::Ordering,
    num::Wrapping,
    ops::{Mul, Neg},
};

use super::field_5x52::FieldElement5x52;

/// Transition matrix
/// Can hold 62 division steps
#[derive(Debug)]
struct TransitionMatrix {
    u: i64,
    v: i64,
    q: i64,
    r: i64,
}

impl TransitionMatrix {
    fn det(&self) -> i128 {
        self.u as i128 * self.r as i128 - self.v as i128 * self.q as i128
    }

    /// Compute the transition matrix and eta for 62 divsteps
    fn divsteps62(eta: &mut i64, f0: Wrapping<u64>, g0: Wrapping<u64>) -> Self {
        let mut u = Wrapping(1u64);
        let mut v = Wrapping(0u64);
        let mut q = Wrapping(0u64);
        let mut r = Wrapping(1u64);

        let mut f = f0;
        let mut g = g0;

        let mut i = 62;

        loop {
            let w: Wrapping<u32>;
            let m: Wrapping<u64>;
            let zeros = (g.0 | (u64::MAX << i)).trailing_zeros() as usize;

            g >>= zeros;
            u <<= zeros;
            v <<= zeros;
            *eta -= zeros as i64;
            i -= zeros as i64;

            if i == 0 {
                break;
            }

            debug_assert!(f.0 & 1 == 1);
            debug_assert!(g.0 & 1 == 1);
            debug_assert!(u * f0 + v * g0 == f << (62 - i) as usize);
            debug_assert!(q * f0 + r * g0 == g << (62 - i) as usize);
            debug_assert!(*eta >= -745 && *eta <= 745);

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

                let limit = if *eta as i16 as i64 + 1 > i {
                    i
                } else {
                    *eta as i16 as i64 + 1
                };
                debug_assert!(limit > 0 && limit <= 62);
                m = Wrapping((u64::MAX >> (64 - limit)) & 63);

                w = Wrapping(((f * g * (f * f - Wrapping(2))) & m).0 as u32);
            } else {
                let limit = if *eta as i16 as i64 + 1 > i {
                    i
                } else {
                    *eta as i16 as i64 + 1
                };
                debug_assert!(limit > 0 && limit <= 62);

                m = Wrapping((u64::MAX >> (64 - limit)) & 15);
                let temp = Wrapping((f + (((f + Wrapping(1)) & Wrapping(4)) << 1)).0 as u32 as u64);
                w = Wrapping(((-temp * g) & m).0 as u32);
            }

            let w = Wrapping(w.0 as u64);
            g += f * w;
            q += u * w;
            r += v * w;
            debug_assert!(g & m == Wrapping(0));
        }

        let t = TransitionMatrix {
            u: u.0 as i64,
            v: v.0 as i64,
            q: q.0 as i64,
            r: r.0 as i64,
        };

        debug_assert!(t.det().unsigned_abs().is_power_of_two());

        t
    }

    /// Multiplies `2^-62 * self` with `[f, g]`
    fn updaet_fg62(&self, len: usize, f: &Signed62, g: &Signed62) -> (Signed62, Signed62) {
        const M62: i64 = (u64::MAX >> 2) as i64;
        let u = self.u as i128;
        let v = self.v as i128;
        let q = self.q as i128;
        let r = self.r as i128;

        let mut fi = f.0[0] as i128;
        let mut gi = g.0[0] as i128;

        let mut f_out = Signed62::ZERO;
        let mut g_out = Signed62::ZERO;

        let mut cf = u * fi + v * gi;
        let mut cg = q * fi + r * gi;

        debug_assert!(cf as i64 & M62 == 0);
        debug_assert!(cg as i64 & M62 == 0);
        cf >>= 62;
        cg >>= 62;

        for i in 1..len {
            fi = f.0[i] as i128;
            gi = g.0[i] as i128;

            cf += u * fi + v * gi;
            cg += q * fi + r * gi;

            f_out.0[i - 1] = cf as i64 & M62;
            g_out.0[i - 1] = cg as i64 & M62;

            cf >>= 62;
            cg >>= 62;
        }

        f_out.0[len - 1] = cf as i64;
        g_out.0[len - 1] = cg as i64;

        (f_out, g_out)
    }

    /// Multiplies `2^-62 * self` with `[d, e]`, modulo `mod_info.modulus`
    fn update_de62(&self, d: &Signed62, e: &Signed62, mod_info: &ModInfo) -> (Signed62, Signed62) {
        debug_assert!(*d > -mod_info.modulus * 2);
        debug_assert!(*d < mod_info.modulus);
        debug_assert!(*e > -mod_info.modulus * 2);
        debug_assert!(*e < mod_info.modulus);

        let m62 = u64::MAX >> 2;

        let mut d_out = Signed62::ZERO;
        let mut e_out = Signed62::ZERO;

        let d0 = d.0[0] as i128;
        let d1 = d.0[1] as i128;
        let d2 = d.0[2] as i128;
        let d3 = d.0[3] as i128;
        let d4 = d.0[4] as i128;

        let e0 = e.0[0] as i128;
        let e1 = e.0[1] as i128;
        let e2 = e.0[2] as i128;
        let e3 = e.0[3] as i128;
        let e4 = e.0[4] as i128;

        let u = self.u as i128;
        let v = self.v as i128;
        let q = self.q as i128;
        let r = self.r as i128;

        let sd = d.0[4] >> 63;
        let se = e.0[4] >> 63;
        let mut md = (self.u & sd) + (self.v & se);
        let mut me = (self.q & sd) + (self.r & se);

        let mut cd = u * d0 + v * e0;
        let mut ce = q * d0 + r * e0;

        md -= (mod_info.modulus_inv62.wrapping_mul(cd as u64) as i64).wrapping_add(md) & m62 as i64;
        me -= (mod_info.modulus_inv62.wrapping_mul(ce as u64) as i64).wrapping_add(me) & m62 as i64;

        cd += mod_info.modulus.0[0] as i128 * md as i128;
        ce += mod_info.modulus.0[0] as i128 * me as i128;

        debug_assert!(cd as u64 & m62 == 0);
        debug_assert!(ce as u64 & m62 == 0);

        cd >>= 62;
        ce >>= 62;

        cd += u * d1 + v * e1;
        ce += q * d1 + r * e1;

        if mod_info.modulus.0[1] != 0 {
            cd += mod_info.modulus.0[1] as i128 * md as i128;
            ce += mod_info.modulus.0[1] as i128 * md as i128;
        }

        d_out.0[0] = (cd as u64 & m62) as i64;
        e_out.0[0] = (ce as u64 & m62) as i64;

        cd >>= 62;
        ce >>= 62;

        cd += u * d2 + v * e2;
        ce += q * d2 + r * e2;

        if mod_info.modulus.0[2] != 0 {
            cd += mod_info.modulus.0[2] as i128 * md as i128;
            ce += mod_info.modulus.0[2] as i128 * md as i128;
        }

        d_out.0[1] = (cd as u64 & m62) as i64;
        e_out.0[1] = (ce as u64 & m62) as i64;

        cd >>= 62;
        ce >>= 62;

        cd += u * d3 + v * e3;
        ce += q * d3 + r * e3;

        if mod_info.modulus.0[2] != 0 {
            cd += mod_info.modulus.0[3] as i128 * md as i128;
            ce += mod_info.modulus.0[3] as i128 * md as i128;
        }

        d_out.0[2] = (cd as u64 & m62) as i64;
        e_out.0[2] = (ce as u64 & m62) as i64;

        cd >>= 62;
        ce >>= 62;

        cd += u * d4 + v * e4 + mod_info.modulus.0[4] as i128 * md as i128;
        ce += q * d4 + r * e4 + mod_info.modulus.0[4] as i128 * me as i128;

        d_out.0[3] = (cd as u64 & m62) as i64;
        e_out.0[3] = (ce as u64 & m62) as i64;

        cd >>= 62;
        ce >>= 62;

        d_out.0[4] = cd as i64;
        e_out.0[4] = ce as i64;

        debug_assert!(d_out > -mod_info.modulus * 2 && d_out < mod_info.modulus);
        debug_assert!(e_out > -mod_info.modulus * 2 && e_out < mod_info.modulus);

        (d_out, e_out)
    }
}

/// Signed 62-bit limb integer representation
#[derive(Clone, Copy, Debug)]
pub(super) struct Signed62(pub(super) [i64; 5]);

impl Signed62 {
    const ZERO: Self = Self([0; 5]);
    const ONE: Self = Self([1, 0, 0, 0, 0]);

    /// Computes the modular inverse of `self`
    pub(super) fn modinv64(&self, mod_info: &ModInfo) -> Self {
        let mut f = mod_info.modulus;
        let mut g = *self;
        let mut eta = -1;
        let mut d = Self::ZERO;
        let mut e = Self::ONE;

        let mut len = 5;

        let mut i = 0;

        loop {
            // Compute transition matrix and new eta
            let t = TransitionMatrix::divsteps62(
                &mut eta,
                Wrapping(f.0[0] as u64),
                Wrapping(g.0[0] as u64),
            );

            (d, e) = t.update_de62(&d, &e, mod_info);

            debug_assert!(f > -mod_info.modulus);
            debug_assert!(f <= mod_info.modulus);
            debug_assert!(g > -mod_info.modulus);
            debug_assert!(g < mod_info.modulus);

            (f, g) = t.updaet_fg62(len, &f, &g);

            if g.is_zero() {
                break;
            }

            let fi = f.0[len - 1];
            let gi = g.0[len - 1];
            let mut cond = (len as i64 - 2) >> 63;
            cond |= fi ^ (fi >> 63);
            cond |= gi ^ (gi >> 63);

            if cond == 0 {
                f.0[len - 2] |= fi << 62;
                g.0[len - 2] |= gi << 62;

                len -= 1;
            }

            // we should never need more than 12 * 62 = 744 division steps
            debug_assert!({
                i += 1;
                i < 12
            });

            debug_assert!(f > -mod_info.modulus);
            debug_assert!(f <= mod_info.modulus);
            debug_assert!(g > -mod_info.modulus);
            debug_assert!(g < mod_info.modulus);
        }

        // at this point g is zero and f is +/-1 (i.e. gcd(self, modulus)) and d is +/- the modular inverse
        debug_assert!(g == Self::ZERO);
        debug_assert!(
            (f == Self::ONE || f == -Self::ONE)
                || (*self == Self::ZERO
                    && d == Self::ZERO
                    && (f == mod_info.modulus || f == -mod_info.modulus))
        );

        d.normalize(f.0[len - 1], mod_info)
    }

    /// Puts `self` in range `[0, mod_info.modulus)` and optionally negates it
    fn normalize(&self, sign: i64, mod_info: &ModInfo) -> Self {
        let m62 = (u64::MAX >> 2) as i64;
        let modulus = mod_info.modulus;

        debug_assert!(self.0.iter().all(|&x| x >= -m62 && x <= m62));
        // debug_assert!(*self > -modulus * 2 && *self < modulus);

        let mut r0 = self.0[0];
        let mut r1 = self.0[1];
        let mut r2 = self.0[2];
        let mut r3 = self.0[3];
        let mut r4 = self.0[4];

        let mut cond_add = r4 >> 63;
        r0 += modulus.0[0] & cond_add;
        r1 += modulus.0[1] & cond_add;
        r2 += modulus.0[2] & cond_add;
        r3 += modulus.0[3] & cond_add;
        r4 += modulus.0[4] & cond_add;

        let cond_neg = sign >> 63;
        r0 = (r0 ^ cond_neg) - cond_neg;
        r1 = (r1 ^ cond_neg) - cond_neg;
        r2 = (r2 ^ cond_neg) - cond_neg;
        r3 = (r3 ^ cond_neg) - cond_neg;
        r4 = (r4 ^ cond_neg) - cond_neg;

        r1 += r0 >> 62;
        r0 &= m62;
        r2 += r1 >> 62;
        r1 &= m62;
        r3 += r2 >> 62;
        r2 &= m62;
        r4 += r3 >> 62;
        r3 &= m62;

        cond_add = r4 >> 63;
        r0 += modulus.0[0] & cond_add;
        r1 += modulus.0[1] & cond_add;
        r2 += modulus.0[2] & cond_add;
        r3 += modulus.0[3] & cond_add;
        r4 += modulus.0[4] & cond_add;

        r1 += r0 >> 62;
        r0 &= m62;
        r2 += r1 >> 62;
        r1 &= m62;
        r3 += r2 >> 62;
        r2 &= m62;
        r4 += r3 >> 62;
        r3 &= m62;

        debug_assert!(r0 >> 62 == 0);
        debug_assert!(r1 >> 62 == 0);
        debug_assert!(r2 >> 62 == 0);
        debug_assert!(r3 >> 62 == 0);
        debug_assert!(r4 >> 62 == 0);

        let r = Signed62([r0, r1, r2, r3, r4]);
        debug_assert!(r >= Signed62::ZERO && r < modulus);
        r
    }

    #[inline(always)]
    pub(super) const fn to_field_elem(self) -> FieldElement5x52 {
        let m52 = u64::MAX >> 12;
        let [a0, a1, a2, a3, a4] = unsafe { core::mem::transmute::<[i64; 5], [u64; 5]>(self.0) };

        debug_assert!(a0 >> 62 == 0);
        debug_assert!(a1 >> 62 == 0);
        debug_assert!(a2 >> 62 == 0);
        debug_assert!(a3 >> 62 == 0);
        debug_assert!(a4 >> 8 == 0);

        FieldElement5x52([
            a0 & m52,
            (a0 >> 52 | a1 << 10) & m52,
            (a1 >> 42 | a2 << 20) & m52,
            (a2 >> 32 | a3 << 30) & m52,
            a3 >> 22 | a4 << 40,
        ])
    }

    #[inline(always)]
    fn is_zero(&self) -> bool {
        self.0.iter().all(|&x| x == 0)
    }
}

impl PartialEq for Signed62 {
    fn eq(&self, other: &Self) -> bool {
        // weak normalize
        let a = self * 1;
        let b = other * 1;

        for i in 0..4 {
            debug_assert!(a.0[i] >> 62 == 0);
            debug_assert!(b.0[i] >> 62 == 0);
        }

        a.0.iter().zip(b.0.iter()).all(|(ai, bi)| ai == bi)
    }
}

impl PartialOrd for Signed62 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // weak normalize
        let a = self * 1;
        let b = other * 1;

        for i in 0..4 {
            debug_assert!(a.0[i] >> 62 == 0);
            debug_assert!(b.0[i] >> 62 == 0);
        }

        for i in (0..5).rev() {
            if a.0[i] != b.0[i] {
                return a.0[i].partial_cmp(&b.0[i]);
            }
        }

        Some(Ordering::Equal)
    }
}

impl Mul<i64> for &Signed62 {
    type Output = Signed62;

    fn mul(self, rhs: i64) -> Self::Output {
        *self * rhs
    }
}

impl Mul<i64> for Signed62 {
    type Output = Signed62;

    fn mul(self, rhs: i64) -> Self::Output {
        const M62: u64 = u64::MAX >> 2;

        let mut c = 0i128;
        let mut r = Signed62::ZERO;
        for i in 0..4 {
            c += self.0[i] as i128 * rhs as i128;
            r.0[i] = (c as u64 & M62) as i64;
            c >>= 62;
        }
        c += self.0[4] as i128 * rhs as i128;

        debug_assert_eq!(c, c as i64 as i128);

        r.0[4] = c as i64;

        r
    }
}

impl Neg for Signed62 {
    type Output = Signed62;

    fn neg(self) -> Self::Output {
        self * (-1)
    }
}

#[derive(Debug)]
pub(super) struct ModInfo {
    /// the modulus. Must be odd and in [3, 2^256]
    modulus: Signed62,
    /// `1/modulus mod 2^62`
    modulus_inv62: u64,
}

impl ModInfo {
    pub(super) const fn new(modulus: [i64; 5], modulus_inv62: u64) -> Self {
        Self {
            modulus: Signed62(modulus),
            modulus_inv62,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Signed62;

    #[allow(clippy::erasing_op)] // multiplication by 0 is intentional
    #[test]
    fn signed62_arithmetic() {
        assert!(Signed62::ONE > Signed62::ZERO);
        assert!(-Signed62::ONE < Signed62::ZERO);
        assert_eq!(-(-Signed62::ONE), Signed62::ONE);
        assert_eq!(-Signed62::ZERO, Signed62::ZERO);
        assert_eq!(Signed62::ONE * 0, Signed62::ZERO);
        assert_eq!(Signed62::ONE * 1, Signed62::ONE);
        assert_eq!(Signed62::ONE * (-1), -Signed62::ONE);
    }
}
