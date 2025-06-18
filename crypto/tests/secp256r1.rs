use crypto::secp256r1::verify;
use p256::{
    ecdsa::{signature::{Signer, Verifier}, Signature, SigningKey, VerifyingKey},
    elliptic_curve::{rand_core::OsRng, sec1::ToEncodedPoint, FieldBytesEncoding},
};
use proptest::prelude::*;
use sha3::Digest;

fn split_signature(sig: &Signature) -> ([u8; 32], [u8; 32]) {
    let r_bytes = sig.r().to_bytes();
    let s_bytes = sig.s().to_bytes();
    (r_bytes.into(), s_bytes.into())
}

fn split_public_key(pk: &VerifyingKey) -> Option<([u8; 32], [u8; 32])> {
    let encoded_point = pk.as_affine().to_encoded_point(false);

    match encoded_point.coordinates() {
        k256::elliptic_curve::sec1::Coordinates::Uncompressed { x, y } => {
            let x = *x;
            let y = *y;
            Some((x.into(), y.into()))
        }
        _ => None
    }
}

#[test]
fn selftest() {
    proptest!(|(digest: [u8; 32])| {
        let signing_key = SigningKey::random(&mut OsRng);
        let verify_key = signing_key.verifying_key();
        let sig: Signature = signing_key.sign(&digest);

        prop_assert!(verify_key.verify(&digest, &sig).is_ok());

        let (r_bytes, s_bytes) = split_signature(&sig);
        let (x_bytes, y_bytes) = split_public_key(&verify_key).unwrap();

        let result = verify(&digest, &r_bytes, &s_bytes, &x_bytes, &y_bytes);

        prop_assert!(result.unwrap());
})
}