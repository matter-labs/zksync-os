use super::*;

use crate::cost_constants::{MODEXP_MINIMAL_COST_ERGS, MODEXP_WORST_CASE_NATIVE_PER_GAS};
use alloc::vec::Vec;
use crypto::modexp::modexp;
use evm_interpreter::ERGS_PER_GAS;
use ruint::aliases::U256;
use zk_ee::{
    internal_error, out_of_ergs_error,
    system::{
        errors::{SystemError, SystemFunctionError},
        Computational, Ergs, SystemFunction,
    },
};

///
/// modexp system function implementation.
///
pub struct ModExpImpl;

impl<R: Resources> SystemFunction<R> for ModExpImpl {
    /// If the input size is less than expected - it will be padded with zeroes.
    /// If the input size is greater - redundant bytes will be ignored.
    ///
    /// Returns `OutOfGas` if not enough resources provided, resources may be not touched.
    ///
    /// Returns `InvalidInput` error if `base_len` > usize max value
    /// or `mod_len` > usize max value
    /// or (`exp_len` > usize max value and `base_len` != 0 and `mod_len` != 0).
    /// In practice, it shouldn't be possible as requires large resources amounts, at least ~1e10 EVM gas.
    fn execute<D: Extend<u8> + ?Sized, A: core::alloc::Allocator + Clone>(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        allocator: A,
    ) -> Result<(), SystemFunctionError> {
        cycle_marker::wrap_with_resources!("modexp", resources, {
            modexp_as_system_function_inner(input, output, resources, allocator)
        })
    }
}

/// Get resources from ergs, with native being ergs * constant
fn resources_from_ergs<R: Resources>(ergs: Ergs) -> R {
    let native =
        <R::Native as Computational>::from_computational(ergs.0 * MODEXP_WORST_CASE_NATIVE_PER_GAS);
    R::from_ergs_and_native(ergs, native)
}

