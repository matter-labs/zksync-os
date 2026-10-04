use super::*;

#[test]
fn single_pair_error_paths_unchanged() {
    use Bls12PrecompileInterfaceError::{InvalidG1Point, PointNotInSubgroup};
    let mut rng = TestRng(3);
    let s = scalar(&rng.below_power_of_two(256));

    assert_rejected(&NOT_ON_CURVE, &s, InvalidG1Point);

    // a coordinate that is not canonical (x + q for the field modulus q), and one with a
    // non-zero byte in its padding
    let generator = encode_point(G1Affine::generator());
    let x = BigUint::from_bytes_be(&generator[16..FIELD_ELEMENT_SERIALIZATION_LEN])
        + BigUint::from(<Fq as PrimeField>::MODULUS);
    let mut not_canonical = generator;
    not_canonical[16..FIELD_ELEMENT_SERIALIZATION_LEN].copy_from_slice(&x.to_bytes_be());
    assert_rejected(&not_canonical, &s, InvalidG1Point);
    let mut padded = generator;
    padded[FIELD_ELEMENT_SERIALIZATION_LEN] = 1;
    assert_rejected(&padded, &s, InvalidG1Point);

    // points of the curve outside of G1, `[r]P != O` by a double-and-add on the integer `r`
    // (`mul_bigint` would reduce it to 0): the two points of order 3, random points (not
    // cofactor-cleared), and the generator plus a point of order 3
    let order_three = G1Affine::new_unchecked(Fq::ZERO, Fq::from(2u64));
    let mut points = vec![order_three, -order_three];
    while points.len() < 10 {
        let x = rng.fq();
        if let Some(p) = G1Affine::get_point_from_x_unchecked(x, rng.next_u64() & 1 == 1) {
            points.push(p);
        }
    }
    points.push((G1Affine::generator().into_group() + order_three).into_affine());
    let r = scalar(&group_order());
    for p in points {
        assert!(p.is_on_curve());
        // a random point of the curve is in G1 with probability 1 / cofactor, about 2^-126
        assert!(!sw_double_and_add_affine(&p, r).is_zero(), "{p:?} is in G1");
        assert_rejected(&encode_point(p), &s, PointNotInSubgroup);
    }
}

/// A sink that rejects every write
struct AlwaysFailDst;

impl TryExtend<u8> for AlwaysFailDst {
    type Error = ();

    fn try_extend<I>(&mut self, _iter: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = u8>,
    {
        Err(())
    }
}

#[test]
fn single_pair_charge_and_output_failures_unchanged() {
    let mut rng = TestRng(4);
    let s = scalar(&rng.below_power_of_two(256));

    // the charge fails before the invalid point is parsed
    let input = encode_pair(&NOT_ON_CURVE, &s);
    let mut resources = TestResources::from_ergs_and_native(
        Ergs::from_legacy_gas_saturating(BLS12_381_G1_MSM_PER_POINT_GAS - 1),
        DecreasingNative::FORMAL_INFINITE,
    );
    let (result, output) = execute(&input, &mut resources);
    let err = result.expect_err("gas must run out");
    assert!(
        matches!(err, SubsystemError::LeafRuntime(RuntimeError::OutOfErgs(_))),
        "{err:?}"
    );
    assert!(output.is_empty());

    let mut resources = TestResources::from_ergs_and_native(
        Ergs::FORMAL_INFINITE,
        DecreasingNative::from_computational(BLS12_381_G1MSM_PER_POINT_NATIVE_COST - 1),
    );
    let (result, output) = execute(&input, &mut resources);
    let err = result.expect_err("native must run out");
    assert!(
        matches!(
            err,
            SubsystemError::LeafRuntime(RuntimeError::FatalRuntimeError(
                FatalRuntimeError::OutOfNativeResources(_)
            ))
        ),
        "{err:?}"
    );
    assert!(output.is_empty());

    // the output fails after the charge
    let input = encode_pair(&encode_point(G1Affine::generator()), &s);
    let mut resources = TestResources::FORMAL_INFINITE;
    let err = Bls12381G1MSMPrecompile::<true>::execute(
        &input,
        &mut AlwaysFailDst,
        &mut resources,
        &mut bls_tests::honest_oracle(),
        &mut zk_ee::system::logger::NullLogger,
        Global,
    )
    .expect_err("the output must fail");
    assert!(
        matches!(
            err,
            SubsystemError::LeafRuntime(RuntimeError::FatalRuntimeError(
                FatalRuntimeError::OutOfReturnMemory(_)
            ))
        ),
        "{err:?}"
    );
    assert_eq!(resources, charged_for_one_pair());
}
