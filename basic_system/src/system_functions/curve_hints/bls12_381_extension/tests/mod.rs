use super::*;
use crypto::affine_glv::Divider;

use oracle_provider::ZkEENonDeterminismSource;

fn oracle() -> ZkEENonDeterminismSource {
    let mut oracle = ZkEENonDeterminismSource::default();
    oracle.add_external_processor(callable_oracles::field_hints::NativeFieldOpsQuery);
    oracle
}

fn inputs() -> Vec<Fq2> {
    let mut values = vec![Fq2::ZERO, Fq2::ONE, -Fq2::ONE];
    for i in 1u64..=24 {
        values.push(Fq2::new(Fq::from(i), Fq::ZERO));
        values.push(Fq2::new(Fq::ZERO, Fq::from(i)));
        values.push(Fq2::new(Fq::from(i * 17), Fq::from(i * i + 5)));
    }
    values
}

#[test]
fn inverses_and_divisions_match_field_arithmetic() {
    let mut oracle = oracle();
    for value in inputs() {
        assert_eq!(bls12_381_fq2_inverse(&mut oracle, &value), value.inverse());
        if !value.is_zero() {
            let numerator = Fq2::new(Fq::from(19u64), Fq::from(31u64));
            let mut slot = MaybeUninit::uninit();
            let mut fraction = [numerator, value];
            let actual = Bls12381Fq2Divider::new(&mut oracle).divide(&mut fraction, &mut slot);
            assert_eq!(*actual * value, numerator);
        }
    }
}

#[test]
fn roots_and_nonresidues_match_field_arithmetic() {
    let mut oracle = oracle();
    let mut nonsquares = 0;
    for value in inputs() {
        for value in [value, value.square()] {
            let root = bls12_381_fq2_sqrt(&mut oracle, &value);
            assert_eq!(root.is_some(), value.sqrt().is_some(), "{value:?}");
            if let Some(root) = root {
                assert_eq!(root.square(), value);
            } else {
                nonsquares += 1;
            }
        }
    }
    assert!(nonsquares > 0);
}

#[test]
#[should_panic(expected = "the field inverse hint is wrong")]
fn false_inverse_hint_is_rejected() {
    use crate::system_functions::{curve_hints::tests::lying_oracle, field_ops::FieldHintOp};
    let mut oracle = lying_oracle(&[FieldHintOp::Bls12381BaseFieldInverse]);
    let _ = bls12_381_fq2_inverse(&mut oracle, &Fq2::new(Fq::from(2u64), Fq::from(3u64)));
}

#[test]
#[should_panic]
fn false_sqrt_hint_is_rejected() {
    use crate::system_functions::{curve_hints::tests::lying_oracle, field_ops::FieldHintOp};
    let mut oracle = lying_oracle(&[FieldHintOp::Bls12381BaseFieldSqrt]);
    let _ = bls12_381_fq2_sqrt(&mut oracle, &Fq2::new(Fq::from(2u64), Fq::from(3u64)));
}
