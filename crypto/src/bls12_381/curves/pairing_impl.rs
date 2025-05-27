use super::*;
use crate::bls12_381::{Fq12, Fq2};
use ark_ec::bls12::g2::EllCoeff;
use ark_ec::pairing::Pairing;
use ark_ec::pairing::PairingOutput;
use ark_ec::short_weierstrass::SWCurveConfig;
use ark_ec::AdditiveGroup;
use ark_ec::AffineRepr;
use ark_ec::CurveGroup;
use ark_ff::BitIteratorBE;
use ark_ff::One;
use ark_ff::{CyclotomicMultSubgroup, Field, Zero};
use ark_serialize::CanonicalDeserialize;
use ark_serialize::CanonicalSerialize;

impl Bls12_381 {
    /// Evaluates the line function at point p.
    fn ell(f: &mut Fq12, coeffs: &EllCoeff<Config>, p: &G1Affine) {
        let mut c0 = coeffs.0;
        let mut c1 = coeffs.1;
        let mut c2 = coeffs.2;
        let (px, py) = p.xy().unwrap();

        match Config::TWIST_TYPE {
            TwistType::M => {
                c2.mul_assign_by_fp(&py);
                c1.mul_assign_by_fp(&px);
                f.mul_by_014(&c0, &c1, &c2);
            }
            TwistType::D => {
                c0.mul_assign_by_fp(&py);
                c1.mul_assign_by_fp(&px);
                f.mul_by_034(&c0, &c1, &c2);
            }
        }
    }

    // Exponentiates `f` by `Self::X`, and stores the result in `result`.
    fn exp_by_x(f: &Fq12, result: &mut Fq12) {
        *result = *f;
        Self::spec_cyclotomic_exp_by_x_inplace(result);

        if Config::X_IS_NEGATIVE {
            result.cyclotomic_inverse_in_place();
        }
    }

    fn spec_cyclotomic_exp_by_x_inplace(f: &mut Fq12) {
        use ark_ff::Zero;
        if f.is_zero() {
            return;
        }

        Self::fast_exp_loop_with_naf(f, Self::X_NAF.iter().copied());
    }

    const X_NAF: [i8; 65] = [
        1, 0, -1, 0, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0,
    ];

    /// `exp_loop` taken from arkworks
    fn fast_exp_loop_with_naf<I: Iterator<Item = i8>>(f: &mut Fq12, e: I) {
        let self_inverse = f.cyclotomic_inverse().unwrap();
        let mut res = Fq12::one();
        let mut found_nonzero = false;
        for value in e {
            if found_nonzero {
                res.cyclotomic_square_in_place();
            }

            if value != 0 {
                found_nonzero = true;

                if value > 0 {
                    res *= &*f;
                } else {
                    res *= &self_inverse;
                }
            }
        }
        *f = res;
    }
}

impl Pairing for Bls12_381 {
    type BaseField = Fq;
    type ScalarField = crate::bls12_381::Fr;
    type G1 = G1Projective;
    type G1Affine = G1Affine;
    type G1Prepared = ark_ec::bls12::G1Prepared<Config>;
    type G2 = G2Projective;
    type G2Affine = G2Affine;
    type G2Prepared = G2PreparedNoAlloc;
    type TargetField = Fq12;

    fn multi_miller_loop(
        a: impl IntoIterator<Item = impl Into<Self::G1Prepared>>,
        b: impl IntoIterator<Item = impl Into<Self::G2Prepared>>,
    ) -> ark_ec::pairing::MillerLoopOutput<Self> {
        let mut a = a.into_iter();
        let mut b = b.into_iter();
        let mut result = Fq12::one();
        loop {
            match (a.next(), b.next()) {
                (Some(p), Some(q)) => {
                    let p: Self::G1Prepared = p.into();
                    if p.is_zero() {
                        continue;
                    }
                    let q: Self::G2Prepared = q.into();
                    if q.is_zero() {
                        continue;
                    }

                    let mut f = Fq12::one();
                    let mut ell_coeffs = q.ell_coeffs.iter();

                    for i in BitIteratorBE::without_leading_zeros(Config::X).skip(1) {
                        f.square_in_place();
                        Self::ell(&mut f, &ell_coeffs.next().unwrap(), &p.0);
                        if i {
                            Self::ell(&mut f, &ell_coeffs.next().unwrap(), &p.0);
                        }
                    }

                    result *= f;
                }
                (None, None) => break,
                _ => {
                    panic!("Caller must check input lengths");
                }
            }
        }

        if Config::X_IS_NEGATIVE {
            result.cyclotomic_inverse_in_place();
        }

        ark_ec::pairing::MillerLoopOutput(result)
    }

