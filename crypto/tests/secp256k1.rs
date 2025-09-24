use zksync_os_crypto::secp256k1::recover;
use zksync_os_crypto::{
    k256::elliptic_curve::{ops::Reduce, rand_core::OsRng},
    k256::{
        ecdsa::{hazmat::bits2field, SigningKey},
        elliptic_curve::group::GroupEncoding,
        Scalar,
    },
    sha3::{Digest, Keccak256},
};

use proptest::{
    arbitrary::Mapped,
    prelude::{any, Arbitrary, BoxedStrategy, Just, Strategy},
    prop_assert_eq, proptest,
};

#[derive(Debug)]
struct Message(String);

impl Arbitrary for Message {
    type Parameters = ();

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        any::<String>().prop_map(Message)
    }

    type Strategy = Mapped<String, Message>;
}

#[derive(Debug, Clone)]
struct PrivateKey(SigningKey);

impl Arbitrary for PrivateKey {
    type Parameters = ();

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        Just(PrivateKey(SigningKey::random(&mut OsRng))).boxed()
    }

    type Strategy = BoxedStrategy<Self>;
}

#[test]
fn selftest() {
    #[cfg(feature = "bigint_ops")]
    crypto::init_lib();

    proptest!(|(message: Message, private_key: PrivateKey)| {
        let message = message.0;
        let private_key = private_key.0;
        let digest = {
            let mut hasher = Keccak256::new();
            hasher.update(&message);
            let res = hasher.finalize();
            let mut hash_bytes = [0u8; 32];
            hash_bytes.copy_from_slice(&res);
            hash_bytes
        };

        let public_key = private_key.verifying_key().as_affine();

        let (signature, recovery_id) = private_key.sign_prehash_recoverable(&digest).unwrap();
        let msg = <Scalar as Reduce<zksync_os_crypto::k256::U256>>::reduce_bytes(
            &bits2field::<zksync_os_crypto::k256::Secp256k1>(&digest)
                .map_err(|_| ())
                .unwrap(),
        );

        let recovered_key = recover(&msg, &signature, &recovery_id).unwrap();

        prop_assert_eq!(recovered_key.to_bytes(), public_key.to_bytes());
    })
}
