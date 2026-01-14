use crypto::k256::{Scalar, U256};
use crypto::secp256k1::FieldElement;
use zk_ee::utils::Bytes32;

pub(crate) fn secp256k1_base_field_sqrt(input: Bytes32) -> (Bytes32, bool) {
    // NOTE: input is in normal form
    let el = FieldElement::from_bytes(input.as_u8_array_ref()).expect("must be normalized");
    assert!(el.normalizes_to_zero() == false);
    let mut candidate = el;
    let is_square_root = candidate.sqrt_in_place_inner();
    (
        Bytes32::from_array(candidate.to_bytes().into()),
        !is_square_root,
    )
}

pub(crate) fn secp256k1_base_field_inverse(input: Bytes32) -> Bytes32 {
    // NOTE: input is in normal form
    let mut el = FieldElement::from_bytes(input.as_u8_array_ref()).expect("must be normalized");
    assert!(el.normalizes_to_zero() == false);
    el.invert_in_place_inner();
    Bytes32::from_array(el.to_bytes().into())
}

pub(crate) fn secp256k1_scalar_field_inverse(input: Bytes32) -> Bytes32 {
    use crypto::k256::elliptic_curve::ops::Invert;
    use crypto::k256::elliptic_curve::scalar::FromUintUnchecked;
    use crypto::k256::elliptic_curve::Curve;

    // NOTE: input is in normal form
    let el = U256::from_be_slice(input.as_u8_array_ref());
    assert!(el < crypto::k256::Secp256k1::ORDER);
    let scalar: Scalar = Scalar::from_uint_unchecked(el);
    let inverse = scalar.invert_vartime().unwrap();

    Bytes32::from_array(inverse.to_bytes().into())
}