    fn final_exponentiation(
        f: ark_ec::pairing::MillerLoopOutput<Self>,
    ) -> Option<PairingOutput<Self>> {
        // Computing the final exponentiation following
        // https://eprint.iacr.org/2020/875
        // Adapted from the implementation in https://github.com/ConsenSys/gurvy/pull/29

        // f1 = r.cyclotomic_inverse_in_place() = f^(p^6)
        let f = f.0;
        let mut f1 = f;
        f1.cyclotomic_inverse_in_place();

        f.inverse().map(|mut f2| {
            // f2 = f^(-1);
            // r = f^(p^6 - 1)
            let mut r = f1 * &f2;

            // f2 = f^(p^6 - 1)
            f2 = r;
            // r = f^((p^6 - 1)(p^2))
            r.frobenius_map_in_place(2);

            // r = f^((p^6 - 1)(p^2) + (p^6 - 1))
            // r = f^((p^6 - 1)(p^2 + 1))
            r *= &f2;

            // Hard part of the final exponentiation:
            // t[0].CyclotomicSquare(&result)
            let mut y0 = r.cyclotomic_square();
            // t[1].Expt(&result)
            let mut y1 = Fq12::zero();
            Self::exp_by_x(&r, &mut y1);
            // t[2].InverseUnitary(&result)
            let mut y2 = r;
            y2.cyclotomic_inverse_in_place();
            // t[1].Mul(&t[1], &t[2])
            y1 *= &y2;
            // t[2].Expt(&t[1])
            Self::exp_by_x(&y1, &mut y2);
            // t[1].InverseUnitary(&t[1])
            y1.cyclotomic_inverse_in_place();
            // t[1].Mul(&t[1], &t[2])
            y1 *= &y2;
            // t[2].Expt(&t[1])
            Self::exp_by_x(&y1, &mut y2);
            // t[1].Frobenius(&t[1])
            y1.frobenius_map_in_place(1);
            // t[1].Mul(&t[1], &t[2])
            y1 *= &y2;
            // result.Mul(&result, &t[0])
            r *= &y0;
            // t[0].Expt(&t[1])
            Self::exp_by_x(&y1, &mut y0);
            // t[2].Expt(&t[0])
            Self::exp_by_x(&y0, &mut y2);
            // t[0].FrobeniusSquare(&t[1])
            y0 = y1;
            y0.frobenius_map_in_place(2);
            // t[1].InverseUnitary(&t[1])
            y1.cyclotomic_inverse_in_place();
            // t[1].Mul(&t[1], &t[2])
            y1 *= &y2;
            // t[1].Mul(&t[1], &t[0])
            y1 *= &y0;
            // result.Mul(&result, &t[1])
            r *= &y1;
            PairingOutput(r)
        })
    }
}

