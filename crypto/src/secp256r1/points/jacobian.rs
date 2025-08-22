use super::{Affine, Storage};
use crate::secp256r1::field::{FieldElement, FieldElementConst};
use core::{fmt::Debug, ops::Neg};

#[derive(Default, Debug, Clone, Copy)]
pub(crate) struct Jacobian {
    pub(crate) x: FieldElement,
    pub(crate) y: FieldElement,
    pub(crate) z: FieldElement,
}

impl Jacobian {
    #[cfg(test)]
    // coordinates are in montgomerry form
    pub(crate) const GENERATOR: Self = Self {
        x: FieldElement::from_words_unchecked([
            8784043285714375740,
            8483257759279461889,
            8789745728267363600,
            1770019616739251654,
        ]),
        y: FieldElement::from_words_unchecked([
            15992936863339206154,
            10037038012062884956,
            15197544864945402661,
            9615747158586711429,
        ]),
        z: FieldElement::ONE,
    };

    pub(crate) fn is_infinity(&self) -> bool {
        self.z.is_zero() || (self.y.is_zero() && self.x.is_zero())
    }

    // https://www.hyperelliptic.org/EFD/g1p/auto-shortw-jacobian-3.html#doubling-dbl-2004-hmv
    pub(crate) fn double_assign(&mut self) {
        if self.is_infinity() {
            *self = Self::default();
            return;
        }
        // T1 = Z1^2
        let mut t1 = self.z;
        t1.square_assign();
        // T2 = X1-T1
        let mut t2 = self.x;
        t2 -= &t1;
        // T1 = X1+T1
        t1 += &self.x;
        // T2 = T2*T1
        t2 *= &t1;
        // T2 = 3*T2
        t2 *= 3;
        // Y3 = 2*Y1
        self.y.double_assign();
        // Z3 = Y3*Z1
        self.z *= &self.y;
        // Y3 = Y3^2
        self.y.square_assign();
        // T3 = Y3*X1
        let mut t3 = self.x;
        t3 *= &self.y;
        // Y3 = Y3^2
        self.y.square_assign();
        // Y3 = half*Y3
        self.y *= &FieldElement::HALF;
        // X3 = T2^2
        self.x = t2;
        self.x.square_assign();
        // T1 = 2*T3
        t1 = t3;
        t1.double_assign();
        // X3 = X3-T1
        self.x -= &t1;
        // T1 = T3-X3
        t1 = t3;
        t1 -= &self.x;
        // T1 = T1*T2
        t1 *= &t2;
        // Y3 = T1-Y3
        self.y.sub_and_negate_assign(&t1);
    }

    // https://www.hyperelliptic.org/EFD/g1p/auto-shortw-jacobian-3.html#addition-add-2007-bl
    pub(crate) fn add_assign(&mut self, other: &Self) {
        if self.is_infinity() {
            *self = *other;
            return;
        }

        if other.is_infinity() {
            return;
        }

        let mut z1z1 = self.z;
        z1z1.square_assign();

        let mut z2z2 = other.z;
        z2z2.square_assign();

        let mut u1 = z2z2;
        u1 *= &self.x;

        let mut u2 = z1z1;
        u2 *= &other.x;

        let mut s1 = z2z2;
        s1 *= &other.z;
        s1 *= &self.y;

        let mut s2 = z1z1;
        s2 *= &self.z;
        s2 *= &other.y;

        let mut h = u2;
        h -= &u1;

        let mut r = s2;
        r -= &s1;

        // if self == other
        if h.is_zero() && r.is_zero() {
            self.double_assign();
            return;
        }

        r.double_assign();

        let mut i = h;
        i.double_assign();
        i.square_assign();

        let mut j = h;
        j *= &i;

        let mut v = u1;
        v *= &i;

        self.x = r;
        self.x.square_assign();
        self.x -= &j;
        self.x -= &v;
        self.x -= &v;

        self.y = v;
        self.y -= &self.x;
        self.y *= &r;

        s1 *= &j;
        s1.double_assign();

        self.y -= &s1;

        self.z *= &other.z;
        self.z.double_assign();
        self.z *= &h;
    }

