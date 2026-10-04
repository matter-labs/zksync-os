use super::super::tests::compare;
use super::*;
use alloc::vec::Vec;
use crypto::ark_ec::AffineRepr;
use crypto::ark_ff::AdditiveGroup;
use zk_ee::system::Resource;

#[test]
fn additions_preserve_curve_and_infinity_cases() {
    // Addition permits torsion points: it must not apply an MSM subgroup check.
    let torsion = G1Affine::new_unchecked(Fq::ZERO, Fq::from(2u64));
    let g = G1Affine::generator();
    let o = G1Affine::identity();
    for (a, b) in [
        (g, g),
        (g, -g),
        (g, o),
        (o, g),
        (o, o),
        (torsion, torsion),
        (g, torsion),
    ] {
        let mut input = Vec::new();
        write_g1(a, &mut input).unwrap();
        write_g1(b, &mut input).unwrap();
        let output = compare::<
            Bls12381G1AdditionPrecompile<false>,
            Bls12381G1AdditionPrecompile<true>,
        >(&input);
        let expected = (a + b).into_affine();
        assert_eq!(
            parse_g1(output.as_slice().try_into().unwrap()).unwrap(),
            expected
        );
    }
    let g = G2Affine::generator();
    let o = G2Affine::identity();
    for (a, b) in [(g, g), (g, -g), (g, o), (o, g), (o, o)] {
        let mut input = Vec::new();
        write_g2(a, &mut input).unwrap();
        write_g2(b, &mut input).unwrap();
        let output = compare::<
            Bls12381G2AdditionPrecompile<false>,
            Bls12381G2AdditionPrecompile<true>,
        >(&input);
        assert_eq!(
            parse_g2(output.as_slice().try_into().unwrap()).unwrap(),
            (a + b).into_affine()
        );
    }
}

#[test]
#[should_panic(expected = "wrong")]
fn wrong_g1_slope_cannot_change_addition() {
    let g = G1Affine::generator();
    let mut input = Vec::new();
    write_g1(g, &mut input).unwrap();
    write_g1(g, &mut input).unwrap();
    let mut oracle = curve_hints::tests::lying_oracle(&[
        crate::system_functions::field_ops::FieldHintOp::Bls12381BaseFieldDivision,
    ]);
    let mut resources = zk_ee::reference_implementations::BaseResources::<
        zk_ee::reference_implementations::DecreasingNative,
    >::FORMAL_INFINITE;
    let _ = Bls12381G1AdditionPrecompile::<true>::execute(
        &input,
        &mut Vec::new(),
        &mut resources,
        &mut oracle,
        &mut zk_ee::system::logger::NullLogger,
        std::alloc::Global,
    );
}
