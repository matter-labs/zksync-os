use core::mem::MaybeUninit;
use super::{field::FieldElementConst, points::{Affine, Jacobian, Storage}, ECMULT_TABLE_SIZE_G};

pub(super) struct GeneratorMultiplesTable([Storage; ECMULT_TABLE_SIZE_G]);

pub(super) const TABLE_G: GeneratorMultiplesTable = GeneratorMultiplesTable::new();

impl GeneratorMultiplesTable {
    const fn new() -> Self {

        let mut pre_g = [const { MaybeUninit::uninit() }; ECMULT_TABLE_SIZE_G];
        let g = Jacobian::<FieldElementConst>::GENERATOR;

        odd_multiples(&mut pre_g, &g);

        unsafe { Self(core::mem::transmute(pre_g)) }
    }

    pub(super) fn get_ge(&self, n: i32) -> Affine {
        if n > 0 {
            self.0[(n - 1) as usize / 2].to_affine()
        } else {
            -(self.0[(-n - 1) as usize / 2].to_affine())
        }
    }
}

const fn odd_multiples(table: &mut [MaybeUninit<Storage>; ECMULT_TABLE_SIZE_G], gen: &Jacobian<FieldElementConst>) {
    use const_for::const_for;
    let mut gj = *gen;

    table[0].write(gj.to_storage());

    let g_double = gen.double();

    const_for!(i in 1..ECMULT_TABLE_SIZE_G => {
        gj = gj.add(&g_double);
        table[i].write(gj.to_storage());
    });
}