    // https://www.hyperelliptic.org/EFD/g1p/auto-shortw-jacobian-3.html#addition-madd-2007-bl
    pub(crate) fn add_ge_assign(&mut self, other: &Affine) {
        if self.is_infinity() {
            *self = other.to_jacobian();
            return;
        }

        if other.is_infinity() {
            return;
        }

        let mut z1z1 = self.z;
        z1z1.square_assign();

        let mut u2 = z1z1;
        u2 *= &other.x;

        let mut s2 = z1z1;
        s2 *= &self.z;
        s2 *= &other.y;

        let mut h = u2;
        h -= &self.x;

        let mut r = s2;
        r -= &self.y;

        if h.is_zero() && r.is_zero() {
            self.double_assign();
            return;
        }

        r.double_assign();

        let mut hh = h;
        hh.square_assign();

        let mut i = hh;
        i *= 4;

        let mut j = h;
        j *= &i;

        let mut v = i;
        v *= &self.x;

        self.x = r;
        self.x.square_assign();
        self.x -= &j;
        self.x -= &v;
        self.x -= &v;

        self.y *= &j;
        self.y.double_assign();

        v -= &self.x;
        v *= &r;

        self.y.sub_and_negate_assign(&v);

        self.z *= &h;
        self.z.double_assign();
    }

    pub(crate) fn to_affine(mut self) -> Affine {
        if self.is_infinity() {
            Affine::INFINITY
        } else {
            self.z.invert_assign();
            self.y *= &self.z;
            self.z.square_assign();
            self.y *= &self.z;
            self.x *= &self.z;

            Affine {
                x: self.x,
                y: self.y,
                infinity: false,
            }
        }
    }
}

impl Neg for Jacobian {
    type Output = Self;

    fn neg(mut self) -> Self::Output {
        self.y.negate_assign();
        self
    }
}

#[derive(Default, Debug, Clone, Copy)]
pub(crate) struct JacobianConst {
    pub(crate) x: FieldElementConst,
    pub(crate) y: FieldElementConst,
    pub(crate) z: FieldElementConst,
}

// only used for contexxt generation
impl JacobianConst {
    const INFINITY: Self = Self {
        x: FieldElementConst::ZERO,
        y: FieldElementConst::ZERO,
        z: FieldElementConst::ZERO,
    };

    // coordinates are in montgomerry form
    pub(crate) const GENERATOR: Self = Self {
        x: FieldElementConst::from_words_unchecked([
            8784043285714375740,
            8483257759279461889,
            8789745728267363600,
            1770019616739251654,
        ]),
        y: FieldElementConst::from_words_unchecked([
            15992936863339206154,
            10037038012062884956,
            15197544864945402661,
            9615747158586711429,
        ]),
        z: FieldElementConst::ONE,
    };

    pub(crate) const fn is_infinity_const(&self) -> bool {
        self.z.is_zero() || (self.x.is_zero() || self.y.is_zero())
    }

    #[cfg(test)]
    pub(crate) fn to_affine(self) -> Affine {
        let x = FieldElement::from_be_bytes(&self.x.to_be_bytes()).unwrap();
        let y = FieldElement::from_be_bytes(&self.y.to_be_bytes()).unwrap();
        let z = FieldElement::from_be_bytes(&self.z.to_be_bytes()).unwrap();

        Jacobian { x, y, z }.to_affine()
    }

