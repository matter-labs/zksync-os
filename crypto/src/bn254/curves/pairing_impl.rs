use super::*;
use crate::bn254::Fq12;
use ark_ec::bn::g2::EllCoeff;
use ark_ec::pairing::Pairing;
use ark_ec::pairing::PairingOutput;
use ark_ec::short_weierstrass::SWCurveConfig;
use ark_ec::AdditiveGroup;
use ark_ec::AffineRepr;
use ark_ec::CurveGroup;
use ark_ff::One;
use ark_ff::{CyclotomicMultSubgroup, Field};
use ark_serialize::CanonicalDeserialize;
use ark_serialize::CanonicalSerialize;

impl Bn254 {
    /// Evaluates the line function at point p.
    fn ell(f: &mut Fq12, coeffs: &EllCoeff<Config>, p: &G1Affine) {
        let mut c0 = coeffs.0;
        let mut c1 = coeffs.1;
        let mut c2 = coeffs.2;

        match Config::TWIST_TYPE {
            TwistType::M => {
                c2.mul_assign_by_fp(&p.y);
                c1.mul_assign_by_fp(&p.x);
                f.mul_by_014(&c0, &c1, &c2);
            }
            TwistType::D => {
                c0.mul_assign_by_fp(&p.y);
                c1.mul_assign_by_fp(&p.x);
                f.mul_by_034(&c0, &c1, &c2);
            }
        }
    }

    fn exp_by_neg_x(mut f: Fq12) -> Fq12 {
        Self::spec_cyclotomic_exp_by_x_inplace(&mut f);

        if !Config::X_IS_NEGATIVE {
            f.cyclotomic_inverse_in_place();
        }
        f
    }

    fn spec_cyclotomic_exp_by_x_inplace(f: &mut Fq12) {
        use ark_ff::Zero;
        if f.is_zero() {
            return;
        }

        Self::fast_exp_loop_with_naf(f, Self::X_NAF.iter().copied());
    }

