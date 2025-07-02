use super::{
    points::{Affine, JacobianConst, Storage},
    ECMULT_TABLE_SIZE_G,
};

pub(super) struct GeneratorMultiplesTable([Storage; ECMULT_TABLE_SIZE_G]);

pub(super) const TABLE_G: GeneratorMultiplesTable = GeneratorMultiplesTable::new();

impl GeneratorMultiplesTable {
    const fn new() -> Self {
        let mut pre_g = [Storage::DEFAULT; ECMULT_TABLE_SIZE_G];
        let g = JacobianConst::GENERATOR;

        odd_multiples(&mut pre_g, &g);

        Self(pre_g)
    }

    pub(super) fn get_ge(&self, n: i32) -> Affine {
        if n > 0 {
            self.0[(n - 1) as usize / 2].to_affine()
        } else {
            -(self.0[(-n - 1) as usize / 2].to_affine())
        }
    }
}

const fn odd_multiples(table: &mut [Storage; ECMULT_TABLE_SIZE_G], gen: &JacobianConst) {
    use const_for::const_for;
    let mut gj = *gen;

    table[0] = gj.to_storage();

    let g_double = gen.double();

    const_for!(i in 1..ECMULT_TABLE_SIZE_G => {
        gj = gj.add(&g_double);
        table[i] = gj.to_storage();
    });
}
