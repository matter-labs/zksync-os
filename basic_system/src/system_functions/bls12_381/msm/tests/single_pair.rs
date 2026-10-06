use super::*;

#[test]
fn single_pair_msm_matches_reference() {
    let mut rng = TestRng(1);
    let scalars = test_scalars(&mut rng);
    for p in test_points(&mut rng, 6) {
        for s in &scalars {
            let generic = msm::<G1Projective, _>(&[p], vec![*s], Global).into_affine();
            let reference = sw_double_and_add_affine(&p, s).into_affine();
            assert_eq!(generic, reference, "msm at {p:?} * {s}");
            assert_eq!(encode_point(generic), encode_point(reference));
        }
    }
}

#[test]
fn single_pair_precompile_output_matches() {
    let mut rng = TestRng(2);
    let scalars = test_scalars(&mut rng);
    let points = test_points(&mut rng, 2);
    let t = scalar(&rng.below_power_of_two(256));
    let infinity_pair = encode_pair(&[0u8; G1_SERIALIZATION_LEN], &t);
    for p in points {
        for s in &scalars {
            let input = encode_pair(&encode_point(p), s);
            let mut resources = TestResources::FORMAL_INFINITE;
            let (result, output) = execute(&input, &mut resources);
            result.unwrap();
            let expected = encode_point(sw_double_and_add_affine(&p, s).into_affine());
            assert_eq!(output, expected, "{p:?} * {s}");
            assert_eq!(resources, charged_for_one_pair());

            // the same product through `msm`: a second pair that adds the point at infinity
            let mut two_pairs = input;
            two_pairs.extend_from_slice(&infinity_pair);
            let mut resources = TestResources::FORMAL_INFINITE;
            let (result, generic) = execute(&two_pairs, &mut resources);
            result.unwrap();
            assert_eq!(output, generic, "{p:?} * {s}");
        }
    }
}