// Based on https://github.com/bluealloy/revm/blob/main/crates/precompile/src/modexp.rs
fn modexp_as_system_function_inner<D: ?Sized + Extend<u8>, A: Allocator + Clone, R: Resources>(
    input: &[u8],
    dst: &mut D,
    resources: &mut R,
    allocator: A,
) -> Result<(), SystemFunctionError> {
    // Check at least we have min gas
    let minimal_resources = resources_from_ergs::<R>(MODEXP_MINIMAL_COST_ERGS);
    if !resources.has_enough(&minimal_resources) {
        return Err(out_of_ergs_error!().into());
    }

    // The format of input is:
    // <length_of_BASE> <length_of_EXPONENT> <length_of_MODULUS> <BASE> <EXPONENT> <MODULUS>
    // Where every length is a 32-byte left-padded integer representing the number of bytes
    // to be taken up by the next value.
    const HEADER_LENGTH: usize = 96;

    // Extract the header
    let mut input_it = input.iter();
    let mut base_len = [0u8; 32];
    for (dst, src) in base_len.iter_mut().zip(&mut input_it) {
        *dst = *src;
    }
    let mut exp_len = [0u8; 32];
    for (dst, src) in exp_len.iter_mut().zip(&mut input_it) {
        *dst = *src;
    }
    let mut mod_len = [0u8; 32];
    for (dst, src) in mod_len.iter_mut().zip(&mut input_it) {
        *dst = *src;
    }
    let base_len = U256::from_be_bytes(base_len);
    let exp_len = U256::from_be_bytes(exp_len);
    let mod_len = U256::from_be_bytes(mod_len);

    // Cast base and modulus to usize, it does not make sense to handle larger values
    //
    // On 32 bit machine precompile will cost at least around ~ (2^32/8)^2/3 ~= 9e16 gas,
    // so should be ok in practice
    let Ok(base_len) = usize::try_from(base_len) else {
        return Err(SystemFunctionError::InvalidInput);
    };
    let Ok(mod_len) = usize::try_from(mod_len) else {
        return Err(SystemFunctionError::InvalidInput);
    };

    // Handle a special case when both the base and mod length are zero.
    if base_len == 0 && mod_len == 0 {
        // should be safe, since we checked that there is enough resources at the beginning
        resources.charge(&minimal_resources)?;
        return Ok(());
    }

    // Cast exponent length to usize, since it does not make sense to handle larger values.
    //
    // At this point base_len != 0 || mod_len != 0
    // So, on 32 bit machine precompile will cost at least around ~ 2^32*8/3 ~= 1e10 gas,
    // so should be ok in practice
    let Ok(exp_len) = usize::try_from(exp_len) else {
        return Err(SystemFunctionError::InvalidInput);
    };

    // Used to extract ADJUSTED_EXPONENT_LENGTH.
    let exp_highp_len = core::cmp::min(exp_len, 32);

    let input = input.get(HEADER_LENGTH..).unwrap_or_default();

    let exp_highp = {
        // get right padded bytes so if data.len is less then exp_len we will get right padded zeroes.
        let exp_it = input.get(base_len..).unwrap_or_default().iter();
        // If exp_len is less then 32 bytes get only exp_len bytes and do left padding.
        let mut out = [0u8; 32];
        for (dst, src) in out[32 - exp_highp_len..].iter_mut().zip(exp_it) {
            *dst = *src;
        }
        U256::from_be_bytes(out)
    };

    // Check if we have enough gas.
    let ergs = ergs_cost(base_len as u64, exp_len as u64, mod_len as u64, &exp_highp)?;
    resources.charge(&resources_from_ergs::<R>(ergs))?;

    let mut input_it = input.iter();
    let mut base = Vec::try_with_capacity_in(base_len, allocator.clone())
        .map_err(|_| SystemError::Internal(internal_error!("alloc")))?;
    base.resize(base_len, 0);
    for (dst, src) in base.iter_mut().zip(&mut input_it) {
        *dst = *src;
    }

    let mut exponent = Vec::try_with_capacity_in(exp_len, allocator.clone())
        .map_err(|_| SystemError::Internal(internal_error!("alloc")))?;
    exponent.resize(exp_len, 0);
    for (dst, src) in exponent.iter_mut().zip(&mut input_it) {
        *dst = *src;
    }

    let mut modulus = Vec::try_with_capacity_in(mod_len, allocator.clone())
        .map_err(|_| SystemError::Internal(internal_error!("alloc")))?;
    modulus.resize(mod_len, 0);
    for (dst, src) in modulus.iter_mut().zip(&mut input_it) {
        *dst = *src;
    }

    // Call the modexp.
    let output = modexp(
        base.as_slice(),
        exponent.as_slice(),
        modulus.as_slice(),
        allocator,
    );

    dst.extend(core::iter::repeat_n(0, mod_len - output.len()).chain(output));

    Ok(())
}

/// Computes the ergs cost for modexp.
/// Returns an OOG error if there's an arithmetic overflow.
pub fn ergs_cost(
    base_size: u64,
    exp_size: u64,
    mod_size: u64,
    exp_highp: &U256,
) -> Result<Ergs, SystemError> {
    let multiplication_complexity = {
        let max_length = core::cmp::max(base_size, mod_size);
        let words = max_length.div_ceil(8);
        words.checked_mul(words).ok_or(out_of_ergs_error!())?
    };
    let iteration_count = {
        let ic = if exp_size <= 32 && exp_highp.is_zero() {
            0
        } else if exp_size <= 32 {
            exp_highp.bit_len() as u64 - 1
        } else {
            8u64.checked_mul(exp_size - 32)
                .ok_or(out_of_ergs_error!())?
                .checked_add(core::cmp::max(1, exp_highp.bit_len() as u64) - 1)
                .ok_or(out_of_ergs_error!())?
        };
        core::cmp::max(1, ic)
    };
    let computed_gas = multiplication_complexity
        .checked_mul(iteration_count)
        .ok_or(out_of_ergs_error!())?
        .checked_div(3)
        .ok_or(out_of_ergs_error!())?;
    let gas = core::cmp::max(200, computed_gas);
    let ergs = gas.checked_mul(ERGS_PER_GAS).ok_or(out_of_ergs_error!())?;
    Ok(Ergs(ergs))
}
