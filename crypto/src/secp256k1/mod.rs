#![allow(long_running_const_eval)]
#![allow(clippy::precedence)]

mod context;
mod field;
mod points;
mod recover;
mod scalars;

#[cfg(test)]
mod test_vectors;

use core::fmt::Debug;
use core::fmt::Display;

pub use context::ECMultContext;
pub use recover::recover_with_context;

#[cfg(feature = "secp256k1-static-context")]
pub use recover::recover;

#[cfg(any(all(target_arch = "riscv32", feature = "bigint_ops"), test))]
pub fn init() {
    scalars::init();
    field::init();
}

#[derive(Debug, PartialEq)]
pub enum Secp256k1Err {
    OperationOverflow,
    InvalidParams,
    RecoveredInfinity,
}

impl Display for Secp256k1Err {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::OperationOverflow => write!(f, "secp256k1: restoring x-coordinate overflowed"),
            Self::InvalidParams => write!(
                f,
                "secp256k1: could not decompress signature to curve point"
            ),
            Self::RecoveredInfinity => write!(f, "secp256k1: recovered the point at infinity"),
        }
    }
}

#[cfg(feature = "secp256k1-static-context")]
pub fn ecrecover_test() {
    #[cfg(feature = "bigint_ops")]
    init();

    use crate::k256::{
        ecdsa::{hazmat::bits2field, SigningKey},
        elliptic_curve::{group::GroupEncoding, ops::Reduce},
        Scalar,
    };
    use crate::sha3::{Digest, Keccak256};
    let message = "In the beginning the Universe was created.
    This had made many people very angry and has been widely regarded as a bad move";
    let private_key = SigningKey::from_bytes(
        &[
            136, 84, 181, 46, 13, 86, 203, 113, 63, 17, 137, 177, 95, 211, 104, 70, 112, 232, 200,
            156, 225, 27, 123, 207, 243, 114, 4, 216, 148, 242, 81, 154,
        ]
        .into(),
    )
    .unwrap();
    let digest = {
        let mut hasher = Keccak256::new();
        hasher.update(message);
        let res = hasher.finalize();
        let mut hash_bytes = [0u8; 32];
        hash_bytes.copy_from_slice(&res);
        hash_bytes
    };

    let public_key = private_key.verifying_key().as_affine();

    let (signature, recovery_id) = private_key.sign_prehash_recoverable(&digest).unwrap();
    let msg = <Scalar as Reduce<crate::k256::U256>>::reduce_bytes(
        &bits2field::<crate::k256::Secp256k1>(&digest)
            .map_err(|_| ())
            .unwrap(),
    );

    let recovered_key = recover(&msg, &signature, &recovery_id).unwrap();

    assert_eq!(recovered_key.to_bytes(), public_key.to_bytes());
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn run_outside() {
        ecrecover_test();
    }
}