impl From<G2Affine> for G2PreparedNoAlloc {
    fn from(q: G2Affine) -> Self {
        if q.infinity {
            #[allow(invalid_value)]
            G2PreparedNoAlloc {
                ell_coeffs: unsafe { core::mem::MaybeUninit::uninit().assume_init() }, // unused/filtered above
                infinity: true,
            }
        } else {
            use ark_ff::{AdditiveGroup, One};
            let two_inv = Fq::one().double().inverse().unwrap();
            let mut i = 0;
            let mut ell_coeffs: [core::mem::MaybeUninit<EllCoeff<Config>>;
                BLS12_381_NUM_ELL_COEFFS] = unsafe {
                [const { core::mem::MaybeUninit::uninit().assume_init() }; BLS12_381_NUM_ELL_COEFFS]
            };

            let mut r = G2HomProjective {
                x: q.x,
                y: q.y,
                z: Fq2::one(),
            };

            for bit in BitIteratorBE::new(Config::X).skip(1) {
                ell_coeffs[i].write(r.double_in_place(&two_inv));
                i += 1;

                if bit {
                    ell_coeffs[i].write(r.add_in_place(&q));
                    i += 1;
                }
            }

            assert_eq!(i, ell_coeffs.len());

            Self {
                ell_coeffs: unsafe { ell_coeffs.map(|el| el.assume_init()) },
                infinity: false,
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct G2HomProjective {
    x: Fq2,
    y: Fq2,
    z: Fq2,
}

impl G2HomProjective {
    fn double_in_place(&mut self, two_inv: &Fq) -> EllCoeff<Config> {
        // Formula for line function when working with
        // homogeneous projective coordinates.

        let mut a = self.x * &self.y;
        a.mul_assign_by_fp(two_inv);
        let b = self.y.square();
        let c = self.z.square();
        let e = <Config as Bls12Config>::G2Config::COEFF_B * &(c.double() + &c);
        let f = e.double() + &e;
        let mut g = b + &f;
        g.mul_assign_by_fp(two_inv);
        let h = (self.y + &self.z).square() - &(b + &c);
        let i = e - &b;
        let j = self.x.square();
        let e_square = e.square();

        self.x = a * &(b - &f);
        self.y = g.square() - &(e_square.double() + &e_square);
        self.z = b * &h;
        match Config::TWIST_TYPE {
            TwistType::M => (i, j.double() + &j, -h),
            TwistType::D => (-h, j.double() + &j, i),
        }
    }

    fn add_in_place(&mut self, q: &G2Affine) -> EllCoeff<Config> {
        // Formula for line function when working with
        // homogeneous projective coordinates.
        let theta = self.y - &(q.y * &self.z);
        let lambda = self.x - &(q.x * &self.z);
        let c = theta.square();
        let d = lambda.square();
        let e = lambda * &d;
        let f = self.z * &c;
        let g = self.x * &d;
        let h = e + &f - &g.double();
        self.x = lambda * &h;
        self.y = theta * &(g - &h) - &(e * &self.y);
        self.z *= &e;
        let j = theta * &q.x - &(lambda * &q.y);

        match Config::TWIST_TYPE {
            TwistType::M => (j, -theta, lambda),
            TwistType::D => (lambda, -theta, j),
        }
    }
}

impl Default for G2PreparedNoAlloc {
    fn default() -> Self {
        Self::from(G2Affine::generator())
    }
}

impl From<G2Projective> for G2PreparedNoAlloc {
    fn from(q: G2Projective) -> Self {
        q.into_affine().into()
    }
}

impl<'a> From<&'a G2Affine> for G2PreparedNoAlloc {
    fn from(other: &'a G2Affine) -> Self {
        (*other).into()
    }
}

impl<'a> From<&'a G2Projective> for G2PreparedNoAlloc {
    fn from(q: &'a G2Projective) -> Self {
        q.into_affine().into()
    }
}

impl G2PreparedNoAlloc {
    pub fn is_zero(&self) -> bool {
        self.infinity
    }
}

pub const BLS12_381_NUM_ELL_COEFFS: usize = const {
    let num_bits_except_top_one = (u64::BITS - Config::X[0].leading_zeros() - 1) as usize;
    let mut result = num_bits_except_top_one;
    let num_non_zero_bits = Config::X[0].count_ones();
    // all non-zero bits except top one
    result += num_non_zero_bits as usize - 1;

    result
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct G2PreparedNoAlloc {
    pub ell_coeffs: [ark_ec::bls12::g2::EllCoeff<Config>; BLS12_381_NUM_ELL_COEFFS],
    pub infinity: bool,
}

impl CanonicalSerialize for G2PreparedNoAlloc {
    fn serialize_with_mode<W: ark_serialize::Write>(
        &self,
        _writer: W,
        _compress: ark_serialize::Compress,
    ) -> Result<(), ark_serialize::SerializationError> {
        unimplemented!("not supported");
    }

    fn serialized_size(&self, _compress: ark_serialize::Compress) -> usize {
        unimplemented!("not supported");
    }
}

impl ark_serialize::Valid for G2PreparedNoAlloc {
    fn check(&self) -> Result<(), ark_serialize::SerializationError> {
        unimplemented!("not supported");
    }
}

impl CanonicalDeserialize for G2PreparedNoAlloc {
    fn deserialize_with_mode<R: ark_serialize::Read>(
        _reader: R,
        _compress: ark_serialize::Compress,
        _validate: ark_serialize::Validate,
    ) -> Result<Self, ark_serialize::SerializationError> {
        unimplemented!("not supported");
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn compute_x_naf() {
        let t = ark_ff::biginteger::arithmetic::find_naf(Config::X);
        let t: Vec<_> = t.into_iter().rev().collect();
        dbg!(t);
    }
}
