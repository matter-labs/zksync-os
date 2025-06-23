use super::{
    context::{GeneratorMultiplesTable, TABLE_G},
    points::Affine,
    scalar::{Scalar, Signature},
    wnaf::Wnaf,
    Secp256r1Err, ECMULT_TABLE_SIZE_A, WINDOW_A, WINDOW_G,
};
use core::mem::MaybeUninit;

type Jacobian = super::points::Jacobian;

pub fn verify(
    digest: &[u8; 32],
    r: &[u8; 32],
    s: &[u8; 32],
    x: &[u8; 32],
    y: &[u8; 32],
) -> Result<bool, Secp256r1Err> {
    let Signature { r, s } = Signature::from_scalars(r, s)?;
    let pk = Affine::from_be_bytes(x, y)?
        .reject_identity()?
        .to_jacobian();
    let z = Scalar::reduce_be_bytes(digest);
    let mut s_inv = s;
    s_inv.invert_assign();

    let u1 = z * &s_inv;
    let u2 = r * &s_inv;

    let x = ecmult(pk, u2, u1, &TABLE_G).to_affine().x;

    let recovered = Scalar::reduce_be_bytes(&x.to_be_bytes());

    let bool = r == recovered;
    Ok(bool)
}

fn ecmult(a: Jacobian, na: Scalar, ng: Scalar, table_g: &GeneratorMultiplesTable) -> Jacobian {
    let (wnaf_a, table_a) = if !na.is_zero() && !a.is_infinity() {
        (Wnaf::new(na, WINDOW_A), OddMultiplesTable::from(&a))
    } else {
        (Wnaf::default(), OddMultiplesTable::default())
    };

    let wnaf_ng = if !ng.is_zero() {
        Wnaf::new(ng, WINDOW_G)
    } else {
        Wnaf::default()
    };

    let bits = wnaf_a.bits().max(wnaf_ng.bits());
    let mut r = Jacobian::default();

    for i in (0..bits).rev() {
        r.double_assign();

        if let Some(n) = wnaf_a.get_digit(i) {
            r.add_assign(&table_a.get(n));
        }

        if let Some(n) = wnaf_ng.get_digit(i) {
            r.add_ge_assign(&table_g.get_ge(n));
        }
    }

    r
}

#[derive(Default)]
struct OddMultiplesTable([Jacobian; ECMULT_TABLE_SIZE_A]);

impl OddMultiplesTable {
    fn from(p: &Jacobian) -> Self {
        debug_assert!(!p.is_infinity());

        let mut table = [const { MaybeUninit::uninit() }; ECMULT_TABLE_SIZE_A];

        let mut p = *p;
        table[0].write(p);
        let mut p_double = p;
        p_double.double_assign();

        for x in table.iter_mut().skip(1) {
            p.add_assign(&p_double);
            x.write(p);
        }

        Self(unsafe {
            core::mem::transmute::<
                [MaybeUninit<Jacobian>; ECMULT_TABLE_SIZE_A],
                [Jacobian; ECMULT_TABLE_SIZE_A],
            >(table)
        })
    }

    fn get(&self, n: i32) -> Jacobian {
        if n > 0 {
            self.0[(n - 1) as usize / 2]
        } else {
            -self.0[(-n - 1) as usize / 2]
        }
    }
}

#[cfg(test)]
mod test {
    use super::{ecmult, Scalar};

    use crate::secp256r1::{
        context::TABLE_G,
        field::FieldElement,
        points::{Jacobian, JacobianConst},
        test_vectors::MUL_TEST_VECTORS,
    };

    #[cfg(feature = "bigint_ops")]
    fn init() {
        crate::secp256r1::init();
        crate::bigint_delegation::init();
    }

    #[test]
    fn test_ecmult_mix() {
        #[cfg(feature = "bigint_ops")]
        init();

        assert_eq!(
            JacobianConst::GENERATOR.double().to_affine(),
            ecmult(Jacobian::GENERATOR, Scalar::ONE, Scalar::ONE, &TABLE_G).to_affine()
        );

        assert_eq!(
            JacobianConst::GENERATOR
                .double()
                .add(&JacobianConst::GENERATOR)
                .to_affine(),
            ecmult(
                Jacobian::GENERATOR,
                Scalar::from_words([2, 0, 0, 0]),
                Scalar::ONE,
                &TABLE_G
            )
            .to_affine()
        );

        assert_eq!(
            JacobianConst::GENERATOR
                .double()
                .add(&JacobianConst::GENERATOR)
                .to_affine(),
            ecmult(
                Jacobian::GENERATOR,
                Scalar::ONE,
                Scalar::from_words([2, 0, 0, 0]),
                &TABLE_G
            )
            .to_affine()
        );

        assert_eq!(
            JacobianConst::GENERATOR.double().double().to_affine(),
            ecmult(
                Jacobian::GENERATOR,
                Scalar::from_words([2, 0, 0, 0]),
                Scalar::from_words([2, 0, 0, 0]),
                &TABLE_G
            )
            .to_affine()
        );

        assert_eq!(
            JacobianConst::GENERATOR.double().double().to_affine(),
            ecmult(
                Jacobian::GENERATOR,
                Scalar::from_words([3, 0, 0, 0]),
                Scalar::ONE,
                &TABLE_G
            )
            .to_affine()
        );

        assert_eq!(
            JacobianConst::GENERATOR.double().double().to_affine(),
            ecmult(
                Jacobian::GENERATOR,
                Scalar::ONE,
                Scalar::from_words([3, 0, 0, 0]),
                &TABLE_G
            )
            .to_affine()
        );
    }

    #[test]
    fn test_ecmult() {
        #[cfg(feature = "bigint_ops")]
        init();

        for (k_bytes, x_bytes, y_bytes) in MUL_TEST_VECTORS {
            let k = Scalar::reduce_be_bytes(k_bytes);
            let expected = Jacobian {
                x: FieldElement::from_be_bytes(x_bytes).unwrap(),
                y: FieldElement::from_be_bytes(y_bytes).unwrap(),
                z: FieldElement::ONE,
            };

            let result = ecmult(Jacobian::default(), Scalar::ZERO, k, &TABLE_G);
            assert_eq!(result.to_affine(), expected.to_affine());

            let result = ecmult(Jacobian::GENERATOR, k, Scalar::ZERO, &TABLE_G);
            assert_eq!(result.to_affine(), expected.to_affine());
        }
    }
}
