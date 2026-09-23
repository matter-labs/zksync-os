use basic_system::system_functions::curve_hints::{decode, encode, HintEncoding};
use crypto::ark_ff::{Field, One, Zero};
use crypto::k256::{Scalar, U256};
use crypto::secp256k1::field::FieldElement;
use zk_ee::utils::Bytes32;

/// Computes the square root candidate for a secp256k1 base field element.
///
/// Returns `(candidate, is_quadratic_non_residue)` where:
/// - `candidate` is `input^((p+1)/4)` (the square root if one exists)
/// - `is_quadratic_non_residue` is `true` if `input` has no square root in the field
///
/// When `is_quadratic_non_residue` is false: `candidate² == input`
/// When `is_quadratic_non_residue` is true:  `candidate² == -input`
pub(crate) fn secp256k1_base_field_sqrt(input: Bytes32) -> (Bytes32, bool) {
    // NOTE: input is in normal form
    let el = FieldElement::from_bytes(input.as_u8_array_ref()).expect("must be normalized");
    assert!(el.normalizes_to_zero() == false);
    let mut candidate = el;
    // sqrt_in_place returns true if the input is a quadratic residue (has a square root)
    let is_quadratic_residue = candidate.sqrt_in_place();
    (
        Bytes32::from_array(candidate.to_bytes().into()),
        !is_quadratic_residue,
    )
}

