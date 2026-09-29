use super::*;
use crate::cost_constants::{ECRECOVER_COST_GAS, ECRECOVER_NATIVE_COST};
use field_ops::Secp256k1HooksWithOracle;
use zk_ee::common_structs::precompile_caches::{
    EcRecoverCache, EcRecoverInput, PrecompileCaches, ECRECOVER_INPUT_LEN,
};
use zk_ee::common_traits::TryExtend;
use zk_ee::oracle::IOOracle;
use zk_ee::out_of_return_memory;
use zk_ee::system::base_system_functions::Secp256k1ECRecoverErrors;
use zk_ee::system::errors::{subsystem::SubsystemError, system::SystemError};
use zk_ee::system::SystemFunctionExt;

///
/// ecrecover system function implementation.
///
pub struct EcRecoverImpl<const USE_ADVICE: bool>;

impl<R: Resources, const USE_ADVICE: bool> SystemFunctionExt<R, Secp256k1ECRecoverErrors>
    for EcRecoverImpl<USE_ADVICE>
{
    /// If the input size is less than expected - it will be padded with zeroes.
    /// If the input size is greater - redundant bytes will be ignored.
    /// If the input is invalid(v != 27|28 or failed to recover signer) returns `Ok(0)`.
    ///
    /// Returns `OutOfGas` if not enough resources provided.
    fn execute<O: IOOracle, L, D: TryExtend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        oracle: &mut O,
        _logger: &mut L,
        _allocator: A,
    ) -> Result<(), SubsystemError<Secp256k1ECRecoverErrors>> {
        Ok(cycle_marker::wrap_with_resources!(
            "ecrecover",
            resources,
            {
                ecrecover_as_system_function_inner::<_, _, _, _, USE_ADVICE>(
                    input,
                    output,
                    resources,
                    Some(oracle),
                    None,
                )
            }
        )?)
    }

    fn execute_with_precompile_caches<
        O: IOOracle,
        L: zk_ee::system::logger::Logger,
        D: TryExtend<u8> + ?Sized,
        A: core::alloc::Allocator + Clone,
    >(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        oracle: &mut O,
        precompile_caches: &mut PrecompileCaches,
        _logger: &mut L,
        _allocator: A,
    ) -> Result<(), SubsystemError<Secp256k1ECRecoverErrors>> {
        Ok(cycle_marker::wrap_with_resources!(
            "ecrecover",
            resources,
            {
                ecrecover_as_system_function_inner::<_, _, _, _, USE_ADVICE>(
                    input,
                    output,
                    resources,
                    Some(oracle),
                    Some(&mut precompile_caches.ecrecover),
                )
            }
        )?)
    }
}

// if the oracle is provided, it will be used for field operations
fn ecrecover_as_system_function_inner<
    O: IOOracle,
    S: ?Sized + MinimalByteAddressableSlice,
    D: ?Sized + TryExtend<u8>,
    R: Resources,
    const USE_ADVICE: bool,
>(
    src: &S,
    dst: &mut D,
    resources: &mut R,
    oracle: Option<&mut O>,
    recoveries: Option<&mut EcRecoverCache>,
) -> Result<(), SystemError> {
    resources.charge_legacy_gas_and_native(ECRECOVER_COST_GAS, ECRECOVER_NATIVE_COST)?;
    // digest, v, r, s in ABI
    let mut input = EcRecoverInput([0u8; ECRECOVER_INPUT_LEN]);
    for (dst, src) in input.0.iter_mut().zip(src.iter()) {
        *dst = *src;
    }

    // follow https://github.com/ethereum/go-ethereum/blob/aadcb886753079d419f966a3bc990f708f8d1c3b/core/vm/contracts.go#L188

    let ([digest, v, r, s], []) = input.0.as_chunks::<32>() else {
        unreachable!("128 bytes are 4 chunks of 32")
    };

    if v[..31].iter().all(|el| *el == 0) == false {
        return Ok(());
    }

    let rec_id = v[31].wrapping_sub(27);
    if (rec_id == 0 || rec_id == 1) == false {
        return Ok(());
    }

    let address_hash = match recoveries
        .as_deref()
        .and_then(|cache| cache.get(&input))
        .copied()
    {
        Some(outcome) => outcome,
        None => {
            let oracle = if USE_ADVICE { oracle } else { None };
            let outcome = ecrecover_inner(digest, r, s, rec_id == 1, oracle)
                .ok()
                .map(|public_key| super::keccak256::keccak256_digest(&public_key.0));
            if let Some(cache) = recoveries {
                cache.insert(&input, outcome);
            }
            outcome
        }
    };
    let Some(address_hash) = address_hash else {
        return Ok(());
    };

    dst.try_extend(core::iter::repeat_n(0, 12).chain(address_hash.into_iter().skip(12)))
        .map_err(|_| out_of_return_memory!())?;

    Ok(())
}

/// Bytes at an address aligned for words: the conversions between big-endian bytes and integers
/// work by words then (see `crypto::bigint_delegation::u256::from_be_bytes`)
#[repr(C, align(4))]
pub struct WordAligned<const N: usize>(pub [u8; N]);

/// The public key (its coordinates `x` and `y`, big-endian) of the signature `(r, s)` of
/// `digest`, for the point of the signature with the coordinate `x = r` and the `y` of the
/// given parity. Fails if `r` or `s` is not in `[1, order - 1]`, or there is no such point or
/// key.
pub fn ecrecover_inner<O: IOOracle>(
    digest: &[u8; 32],
    r: &[u8; 32],
    s: &[u8; 32],
    y_is_odd: bool,
    oracle: Option<&mut O>,
) -> Result<WordAligned<64>, ()> {
    use crypto::secp256k1::{hooks::DefaultSecp256k1Hooks, recover_from_bytes_with_hooks};

    let public_key = match oracle {
        Some(oracle) => recover_from_bytes_with_hooks(
            digest,
            r,
            s,
            y_is_odd,
            &mut Secp256k1HooksWithOracle::new(oracle),
        ),
        None => recover_from_bytes_with_hooks(digest, r, s, y_is_odd, &mut DefaultSecp256k1Hooks),
    }
    .map_err(|_| ())?;

    // represent as bytes, and we do not need compression
    let mut encoded = WordAligned([0u8; 64]);
    let ([x, y], []) = encoded.0.as_chunks_mut::<32>() else {
        unreachable!("64 bytes are 2 chunks of 32")
    };
    public_key.write_coordinates(x, y);

    Ok(encoded)
}

#[cfg(test)]
mod tests;
