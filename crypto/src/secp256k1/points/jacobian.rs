use crate::secp256k1::field::{FieldElement, FieldElementConst};

use super::{affine::AffineConst, Affine, AffineStorage};

#[derive(Debug, Clone, Copy)]
pub(crate) struct JacobianConst {
    pub(crate) x: FieldElementConst,
    pub(crate) y: FieldElementConst,
    pub(crate) z: FieldElementConst,
}

impl JacobianConst {
    pub(crate) const INFINITY: Self = Self {
        x: FieldElementConst::ZERO,
        y: FieldElementConst::ZERO,
        z: FieldElementConst::ZERO,
    };

    pub(crate) const GENERATOR: Self = AffineConst::GENERATOR.to_jacobian();

    const X_MAGNITUDE_MAX: u32 = 4;
    #[cfg(debug_assertions)]
    const Y_MAGNITUDE_MAX: u32 = 4;
    #[cfg(debug_assertions)]
    const Z_MAGNITUDE_MAX: u32 = 1;

    #[inline(always)]
    pub(super) const fn assert_verify(&self) {
        #[cfg(all(debug_assertions, not(feature = "bigint_ops")))]
        {
            debug_assert!(self.x.0.magnitude <= Self::X_MAGNITUDE_MAX);
            debug_assert!(self.x.0.magnitude <= 32);
            debug_assert!(self.y.0.magnitude <= Self::Y_MAGNITUDE_MAX);
            debug_assert!(self.y.0.magnitude <= 32);
            debug_assert!(self.z.0.magnitude <= Self::Z_MAGNITUDE_MAX);
            debug_assert!(self.z.0.magnitude <= 32);
        }
    }

    pub(crate) const fn is_infinity(&self) -> bool {
        self.z.normalizes_to_zero() || (self.y.normalizes_to_zero() && self.x.normalizes_to_zero())
    }

    pub(crate) const fn to_affine_const(self) -> AffineConst {
        self.assert_verify();

        if self.is_infinity() {
            return AffineConst::INFINITY;
        }

        let zi = self.z.invert();
        let z2 = zi.square();
        let z3 = z2.mul(&zi);

        AffineConst {
            x: self.x.mul(&z2),
            y: self.y.mul(&z3),
            infinity: false,
        }
    }

    pub(crate) const fn to_affine_storage_const(self) -> AffineStorage {
        self.to_affine_const().to_storage()
    }

    // https://www.hyperelliptic.org/EFD/g1p/auto-shortw-jacobian-0.html#doubling-dbl-2009-l
    pub(crate) const fn double(&self, rzr: Option<&mut FieldElementConst>) -> Self {
        self.assert_verify();

        if self.is_infinity() {
            if let Some(rzr) = rzr {
                *rzr = FieldElementConst::ONE;
            }
            return Self::INFINITY;
        }

        if let Some(rzr) = rzr {
            *rzr = self.y.normalize().mul_int(2);
        }

        // A = X1^2
        // B = Y1^2
        // C = B^2
        // D = 2*((X1+B)^2-A-C)
        // E = 3*A
        // F = E^2
        // X3 = F-2*D
        // Y3 = E*(D-X3)-8*C
        // Z3 = 2*Y1*Z1

        let a = self.x.square();
        let b = self.y.square();
        let c = b.square();
        let d = self
            .x
            .add(&b)
            .square()
            .add(&a.negate(1))
            .add(&c.negate(1))
            .mul_int(2)
            .normalize();
        let e = a.mul_int(3);
        let f = e.square();

        let x = d.mul_int(2).normalize().negate(1).add(&f).normalize();

        let g = x.negate(1).add(&d).mul(&e);
        let y = g.add(&c.mul_int(8).normalize().negate(1));
        let z = self.z.mul(&self.y).mul_int(2).normalize();

        let ret = Self { x, y, z };

        ret.assert_verify();

        ret
    }

