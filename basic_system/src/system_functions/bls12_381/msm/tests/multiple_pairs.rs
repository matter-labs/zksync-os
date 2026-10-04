use super::*;

#[test]
fn affine_two_pair_and_larger_bucket_msm_match_integer_multiples() {
    let g = G1Affine::generator();
    let scalars = [
        Scalar::new([u64::MAX; 4]),
        scalar(&(group_order() + 1u32)),
        scalar(&BigUint::from(0u32)),
    ];
    for n in [2, 3] {
        let points = [g, -g, g];
        let mut input = Vec::new();
        let mut expected = G1Projective::zero();
        for i in 0..n {
            input.extend(encode_pair(&encode_point(points[i]), &scalars[i]));
            expected += sw_double_and_add_affine(&points[i], scalars[i].as_ref());
        }
        let mut resources = TestResources::FORMAL_INFINITE;
        let (result, output) = execute(&input, &mut resources);
        result.unwrap();
        assert_eq!(
            parse_g1(output.as_slice().try_into().unwrap()).unwrap(),
            expected.into_affine()
        );
    }
}

#[test]
fn g2_msm_normalization_and_subgroup_hints_preserve_results() {
    let g = G2Affine::generator();
    for points in [vec![g], vec![g, -g], vec![g, g, G2Affine::identity()]] {
        let mut input = Vec::new();
        for point in points {
            write_g2(point, &mut input).unwrap();
            input.extend([0xffu8; SCALAR_SERIALIZATION_LEN]);
        }
        bls_tests::compare::<Bls12381G2MSMPrecompile<false>, Bls12381G2MSMPrecompile<true>>(&input);
    }
}
