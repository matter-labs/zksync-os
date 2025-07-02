use super::*;

use crate::cost_constants::P256_VERIFY_COST_ERGS;
use zk_ee::system::errors::SystemFunctionError;
use zk_ee::system::SystemFunction;

// TODO(EVM-1072): think about error cases, as others follow evm specs
/// p256 verify system function implementation.
/// Returns the size in bytes of output.
///
/// Input length should be 160, otherwise `InternalError` will be returned.
///
/// In case of invalid input `Ok(0)` will be returned and resources will be charged.
///
/// If dst len less than needed(1) returns `InternalError`.
pub struct P256VerifyImpl;

impl<R: Resources> SystemFunction<R> for P256VerifyImpl {
    fn execute<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        src: &[u8],
        dst: &mut D,
        resources: &mut R,
        _: A,
    ) -> Result<(), SystemFunctionError> {
        cycle_marker::wrap_with_resources!("p256_verify", resources, {
            p256_verify_as_system_function_inner(src, dst, resources)
        })
    }
}

fn p256_verify_as_system_function_inner<
    S: ?Sized + MinimalByteAddressableSlice,
    D: ?Sized + Extend<u8>,
    R: Resources,
>(
    src: &S,
    dst: &mut D,
    resources: &mut R,
) -> Result<(), SystemFunctionError> {
    if src.len() != 160 {
        return Err(SystemFunctionError::InvalidInput);
    }
    resources.charge(&R::from_ergs(P256_VERIFY_COST_ERGS))?;
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
            return Ok(());
        };

        result
    };

    dst.extend(core::iter::once(is_valid as u8));

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