    // https://www.hyperelliptic.org/EFD/g1p/auto-shortw-jacobian-3.html#doubling-dbl-2001-b
    pub(crate) const fn double(&self) -> Self {
        if self.is_infinity_const() {
            return Self::INFINITY;
        }
        let delta = self.z.square();
        let gamma = self.y.square();
        let beta = self.x.mul(&gamma);
        // alpha = 3*(X1-delta)*(X1+delta)
        let alpha = self.x.sub(&delta).mul(&self.x.add(&delta)).mul_int(3);

        // X3 = alpha^2-8*beta
        let x = alpha.square().sub(&beta.mul_int(8));
        // Z3 = (Y1+Z1)2-gamma-delta
        let z = self.y.add(&self.z).square().sub(&gamma).sub(&delta);
        // Y3 = alpha*(4*beta-X3)-8*gamma2
        let y = alpha
            .mul(&beta.mul_int(4).sub(&x))
            .sub(&gamma.square().mul_int(8));

        Self { x, y, z }
    }

    // https://www.hyperelliptic.org/EFD/g1p/auto-shortw-jacobian-3.html#addition-add-2007-bl
    pub(crate) const fn add(&self, rhs: &Self) -> Self {
        if self.is_infinity_const() {
            return *rhs;
        }
        if rhs.is_infinity_const() {
            return *self;
        }
        let z1z1 = self.z.square();
        let z2z2 = rhs.z.square();
        let u1 = self.x.mul(&z2z2);
        let u2 = rhs.x.mul(&z1z1);
        let s1 = self.y.mul(&rhs.z).mul(&z2z2);
        let s2 = rhs.y.mul(&self.z).mul(&z1z1);
        let h = u2.sub(&u1);
        let r = s2.sub(&s1).mul_int(2);

        if h.is_zero() && r.is_zero() {
            return self.double();
        }

        let i = h.mul_int(2).square();
        let j = h.mul(&i);
        let v = u1.mul(&i);

        let x = r.square().sub(&j).sub(&v.mul_int(2));
        let y = r.mul(&v.sub(&x)).sub(&s1.mul(&j).mul_int(2));
        let z = self.z.add(&rhs.z).square().sub(&z1z1).sub(&z2z2).mul(&h);

        Self { x, y, z }
    }

    pub(crate) const fn to_storage(self) -> Storage {
        assert!(!self.is_infinity_const());

        let zi = self.z.invert();
        let zi2 = zi.square();
        let x = self.x.mul(&zi2);
        let y = self.y.mul(&zi2).mul(&zi);

        Storage {
            x: x.to_fe(),
            y: y.to_fe(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::secp256r1::{
        field::FieldElement,
        points::{Affine, Jacobian, JacobianConst},
        test_vectors::ADD_TEST_VECTORS,
    };

    #[cfg(feature = "bigint_ops")]
    fn init() {
        crate::secp256r1::init();
        crate::bigint_delegation::init();
    }

    #[test]
    fn compare_double() {
        #[cfg(feature = "bigint_ops")]
        init();

        let mut g = Jacobian::GENERATOR;
        let mut g_const = JacobianConst::GENERATOR;
        for _ in 0..100 {
            g_const = g_const.double();
            g.double_assign();
            assert_eq!(g_const.to_affine(), g.to_affine())
        }
    }

    #[test]
    fn compare_add() {
        #[cfg(feature = "bigint_ops")]
        init();

        let mut a = Jacobian::GENERATOR;
        let mut b = JacobianConst::GENERATOR;
        let mut c = Jacobian::GENERATOR;

        let ge = Jacobian::GENERATOR.to_affine();

        for _ in 0..100 {
            a.add_assign(&Jacobian::GENERATOR);
            c.add_ge_assign(&ge);
            b = b.add(&JacobianConst::GENERATOR);

            assert_eq!(a.to_affine(), b.to_affine());
            assert_eq!(a.to_affine(), c.to_affine());
        }
    }

    #[test]
    fn test_add() {
        #[cfg(feature = "bigint_ops")]
        init();

        let mut g = Jacobian::GENERATOR;

        for (x_bytes, y_bytes) in ADD_TEST_VECTORS {
            let expected = Affine {
                x: FieldElement::from_be_bytes(x_bytes).unwrap(),
                y: FieldElement::from_be_bytes(y_bytes).unwrap(),
                infinity: false,
            };

            assert_eq!(g.to_affine(), expected);
            g.add_assign(&Jacobian::GENERATOR);
        }
    }
}