    const X_NAF: [i8; 63] = [
        1, 0, 0, 0, 1, 0, 1, 0, 0, -1, 0, 1, 0, 1, 0, -1, 0, 0, 1, 0, 1, 0, -1, 0, -1, 0, -1, 0, 1,
        0, 0, 0, 1, 0, 0, 1, 0, 1, 0, 1, 0, -1, 0, 1, 0, 0, 1, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, -1,
        0, 0, 0, 1,
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

impl Pairing for Bn254 {
    type BaseField = Fq;
    type ScalarField = crate::bn254::Fr;
    type G1 = G1Projective;
    type G1Affine = G1Affine;
    type G1Prepared = ark_ec::bn::G1Prepared<Config>;
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

                    for i in (1..Config::ATE_LOOP_COUNT.len()).rev() {
                        if i != Config::ATE_LOOP_COUNT.len() - 1 {
                            f.square_in_place();
                        }

                        Self::ell(&mut f, ell_coeffs.next().unwrap(), &p.0);

                        let bit = Config::ATE_LOOP_COUNT[i - 1];
                        if bit == 1 || bit == -1 {
                            Self::ell(&mut f, &ell_coeffs.next().unwrap(), &p.0);
                        }
                    }

                    if Config::X_IS_NEGATIVE {
                        f.cyclotomic_inverse_in_place();
                    }

                    Self::ell(&mut f, ell_coeffs.next().unwrap(), &p.0);
                    Self::ell(&mut f, ell_coeffs.next().unwrap(), &p.0);

                    result *= f;
                }
                (None, None) => break,
                _ => {
                    panic!("Caller must check input lengths");
                }
            }
        }

        ark_ec::pairing::MillerLoopOutput(result)
    }

    fn final_exponentiation(
        f: ark_ec::pairing::MillerLoopOutput<Self>,
    ) -> Option<PairingOutput<Self>> {
        // Easy part: result = elt^((q^6-1)*(q^2+1)).
        // Follows, e.g., Beuchat et al page 9, by computing result as follows:
        //   elt^((q^6-1)*(q^2+1)) = (conj(elt) * elt^(-1))^(q^2+1)
        let f: crate::bn254::Fq12 = f.0;

        // f1 = r.cyclotomic_inverse_in_place() = f^(p^6)
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

            // Hard part follows Laura Fuentes-Castaneda et al. "Faster hashing to G2"
            // by computing:
            //
            // result = elt^(q^3 * (12*z^3 + 6z^2 + 4z - 1) +
            //               q^2 * (12*z^3 + 6z^2 + 6z) +
            //               q   * (12*z^3 + 6z^2 + 4z) +
            //               1   * (12*z^3 + 12z^2 + 6z + 1))
            // which equals
            //
            // result = elt^( 2z * ( 6z^2 + 3z + 1 ) * (q^4 - q^2 + 1)/r ).

            let y0 = Self::exp_by_neg_x(r);
            let y1 = y0.cyclotomic_square();
            let y2 = y1.cyclotomic_square();
            let mut y3 = y2 * &y1;
            let y4 = Self::exp_by_neg_x(y3);
            let y5 = y4.cyclotomic_square();
            let mut y6 = Self::exp_by_neg_x(y5);
            y3.cyclotomic_inverse_in_place();
            y6.cyclotomic_inverse_in_place();
            let y7 = y6 * &y4;
            let mut y8 = y7 * &y3;
            let y9 = y8 * &y1;
            let y10 = y8 * &y4;
            let y11 = y10 * &r;
            let mut y12 = y9;
            y12.frobenius_map_in_place(1);
            let y13 = y12 * &y11;
            y8.frobenius_map_in_place(2);
            let y14 = y8 * &y13;
            r.cyclotomic_inverse_in_place();
            let mut y15 = r * &y9;
            y15.frobenius_map_in_place(3);
            let y16 = y15 * &y14;

            PairingOutput(y16)
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
            let mut ell_coeffs: [core::mem::MaybeUninit<EllCoeff<Config>>; BN254_NUM_ELL_COEFFS] = unsafe {
                [const { core::mem::MaybeUninit::uninit().assume_init() }; BN254_NUM_ELL_COEFFS]
            };

            let mut r = G2HomProjective {
                x: q.x,
                y: q.y,
                z: Fq2::one(),
            };

            let neg_q = -q;

            for bit in Config::ATE_LOOP_COUNT.iter().rev().skip(1) {
                ell_coeffs[i].write(r.double_in_place(&two_inv));
                i += 1;

                let coeff = match bit {
                    1 => r.add_in_place(&q),
                    -1 => r.add_in_place(&neg_q),
                    _ => continue,
                };
                ell_coeffs[i].write(coeff);
                i += 1;
            }

            let q1 = mul_by_char(q);
            let mut q2 = mul_by_char(q1);

            if Config::X_IS_NEGATIVE {
                r.y = -r.y;
            }

            q2.y = -q2.y;

            ell_coeffs[i].write(r.add_in_place(&q1));
            i += 1;
            ell_coeffs[i].write(r.add_in_place(&q2));
            i += 1;

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
        let e = <Config as BnConfig>::G2Config::COEFF_B * &(c.double() + &c);
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

fn mul_by_char(r: G2Affine) -> G2Affine {
    // multiply by field characteristic
    use ark_ff::Field;

    let mut s = r;
    s.x.frobenius_map_in_place(1);
    s.x *= &Config::TWIST_MUL_BY_Q_X;
    s.y.frobenius_map_in_place(1);
    s.y *= &Config::TWIST_MUL_BY_Q_Y;

    s
}

pub const BN254_NUM_ELL_COEFFS: usize = const {
    let mut result = 2;

    let mut i = 0;
    while i < Config::ATE_LOOP_COUNT.len() - 1 {
        result += 1;
        if Config::ATE_LOOP_COUNT[i] != 0 {
            result += 1;
        }

        i += 1;
    }

    result
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct G2PreparedNoAlloc {
    /// Stores the coefficients of the line evaluations as calculated in
    /// <https://eprint.iacr.org/2013/722.pdf>
    pub ell_coeffs: [ark_ec::bn::g2::EllCoeff<Config>; BN254_NUM_ELL_COEFFS],
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