    // https://www.hyperelliptic.org/EFD/g1p/auto-shortw-jacobian-0.html#addition-madd-2007-bl
    pub(crate) const fn add_ge(
        &self,
        a: &AffineConst,
        rzr: Option<&mut FieldElementConst>,
    ) -> Self {
        self.assert_verify();
        a.assert_verify();

        if self.is_infinity() {
            debug_assert!(rzr.is_none());
            return a.to_jacobian();
        }

        if a.is_infinity() {
            if let Some(rzr) = rzr {
                *rzr = FieldElementConst::ONE
            }

            return *self;
        }

        let mut ret = JacobianConst::INFINITY;

        let z12 = self.z.square();
        let u1 = self.x;
        let u2 = a.x.mul(&z12);
        let s1 = self.y;
        let s2 = a.y.mul(&z12).mul(&self.z);
        let h = u1.negate(Self::X_MAGNITUDE_MAX).add(&u2);
        let i = s2.negate(1).add(&s1);
        if h.normalizes_to_zero() {
            if i.normalizes_to_zero() {
                ret = self.double(rzr);
            } else if let Some(rzr) = rzr {
                *rzr = FieldElementConst::ZERO;
            }
            return ret;
        }

        if let Some(rzr) = rzr {
            *rzr = h;
        }
        ret.z = self.z.mul(&h);

        let h2 = h.square().negate(1);
        let mut h3 = h2.mul(&h);
        let t = u1.mul(&h2);

        ret.x = i.square().add(&h3).add(&t.mul_int(2)).normalize();

        ret.y = t.add(&ret.x).mul(&i);
        h3 = h3.mul(&s1);
        ret.y = ret.y.add(&h3).normalize();

        ret.assert_verify();

        ret
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Jacobian {
    pub(crate) x: FieldElement,
    pub(crate) y: FieldElement,
    pub(crate) z: FieldElement,
}

impl Jacobian {
    pub(crate) const INFINITY: Self = Self {
        x: FieldElement::ZERO,
        y: FieldElement::ZERO,
        z: FieldElement::ZERO,
    };

    const X_MAGNITUDE_MAX: u32 = 4;
    const Y_MAGNITUDE_MAX: u32 = 4;
    #[cfg(debug_assertions)]
    const Z_MAGNITUDE_MAX: u32 = 1;

    #[inline(always)]
    pub(super) const fn assert_verify(&self) {
        #[cfg(all(debug_assertions, not(feature = "bigint_ops")))]
        {
            debug_assert!(self.x.0.magnitude <= Self::X_MAGNITUDE_MAX);
            debug_assert!(self.x.0.magnitude <= 32);
            debug_assert!(self.y.0.magnitude <= Self::Y_MAGNITUDE_MAX);
            debug_assert!(self.y.0.magnitude <= 32);
            debug_assert!(self.z.0.magnitude <= Self::Z_MAGNITUDE_MAX);
            debug_assert!(self.z.0.magnitude <= 32);
        }
    }

    pub(crate) fn is_infinity(&self) -> bool {
        self.z.normalizes_to_zero() || (self.y.normalizes_to_zero() && self.x.normalizes_to_zero())
    }

    pub(crate) fn to_affine(self) -> Affine {
        self.assert_verify();

        if self.is_infinity() {
            return Affine::INFINITY;
        }

        let mut zi = self.z;
        zi.invert_in_place();

        let mut ret = Affine {
            x: zi,
            y: zi,
            infinity: false,
        };

        ret.x.square_in_place();
        ret.y *= ret.x;

        ret.x *= self.x;
        ret.y *= self.y;

        ret
    }

    // this is essentially this algorithm https://www.hyperelliptic.org/EFD/g1p/auto-shortw-jacobian-0.html#doubling-dbl-2009-l
    // but reorganized to reduce copies
    pub(crate) fn double_in_place(&mut self, rzr: Option<&mut FieldElement>) {
        self.assert_verify();

        if self.is_infinity() {
            if let Some(rzr) = rzr {
                *rzr = FieldElement::ONE;
            }

            return;
        }

        if let Some(rzr) = rzr {
            *rzr = self.y;
            rzr.double_in_place();
        }

        self.z *= self.y;
        self.z.double_in_place();
        self.z.normalize_in_place();

        let mut a = self.x;
        a.square_in_place();

        self.y.square_in_place();
        let b = self.y;
        self.y.square_in_place();
        self.y.negate_in_place(1);

        self.x += b;
        self.x.square_in_place();
        self.x -= a;
        self.x += self.y;
        self.x.double_in_place();
        self.x.normalize_in_place();

        let mut d = self.x;

        a *= 3;
        let mut f = a;
        f.square_in_place();

        self.x.double_in_place();
        self.x.normalize_in_place();
        self.x.negate_in_place(1);
        self.x += f;
        self.x.normalize_in_place();

        d -= self.x;
        d *= a;
        self.y *= 8;
        self.y.normalize_in_place();
        self.y.add_in_place(&d);
        self.y.normalize_in_place();

        self.assert_verify();
    }

    // essentially this
    // - https://github.com/bitcoin-core/secp256k1/blob/master/src/group_impl.h#L590
    // - https://www.hyperelliptic.org/EFD/g1p/auto-shortw-jacobian-0.html#addition-madd
    // Includes optimisations to reduce copies
    pub(crate) fn add_ge_in_place(&mut self, mut a: Affine, rzr: Option<&mut FieldElement>) {
        self.assert_verify();
        a.assert_verify();

        if self.is_infinity() {
            debug_assert!(rzr.is_none());
            *self = a.to_jacobian();
            return;
        }

        if a.is_infinity() {
            if let Some(rzr) = rzr {
                *rzr = FieldElement::ONE;
            }

            return;
        }

        let mut z12 = self.z;
        z12.square_in_place();
        a.x *= z12;
        a.y *= z12;
        a.y *= self.z;
        a.y.negate_in_place(1);

        let mut h = self.x;
        h.negate_in_place(Self::X_MAGNITUDE_MAX);
        h += a.x;

        let mut i = self.y;
        i += a.y;

        if h.normalizes_to_zero() {
            if i.normalizes_to_zero() {
                self.double_in_place(rzr);
            } else {
                if let Some(rzr) = rzr {
                    *rzr = FieldElement::ZERO;
                }
                *self = Jacobian::INFINITY;
            }
            return;
        }

        if let Some(rzr) = rzr {
            *rzr = h;
        }

        self.z *= h;
        self.z.normalize_in_place();

        let mut h2 = h;
        h2.square_in_place();
        h2.negate_in_place(1);
        h *= h2;

        self.x *= h2;
        let mut t = self.x;
        self.x.double_in_place();
        self.x += h;
        let mut i2 = i;
        i2.square_in_place();
        self.x += i2;
        self.x.normalize_in_place();

        t += self.x;
        t *= i;
        self.y *= h;
        self.y += t;
        self.y.normalize_in_place();
    }

    // https://github.com/bitcoin-core/secp256k1/blob/master/src/group_impl.h#L653
    pub(crate) fn add_zinv_in_place(&mut self, mut b: Affine, z: &FieldElement) {
        self.assert_verify();
        b.assert_verify();

        if b.is_infinity() {
            return;
        }

        if self.is_infinity() {
            let mut z2 = *z;
            z2.square_in_place();
            let mut z3 = z2;
            z3 *= z;

            self.x = b.x;
            self.x *= z2;

            self.y = b.y;
            self.y *= z3;

            self.z = FieldElement::ONE;

            return;
        }

        //  We need to calculate (rx,ry,rz) = (ax,ay,az) + (bx,by,1/z). Due to
        // secp256k1's isomorphism we can multiply the Z coordinates on both sides
        // by z, and get: (rx,ry,rz*z) = (ax,ay,az*z) + (bx,by,1).
        // This means that (rx,ry,rz) can be calculated as
        // (ax,ay,az*z) + (bx,by,1), when not applying the z factor to rz.
        // The variable az below holds the modified Z coordinate for a, which is used
        // for the computation of rx and ry, but not for rz.
        let mut az = self.z;
        az.mul_in_place(z);

        let mut z12 = az;
        z12.square_in_place();

        b.x *= z12;
        b.y *= z12;
        b.y *= az;

        let mut h = self.x;
        h.negate_in_place(Self::X_MAGNITUDE_MAX);
        h += b.x;

        let mut i = self.y;
        i.negate_in_place(Self::Y_MAGNITUDE_MAX);
        i += b.y;

        if h.normalizes_to_zero() {
            if i.normalizes_to_zero() {
                self.double_in_place(None);
            } else {
                *self = Jacobian::INFINITY;
            }
            return;
        }

        self.z *= h;
        self.z.normalize_in_place();

        let mut h2 = h;
        h2.square_in_place();
        h *= h2;

        let mut i2 = i;
        i2.square_in_place();

        self.x *= h2;
        let t = self.x;
        self.x.double_in_place();
        self.x += h;
        self.x.negate_in_place(Self::X_MAGNITUDE_MAX);
        self.x += i2;
        self.x.normalize_in_place();

        self.y *= h;
        self.y.negate_in_place(Self::Y_MAGNITUDE_MAX);
        let mut x = self.x;
        x.negate_in_place(Self::X_MAGNITUDE_MAX);
        x += t;
        x *= i;
        self.y += x;
        self.y.normalize_in_place();
    }
}

#[cfg(test)]
impl proptest::arbitrary::Arbitrary for Jacobian {
    type Parameters = ();

    fn arbitrary_with(_args: Self::Parameters) -> Self::Strategy {
        use proptest::prelude::{any, Strategy};

        any::<Affine>().prop_map(|a| a.to_jacobian())
    }

    type Strategy = proptest::arbitrary::Mapped<Affine, Self>;
}

#[cfg(test)]
mod tests {
    use proptest::{prop_assert_eq, proptest};

    use crate::secp256k1::{
        field::{FieldElement, FieldElementConst},
        points::{affine::AffineConst, Affine, Jacobian},
        test_vectors::ADD_TEST_VECTORS,
    };

    use super::JacobianConst;

    #[test]
    fn test_add_basic() {
        #[cfg(feature = "bigint_ops")]
        crate::secp256k1::init();

        let g = Affine::GENERATOR;
        let mut a = Jacobian::INFINITY;
        a.add_ge_in_place(Affine::INFINITY, None);
        assert_eq!(a.to_affine(), Affine::INFINITY);

        a.add_ge_in_place(g, None);
        assert_eq!(a.to_affine(), g);

        let mut g2 = g.to_jacobian();
        g2.add_ge_in_place(g, None);
        let mut g4 = g2;
        g4.add_ge_in_place(g2.to_affine(), None);

        let mut g4_double = g.to_jacobian();
        g4_double.double_in_place(None);
        g4_double.double_in_place(None);

        assert_eq!(g4.to_affine(), g4_double.to_affine());
    }

    #[test]
    fn test_add_basic_const() {
        let g = AffineConst::GENERATOR;
        let a = JacobianConst::INFINITY.add_ge(&AffineConst::INFINITY, None);
        assert_eq!(a.to_affine_const(), AffineConst::INFINITY);

        assert_eq!(a.add_ge(&g, None).to_affine_const(), g);

        let g2 = g.to_jacobian().add_ge(&g, None);
        let g4 = g2.add_ge(&g2.to_affine_const(), None);
        assert_eq!(
            g4.to_affine_const(),
            g.to_jacobian().double(None).double(None).to_affine_const()
        )
    }

    #[test]
    fn test_repeated_add() {
        #[cfg(feature = "bigint_ops")]
        crate::secp256k1::init();

        let g = Affine::GENERATOR;

        let mut p = g.to_jacobian();

        for i in 0..ADD_TEST_VECTORS.len() {
            let a = p.to_affine();

            let expected = Affine {
                x: FieldElement::from_bytes(&ADD_TEST_VECTORS[i].0).unwrap(),
                y: FieldElement::from_bytes(&ADD_TEST_VECTORS[i].1).unwrap(),
                infinity: false,
            };

            assert_eq!(expected, a);

            p.add_ge_in_place(g, None);
        }
    }

    #[test]
    fn test_repeated_add_const() {
        let g = AffineConst::GENERATOR;

        let mut p = g.to_jacobian();

        for i in 0..ADD_TEST_VECTORS.len() {
            let a = p.to_affine_const();

            let expected = AffineConst {
                x: FieldElementConst::from_bytes_unchecked(&ADD_TEST_VECTORS[i].0),
                y: FieldElementConst::from_bytes_unchecked(&ADD_TEST_VECTORS[i].1),
                infinity: false,
            };

            assert_eq!(expected, a);

            p = p.add_ge(&g, None);
        }
    }

    #[cfg(feature = "secp256k1-static-context")]
    #[test]
    fn test_double() {
        #[cfg(feature = "bigint_ops")]
        crate::secp256k1::init();

        use crate::secp256k1::context::ECRECOVER_CONTEXT;

        // tt = 8G
        let mut tt = Affine::GENERATOR.to_jacobian();
        tt.double_in_place(None);
        tt.double_in_place(None);
        tt.double_in_place(None);

        let tt = tt.to_affine();

        // t = 5G + 3G
        let mut t = ECRECOVER_CONTEXT.pre_g[2].to_affine().to_jacobian();
        t.add_ge_in_place(ECRECOVER_CONTEXT.pre_g[1].to_affine(), None);

        let t = t.to_affine();
        assert_eq!(t, tt);
    }

    #[test]
    fn test_add_zinv() {
        #[cfg(feature = "bigint_ops")]
        crate::secp256k1::init();

        proptest!(|(a: Jacobian, b: Jacobian)| {
            let mut t = a;
            t.add_zinv_in_place(Affine { x: b.x, y: b.y, infinity: false}, &b.z);

            let mut tt = a;
            tt.add_ge_in_place(b.to_affine(), None);

            prop_assert_eq!(t.to_affine(), tt.to_affine());

            let mut t = Jacobian::INFINITY;
            let a = a.to_affine();
            t.add_zinv_in_place(a, &FieldElement::ONE);
            assert_eq!(t.to_affine(), a);
        })
    }
}