pub(crate) fn secp256k1_base_field_inverse(input: Bytes32) -> Bytes32 {
    // NOTE: input is in normal form
    let mut el = FieldElement::from_bytes(input.as_u8_array_ref()).expect("must be normalized");
    assert!(el.normalizes_to_zero() == false);
    el.invert_in_place();
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

/// The element in the hint encoding, as `N` words
fn to_words<F: HintEncoding, const N: usize>(value: &F) -> [Bytes32; N] {
    assert_eq!(4 * N, F::TOTAL_LIMBS);
    let mut limbs = vec![0u64; F::TOTAL_LIMBS];
    encode(value, &mut limbs);
    let mut words = [Bytes32::ZERO; N];
    for (word, chunk) in words.iter_mut().zip(limbs.as_chunks::<4>().0) {
        let mut bytes = [0u8; 32];
        for (dst, limb) in bytes.as_chunks_mut::<8>().0.iter_mut().zip(chunk) {
            *dst = limb.to_le_bytes();
        }
        *word = Bytes32::from_array(bytes);
    }
    words
}

/// The element from its hint encoding at the start of `bytes`. The operand comes from the
/// guest's own code, so a non-canonical one is a bug.
fn from_bytes<F: HintEncoding>(bytes: &[u8]) -> F {
    let limbs: Vec<u64> = bytes[..8 * F::TOTAL_LIMBS]
        .as_chunks::<8>()
        .0
        .iter()
        .map(|chunk| u64::from_le_bytes(*chunk))
        .collect();
    decode(&limbs).expect("the operand is a canonical field element")
}

/// The inverse of a non-zero field element, in `N` words
pub(crate) fn inverse<F: HintEncoding, const N: usize>(bytes: &[u8]) -> [Bytes32; N] {
    let el: F = from_bytes(bytes);
    let inverse = el.inverse().expect("the operand is non-zero");
    to_words(&inverse)
}

/// The square root candidate of a bls12-381 base field element, as `secp256k1_base_field_sqrt`:
/// `(candidate, is_quadratic_non_residue)` with `candidate² == input` for a square and
/// `candidate² == -input` otherwise (`p = 3 mod 4`, so exactly one of them is a square)
pub(crate) fn bls12_381_base_field_sqrt(bytes: &[u8]) -> ([Bytes32; 2], bool) {
    let el: crypto::bls12_381::Fq = from_bytes(bytes);
    assert!(!el.is_zero());
    match el.sqrt() {
        Some(root) => (to_words(&root), false),
        None => {
            let root = (-el).sqrt().expect("-1 is not a square, so -el is one");
            (to_words(&root), true)
        }
    }
}

/// The claim about the bn254 pairing product over the encoded affine pairs (6 base field
/// elements each), see `curve_hints::PairingClaim`: the identity flag, then `c`, `d = c^-1`
/// and the scaling factor (`crypto::residue_witness::bn254`) for an identity, or the inverse
/// of the Miller loop output and zeros otherwise. `claim_not_identity` forces the latter (for
/// tests of the exact path).
pub(crate) fn bn254_pairing_residue_witness(
    bytes: &[u8],
    claim_not_identity: bool,
) -> (bool, ([Bytes32; 12], ([Bytes32; 12], [Bytes32; 6]))) {
    use crypto::ark_ec::pairing::{MillerLoopOutput, Pairing};
    use crypto::bn254::curves::Bn254;
    use crypto::bn254::{Fq, Fq12, Fq2, Fq6, G1Affine, G2Affine};
    let fq = |i: usize| -> Fq { from_bytes(&bytes[32 * i..]) };
    let pairs: Vec<(G1Affine, G2Affine)> = (0..bytes.len() / 192)
        .map(|k| {
            let e = |i: usize| fq(6 * k + i);
            (
                G1Affine::new_unchecked(e(0), e(1)),
                G2Affine::new_unchecked(Fq2::new(e(2), e(3)), Fq2::new(e(4), e(5))),
            )
        })
        .collect();
    let f = Bn254::multi_miller_loop(
        pairs.iter().map(|(g1, _)| g1),
        pairs.iter().map(|(_, g2)| g2),
    )
    .0;
    let is_identity = !claim_not_identity
        && Bn254::final_exponentiation(MillerLoopOutput(f))
            .expect("non-zero")
            .0
            .is_one();
    if is_identity {
        let (c, d, s) = crypto::residue_witness::bn254::witness(&f)
            .expect("the final exponentiation found an identity, which has a witness");
        (true, (to_words(&c), (to_words(&d), to_words(&s))))
    } else {
        let f_inverse = f.inverse().expect("non-zero");
        (
            false,
            (
                to_words(&f_inverse),
                (to_words(&Fq12::zero()), to_words(&Fq6::zero())),
            ),
        )
    }
}

/// The claim about `e(p1, G2) e(p2, tau G2)` for the encoded affine `p1, p2`, as
/// `bn254_pairing_residue_witness`: the identity flag, then `d` and the scaling factor
/// (`crypto::residue_witness::bls12_381`) for an identity, or the inverse of the Miller loop
/// output and zeros otherwise
pub(crate) fn bls12_381_kzg_residue_witness(
    bytes: &[u8],
    claim_not_identity: bool,
) -> (bool, ([Bytes32; 24], [Bytes32; 12])) {
    use crypto::ark_ec::pairing::Pairing;
    use crypto::ark_ff::One;
    use crypto::bls12_381::curves::Bls12_381;
    use crypto::bls12_381::{Fq, Fq12, Fq6, G1Affine};
    let fq = |i: usize| -> Fq { from_bytes(&bytes[64 * i..]) };
    let p1 = G1Affine::new_unchecked(fq(0), fq(1));
    let p2 = G1Affine::new_unchecked(fq(2), fq(3));
    let g2 = [
        crypto::bls12_381::consts::PREPARED_G2_GENERATOR,
        crypto::bls12_381::consts::PREPARED_G2_BY_TAU,
    ];
    let f = Bls12_381::multi_miller_loop_with_initial(&Fq12::one(), [p1, p2], g2.clone());
    let is_identity = !claim_not_identity
        && Bls12_381::final_exponentiation(Bls12_381::multi_miller_loop([p1, p2], g2))
            .expect("non-zero")
            .0
            .is_one();
    if is_identity {
        let (d, s) = crypto::residue_witness::bls12_381::witness(&f)
            .expect("the final exponentiation found an identity, which has a witness");
        (true, (to_words(&d), to_words(&s)))
    } else {
        // the inverse of the output of the plain (conjugated) loop, which the exact path runs
        let mut conjugated = f;
        conjugated.conjugate_in_place();
        let f_inverse = conjugated.inverse().expect("non-zero");
        (false, (to_words(&f_inverse), to_words(&Fq6::zero())))
    }
}
