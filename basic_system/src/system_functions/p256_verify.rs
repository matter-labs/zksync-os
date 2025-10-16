use super::*;

use crate::cost_constants::{P256_NATIVE_COST, P256_VERIFY_COST_ERGS};
use zk_ee::common_traits::TryExtend;
use zk_ee::out_of_return_memory;
use zk_ee::system::{
    base_system_functions::{P256VerifyErrors, SystemFunction},
    errors::subsystem::SubsystemError,
    Computational,
};

///
/// p256 verify system function implementation.
/// Follows the spec in: https://eips.ethereum.org/EIPS/eip-7951
///
pub struct P256VerifyImpl;

impl<R: Resources> SystemFunction<R, P256VerifyErrors> for P256VerifyImpl {
    fn execute<D: TryExtend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        src: &[u8],
        dst: &mut D,
        resources: &mut R,
        _: A,
    ) -> Result<(), SubsystemError<P256VerifyErrors>> {
        cycle_marker::wrap_with_resources!("p256_verify", resources, {
            p256_verify_as_system_function_inner(src, dst, resources)
        })
    }
}

///
/// Return value should be 1 if successful or empty in any other case:
///  - signature verification failure
///  - input is invalid
///
/// Input is considered invalid if:
///  1. input length is not 160
///  2. invalid field encoding (overflow p)
///  3. signature components are out of the following bounds: 0 < r < n and 0 < s < n
///  4. point not in curve, (x,y) should satisfy = y^2 ≡ x^3 + a*x + b (mod p)
///  5. point (x, y) must no be infinity (represented as (0,0))
///
///  Post checks:
///  6. modular comp r' ≡ r (mod n) for recovered
///  7. recovered is not infinity
///
///
///  2-7 are checked internally by crypto crate.
///
fn p256_verify_as_system_function_inner<
    S: ?Sized + MinimalByteAddressableSlice,
    D: ?Sized + TryExtend<u8>,
    R: Resources,
>(
    src: &S,
    dst: &mut D,
    resources: &mut R,
) -> Result<(), SubsystemError<P256VerifyErrors>> {
    let native = <R as Resources>::Native::from_computational(P256_NATIVE_COST);
    resources.charge(&R::from_ergs_and_native(P256_VERIFY_COST_ERGS, native))?;

    if src.len() != 160 {
        // Empty returndata indicates failure.
        return Ok(());
    }

    // digest, r, s, x, y
    let mut buffer = [0u8; 160];
    for (dst, src) in buffer.iter_mut().zip(src.iter()) {
        *dst = *src;
    }

    let mut it = buffer.array_chunks::<32>();
    let is_valid = unsafe {
        let digest = it.next().unwrap_unchecked();
        let r = it.next().unwrap_unchecked();
        let s = it.next().unwrap_unchecked();
        let x = it.next().unwrap_unchecked();
        let y = it.next().unwrap_unchecked();

        let Ok(result) = secp256r1_verify_inner(digest, r, s, x, y) else {
            // Empty returndata indicates failure.
            return Ok(());
        };

        result
    };

    // Only set return data if valid, otherwise it should be empty
    if is_valid {
        dst.try_extend(ruint::aliases::U256::ONE.to_be_bytes::<32>())
            .map_err(|_| out_of_return_memory!())?;
    }

    Ok(())
}

pub fn secp256r1_verify_inner(
    digest: &[u8; 32],
    r: &[u8; 32],
    s: &[u8; 32],
    x: &[u8; 32],
    y: &[u8; 32],
) -> Result<bool, ()> {
    crypto::secp256r1::verify(digest, r, s, x, y).map_err(|_| ())
}
