use super::{Fq, Fq2, Fq2Config};
#[cfg(any(all(target_arch = "riscv32", feature = "bigint_ops"), test))]
use crate::ark_ff_delegation::MontFp;
#[cfg(not(any(all(target_arch = "riscv32", feature = "bigint_ops"), test)))]
use ark_ff::MontFp;
use ark_ff::{AdditiveGroup, Field, Fp2Config, Fp6, Fp6Config};

pub type Fq6 = Fp6<Fq6Config>;

#[derive(Clone, Copy)]
pub struct Fq6Config;

impl Fp6Config for Fq6Config {
    type Fp2Config = Fq2Config;

    /// NONRESIDUE = U+9
    const NONRESIDUE: Fq2 = Fq2::new(MontFp!("9"), Fq::ONE);

    const FROBENIUS_COEFF_FP6_C1: &'static [Fq2] = &[
        // Fp2::NONRESIDUE^(((q^0) - 1) / 3)
        Fq2::new(Fq::ONE, Fq::ZERO),
        // Fp2::NONRESIDUE^(((q^1) - 1) / 3)
        Fq2::new(
            MontFp!(
                "21575463638280843010398324269430826099269044274347216827212613867836435027261"
            ),
            MontFp!(
                "10307601595873709700152284273816112264069230130616436755625194854815875713954"
            ),
        ),
        // Fp2::NONRESIDUE^(((q^2) - 1) / 3)
        Fq2::new(
            MontFp!(
                "21888242871839275220042445260109153167277707414472061641714758635765020556616"
            ),
            Fq::ZERO,
        ),
        // Fp2::NONRESIDUE^(((q^3) - 1) / 3)
        Fq2::new(
            MontFp!("3772000881919853776433695186713858239009073593817195771773381919316419345261"),
            MontFp!("2236595495967245188281701248203181795121068902605861227855261137820944008926"),
        ),
        // Fp2::NONRESIDUE^(((q^4) - 1) / 3)
        Fq2::new(
            MontFp!("2203960485148121921418603742825762020974279258880205651966"),
            Fq::ZERO,
        ),
        // Fp2::NONRESIDUE^(((q^5) - 1) / 3)
        Fq2::new(
            MontFp!(
                "18429021223477853657660792034369865839114504446431234726392080002137598044644"
            ),
            MontFp!("9344045779998320333812420223237981029506012124075525679208581902008406485703"),
        ),
    ];

    const FROBENIUS_COEFF_FP6_C2: &'static [Fq2] = &[
        // Fp2::NONRESIDUE^((2*(q^0) - 2) / 3)
        Fq2::new(Fq::ONE, Fq::ZERO),
        // Fp2::NONRESIDUE^((2*(q^1) - 2) / 3)
        Fq2::new(
            MontFp!("2581911344467009335267311115468803099551665605076196740867805258568234346338"),
            MontFp!(
                "19937756971775647987995932169929341994314640652964949448313374472400716661030"
            ),
        ),
        // Fp2::NONRESIDUE^((2*(q^2) - 2) / 3)
        Fq2::new(
            MontFp!("2203960485148121921418603742825762020974279258880205651966"),
            Fq::ZERO,
        ),
        // Fp2::NONRESIDUE^((2*(q^3) - 2) / 3)
        Fq2::new(
            MontFp!("5324479202449903542726783395506214481928257762400643279780343368557297135718"),
            MontFp!(
                "16208900380737693084919495127334387981393726419856888799917914180988844123039"
            ),
        ),
        // Fp2::NONRESIDUE^((2*(q^4) - 2) / 3)
        Fq2::new(
            MontFp!(
                "21888242871839275220042445260109153167277707414472061641714758635765020556616"
            ),
            Fq::ZERO,
        ),
        // Fp2::NONRESIDUE^((2*(q^5) - 2) / 3)
        Fq2::new(
            MontFp!(
                "13981852324922362344252311234282257507216387789820983642040889267519694726527"
            ),
            MontFp!("7629828391165209371577384193250820201684255241773809077146787135900891633097"),
        ),
    ];

    #[inline(always)]
    fn mul_fp2_by_nonresidue_in_place(fe: &mut Fq2) -> &mut Fq2 {
        // (c0+u*c1)*(9+u) = (9*c0-c1)+u*(9*c1+c0)
        let mut f = *fe;
        f.double_in_place().double_in_place().double_in_place();
        let mut c0 = fe.c1;
        Fq2Config::mul_fp_by_nonresidue_in_place(&mut c0);
        c0 += &f.c0;
        c0 += &fe.c0;
        let c1 = f.c1 + fe.c1 + fe.c0;
        *fe = Fq2::new(c0, c1);
        fe
    }
}
