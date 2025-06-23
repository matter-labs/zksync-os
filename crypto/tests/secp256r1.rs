use crypto::{
    p256::{
        ecdsa::{
            signature::hazmat::{PrehashSigner, PrehashVerifier},
            Signature, SigningKey, VerifyingKey,
        },
        elliptic_curve::{rand_core::OsRng, sec1::ToEncodedPoint},
    },
    secp256r1::verify,
    sha3::{Digest, Keccak256},
};
use proptest::prelude::*;

fn split_signature(sig: &Signature) -> ([u8; 32], [u8; 32]) {
    let r_bytes = sig.r().to_bytes();
    let s_bytes = sig.s().to_bytes();
    (r_bytes.into(), s_bytes.into())
}

fn split_public_key(pk: &VerifyingKey) -> Option<([u8; 32], [u8; 32])> {
    let encoded_point = pk.as_affine().to_encoded_point(false);

    match encoded_point.coordinates() {
        p256::elliptic_curve::sec1::Coordinates::Uncompressed { x, y } => {
            let x = *x;
            let y = *y;
            Some((x.into(), y.into()))
        }
        _ => None,
    }
}

#[test]
fn selftest() {
    proptest!(|(msg: [u8; 100])| {
            let digest = {
                let mut hasher = Keccak256::new();
                hasher.update(&msg);
                let res = hasher.finalize();
                let mut hash_bytes = [0u8; 32];
                hash_bytes.copy_from_slice(&res);
                hash_bytes
            };

            let signing_key = SigningKey::random(&mut OsRng);
            let verify_key = signing_key.verifying_key();
            let sig: Signature = signing_key.sign_prehash(&digest).unwrap();

            // sanity check
            prop_assert!(verify_key.verify_prehash(&digest, &sig).is_ok());

            let (r_bytes, s_bytes) = split_signature(&sig);
            let (x_bytes, y_bytes) = split_public_key(&verify_key).unwrap();

            let result = verify(&digest, &r_bytes, &s_bytes, &x_bytes, &y_bytes);

            prop_assert!(result.unwrap());
    })
}

#[test]
fn bad_message() {
    proptest!(|(msg: [u8; 10], bad_msg: [u8; 10])| {
            if msg != bad_msg {
                let digest = {
                    let mut hasher = Keccak256::new();
                    hasher.update(&msg);
                    let res = hasher.finalize();
                    let mut hash_bytes = [0u8; 32];
                    hash_bytes.copy_from_slice(&res);
                    hash_bytes
                };

                let bad_digest = {
                    let mut hasher = Keccak256::new();
                    hasher.update(&bad_msg);
                    let res = hasher.finalize();
                    let mut hash_bytes = [0u8; 32];
                    hash_bytes.copy_from_slice(&res);
                    hash_bytes
                };

                let signing_key = SigningKey::random(&mut OsRng);
                let verify_key = signing_key.verifying_key();
                let sig: Signature = signing_key.sign_prehash(&digest).unwrap();

                // sanity check
                prop_assert!(verify_key.verify_prehash(&bad_digest, &sig).is_err());

                let (r_bytes, s_bytes) = split_signature(&sig);
                let (x_bytes, y_bytes) = split_public_key(&verify_key).unwrap();

                let result = verify(&bad_digest, &r_bytes, &s_bytes, &x_bytes, &y_bytes);
                prop_assert!(!result.unwrap());
            }
    })
}

#[test]
fn bad_signature() {
    proptest!(|(msg: [u8; 100], sig: [u8; 64])| {
            let digest = {
                let mut hasher = Keccak256::new();
                hasher.update(&msg);
                let res = hasher.finalize();
                let mut hash_bytes = [0u8; 32];
                hash_bytes.copy_from_slice(&res);
                hash_bytes
            };

            let signing_key = SigningKey::random(&mut OsRng);
            let verify_key = signing_key.verifying_key();
            let sig = Signature::from_bytes(&sig.into()).unwrap();

            if sig != signing_key.sign_prehash(&digest).unwrap() {
                // sanity check
                prop_assert!(verify_key.verify_prehash(&digest, &sig).is_err());

                let (r_bytes, s_bytes) = split_signature(&sig);
                let (x_bytes, y_bytes) = split_public_key(&verify_key).unwrap();

                let result = verify(&digest, &r_bytes, &s_bytes, &x_bytes, &y_bytes);

                prop_assert!(!result.unwrap());
            }
    })
}

#[test]
fn bad_signing_key() {
    proptest!(|(msg: [u8; 100])| {
            let digest = {
                let mut hasher = Keccak256::new();
                hasher.update(&msg);
                let res = hasher.finalize();
                let mut hash_bytes = [0u8; 32];
                hash_bytes.copy_from_slice(&res);
                hash_bytes
            };

            let signing_key = SigningKey::random(&mut OsRng);
            let bad_signing_key = SigningKey::random(&mut OsRng);

            if signing_key != bad_signing_key {
                let bad_verify_key = bad_signing_key.verifying_key();
                let sig: Signature = signing_key.sign_prehash(&digest).unwrap();

                // sanity check
                prop_assert!(bad_verify_key.verify_prehash(&digest, &sig).is_err());

                let (r_bytes, s_bytes) = split_signature(&sig);
                let (x_bytes, y_bytes) = split_public_key(&bad_verify_key).unwrap();

                let result = verify(&digest, &r_bytes, &s_bytes, &x_bytes, &y_bytes);

                prop_assert!(!result.unwrap());
            }
    })
}
