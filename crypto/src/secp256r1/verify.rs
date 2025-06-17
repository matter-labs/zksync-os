use super::{
    context::{GeneratorMultiplesTable, TABLE_G},
    field::FieldElement,
    points::Affine,
    scalar::{Scalar, Signature},
    wnaf::Wnaf,
    Secp256r1Err, ECMULT_TABLE_SIZE_A, WINDOW_A, WINDOW_G};
use core::mem::MaybeUninit;

type Jacobian = super::points::Jacobian<FieldElement>;

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

    Ok(r == Scalar::reduce_be_bytes(&x.to_be_bytes()))
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

        for i in 1..ECMULT_TABLE_SIZE_A {
            p.add_assign(&p_double);
            table[i].write(p);
        }

        Self(unsafe { core::mem::transmute(table) })
    }

    fn get(&self, n: i32) -> Jacobian {
        if n > 0 {
            self.0[(n - 1) as usize / 2]
        } else {
            -self.0[(-n - 1) as usize / 2]
        }
    }
}
