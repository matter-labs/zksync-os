use super::*;

use alloc::vec::Vec;
use evm_interpreter::ERGS_PER_GAS;
use ruint::aliases::U256;
use zk_ee::common_traits::TryExtend;
use zk_ee::oracle::query_ids::ADVICE_SUBSPACE_MASK;
use zk_ee::oracle::IOOracle;
use zk_ee::system::logger::Logger;
use zk_ee::system::SystemFunctionExt;
use zk_ee::{
    interface_error, internal_error, out_of_ergs_error,
    system::{
        base_system_functions::ModExpErrors,
        errors::{subsystem::SubsystemError, system::SystemError},
        Computational, Ergs, ModExpInterfaceError,
    },
};

use crate::cost_constants::{
    MODEXP_BASE_NATIVE_COST, MODEXP_MINIMAL_COST_ERGS, MODEXP_PER_OP_DIGIT_SQ_NATIVE_COST,
    MODEXP_PER_OP_OVERHEAD_NATIVE_COST,
};

/// Count the bit length and popcount of a big-endian byte slice,
/// skipping leading zero bytes.
fn exp_bit_len_and_popcount(exp: &[u8]) -> (u64, u64) {
    let mut bit_len: u64 = 0;
    let mut popcount: u64 = 0;
    let mut leading = true;
    for &byte in exp {
        if leading {
            if byte == 0 {
                continue;
            }
            leading = false;
            bit_len = 8 - byte.leading_zeros() as u64;
            popcount = byte.count_ones() as u64;
        } else {
            bit_len += 8;
            popcount += byte.count_ones() as u64;
        }
    }
    (bit_len, popcount)
}

/// Compute the native cost from the number of square-and-multiply
/// operations and the operand digit count.
///
/// The prover performs one modular squaring per exponent bit (after the
/// leading 1) and one modular multiplication per set bit (excluding the
/// leading 1). Each operation does a schoolbook multiply of `digits x digits`
/// pairs, each producing bigint delegations at a fixed effective-cycle cost.
fn modexp_native_from_ops(total_ops: u64, digits: u64) -> u64 {
    MODEXP_BASE_NATIVE_COST
        .saturating_add(total_ops.saturating_mul(MODEXP_PER_OP_OVERHEAD_NATIVE_COST))
        .saturating_add(
            total_ops
                .saturating_mul(digits.saturating_mul(digits))
                .saturating_mul(MODEXP_PER_OP_DIGIT_SQ_NATIVE_COST),
        )
}

// Query ID for modular exponentiation advice from oracle
pub const MODEXP_ADVICE_QUERY_ID: u32 = ADVICE_SUBSPACE_MASK | 0x10;

/// Parameters for modular exponentiation oracle query
/// Used to request division advice for big integer operations during modexp
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ModExpAdviceParamsGeneric<W> {
    pub op: W,          // Operation type (0 = division)
    pub a_ptr: W,       // Pointer to dividend
    pub a_len: W,       // Length of dividend in words
    pub b_ptr: W,       // Pointer to divisor
    pub b_len: W,       // Length of divisor in words
    pub modulus_ptr: W, // Pointer to modulus
    pub modulus_len: W, // Length of modulus in words
}

/// Used for proving (RISC-V 32-bit)
pub type ModExpAdviceParams = ModExpAdviceParamsGeneric<u32>;

/// Used for native execution (64-bit)
pub type ModExpAdviceParams64 = ModExpAdviceParamsGeneric<u64>;

pub mod advice;

///
/// modexp system function implementation.
///
pub struct ModExpImpl<const USE_ADVICE: bool>;

impl<R: Resources, const USE_ADVICE: bool> SystemFunctionExt<R, ModExpErrors>
    for ModExpImpl<USE_ADVICE>
{
    /// If the input size is less than expected - it will be padded with zeroes.
    /// If the input size is greater - redundant bytes will be ignored.
    ///
    /// Returns `OutOfGas` if not enough resources provided, resources may be not touched.
    ///
    /// Returns `InvalidInput` error if `base_len` > usize max value
    /// or `mod_len` > usize max value
    /// or (`exp_len` > usize max value and `base_len` != 0 and `mod_len` != 0).
    /// In practice, it shouldn't be possible as requires large resources amounts, at least ~1e10 EVM gas.
    fn execute<
        O: IOOracle,
        L: Logger,
        D: TryExtend<u8> + ?Sized,
        A: core::alloc::Allocator + Clone,
    >(
        input: &[u8],
        output: &mut D,
        resources: &mut R,
        oracle: &mut O,
        logger: &mut L,
        allocator: A,
    ) -> Result<(), SubsystemError<ModExpErrors>> {
        cycle_marker::wrap_with_resources!("modexp", resources, {
            modexp_as_system_function_inner::<_, _, _, _, _, USE_ADVICE>(
                input, output, resources, oracle, logger, allocator,
            )
        })
    }
}

fn read_padded(dst: &mut Vec<u8, impl Allocator>, src: &mut &[u8], provided_len: usize) {
    let source_len = src.len();
    let to_take = core::cmp::min(source_len, provided_len);
    let (bytes, rest) = (*src).split_at(to_take);
    *src = rest;
    dst.extend_from_slice(&bytes);

    if provided_len > source_len {
        dst.resize(provided_len, 0);
    }
}

// Based on https://github.com/bluealloy/revm/blob/main/crates/precompile/src/modexp.rs
#[allow(unused_variables)]
fn modexp_as_system_function_inner<
    O: IOOracle,
    L: Logger,
    D: ?Sized + TryExtend<u8>,
    A: Allocator + Clone,
    R: Resources,
    // Toggle delegation-based implementation for forward run, to be able
    // to capture oracle queries.
    const USE_ADVICE: bool,
>(
    input: &[u8],
    dst: &mut D,
    resources: &mut R,
    oracle: &mut O,
    logger: &mut L,
    allocator: A,
) -> Result<(), SubsystemError<ModExpErrors>> {
    // Check at least we have min gas
    let minimal_native = <R::Native as Computational>::from_computational(MODEXP_BASE_NATIVE_COST);
    let minimal_resources = R::from_ergs_and_native(MODEXP_MINIMAL_COST_ERGS, minimal_native);
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
        return Err(interface_error!(ModExpInterfaceError::InvalidInputLength));
    };
    let Ok(mod_len) = usize::try_from(mod_len) else {
        return Err(interface_error!(ModExpInterfaceError::InvalidInputLength));
    };

    // Under fusaka repricing (EIP-7883), the base==mod==0 case is not special-cased:
    // the gas formula must still be evaluated because multiplication_complexity is a
    // fixed 16 for small inputs, and the exponent can still contribute to iteration_count.

    // Cast exponent length to usize, since it does not make sense to handle larger values.
    //
    // At this point base_len != 0 || mod_len != 0
    // So, on 32 bit machine precompile will cost at least around ~ 2^32*8/3 ~= 1e10 gas,
    // so should be ok in practice
    let Ok(exp_len) = usize::try_from(exp_len) else {
        return Err(interface_error!(ModExpInterfaceError::InvalidInputLength));
    };

    // EIP-7823: reject inputs where any length exceeds 1024 bytes
    {
        const EIP_7823_LENGTH_LIMIT: usize = 1024;
        if base_len > EIP_7823_LENGTH_LIMIT
            || exp_len > EIP_7823_LENGTH_LIMIT
            || mod_len > EIP_7823_LENGTH_LIMIT
        {
            return Err(interface_error!(
                ModExpInterfaceError::InputLengthExceedsLimit
            ));
        }
    }

    // Used to extract ADJUSTED_EXPONENT_LENGTH.
    let exp_highp_len = core::cmp::min(exp_len, 32);

    let mut input = input.get(HEADER_LENGTH..).unwrap_or_default();

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

    // Gate the operand allocations on available resources *before* allocating
    // and zero-filling. Both ergs (length-derived) and a conservative native
    // upper bound (worst-case exponent) are computable from the header lengths
    // and `exp_highp` alone, without materializing the operands. This prevents
    // an input declaring large lengths with a minimal payload from forcing
    // large allocations / zero-fills before any commensurate charge (relevant
    // when the EIP-7823 length cap is not enabled).
    let ergs = ergs_cost(base_len as u64, exp_len as u64, mod_len as u64, &exp_highp)?;
    let conservative_native =
        native_cost::<R>(base_len as u64, exp_len as u64, mod_len as u64, &exp_highp)?;
    if !resources.has_enough(&R::from_ergs_and_native(ergs, conservative_native)) {
        return Err(out_of_ergs_error!().into());
    }

    let mut base = Vec::try_with_capacity_in(base_len, allocator.clone())
        .map_err(|_| SystemError::LeafDefect(internal_error!("alloc")))?;
    read_padded(&mut base, &mut input, base_len);

    let mut exponent = Vec::try_with_capacity_in(exp_len, allocator.clone())
        .map_err(|_| SystemError::LeafDefect(internal_error!("alloc")))?;
    read_padded(&mut exponent, &mut input, exp_len);

    let mut modulus = Vec::try_with_capacity_in(mod_len, allocator.clone())
        .map_err(|_| SystemError::LeafDefect(internal_error!("alloc")))?;
    read_padded(&mut modulus, &mut input, mod_len);

    // Charge the exact native (scans the materialized exponent's set bits).
    // It never exceeds `conservative_native` checked above, so it cannot fail
    // after the gate passed.
    let native = native_cost_from_exp_data::<R>(base_len as u64, &exponent, mod_len as u64)?;
    resources.charge(&R::from_ergs_and_native(ergs, native))?;

    debug_assert_eq!(base.len(), base_len);
    debug_assert_eq!(exponent.len(), exp_len);
    debug_assert_eq!(modulus.len(), mod_len);

    // Call the modexp.

    #[cfg(any(all(target_arch = "riscv32", feature = "proving"), test))]
    let output = self::advice::modexp(
        base.as_slice(),
        exponent.as_slice(),
        modulus.as_slice(),
        oracle,
        logger,
        allocator,
    );

    #[cfg(not(any(all(target_arch = "riscv32", feature = "proving"), test)))]
    let output = if USE_ADVICE {
        self::advice::modexp(
            base.as_slice(),
            exponent.as_slice(),
            modulus.as_slice(),
            oracle,
            logger,
            allocator,
        )
    } else {
        ::modexp::modexp(
            base.as_slice(),
            exponent.as_slice(),
            modulus.as_slice(),
            allocator,
        )
    };

    if output.len() >= mod_len {
        // truncate
        dst.try_extend(output[(output.len() - mod_len)..].iter().copied())
            .map_err(|_| out_of_ergs_error!())?;
    } else {
        dst.try_extend(core::iter::repeat_n(0, mod_len - output.len()).chain(output))
            .map_err(|_| out_of_ergs_error!())?;
    }

    Ok(())
}

/// Computes the ergs cost for modexp (Fusaka repricing, EIP-7883).
/// Returns an OOG error if there's an arithmetic overflow.
pub fn ergs_cost(
    base_size: u64,
    exp_size: u64,
    mod_size: u64,
    exp_highp: &U256,
) -> Result<Ergs, SystemError> {
    let multiplication_complexity = {
        let max_length = core::cmp::max(base_size, mod_size);
        if max_length <= 32 {
            16u64
        } else {
            let words = max_length.div_ceil(8);
            words
                .checked_mul(words)
                .ok_or(out_of_ergs_error!())?
                .checked_mul(2)
                .ok_or(out_of_ergs_error!())?
        }
    };
    let iteration_count = {
        let ic = if exp_size <= 32 && exp_highp.is_zero() {
            0
        } else if exp_size <= 32 {
            exp_highp.bit_len() as u64 - 1
        } else {
            16u64
                .checked_mul(exp_size - 32)
                .ok_or(out_of_ergs_error!())?
                .checked_add(core::cmp::max(1, exp_highp.bit_len() as u64) - 1)
                .ok_or(out_of_ergs_error!())?
        };
        core::cmp::max(1, ic)
    };
    let computed_gas = multiplication_complexity
        .checked_mul(iteration_count)
        .ok_or(out_of_ergs_error!())?;
    let gas = core::cmp::max(500, computed_gas);
    let ergs = gas.checked_mul(ERGS_PER_GAS).ok_or(out_of_ergs_error!())?;
    Ok(Ergs(ergs))
}

/// Computes the native cost for modexp by scanning the exponent.
pub fn native_cost_from_exp_data<R: Resources>(
    base_size: u64,
    exp_data: &[u8],
    mod_size: u64,
) -> Result<R::Native, SystemError> {
    let digits = core::cmp::max(1, core::cmp::max(base_size, mod_size).div_ceil(32));
    let (bit_len, popcount) = exp_bit_len_and_popcount(exp_data);
    let squares = bit_len.saturating_sub(1);
    let multiplies = popcount.saturating_sub(1);
    let cost = modexp_native_from_ops(squares + multiplies, digits);
    Ok(<R::Native as Computational>::from_computational(cost))
}

/// Conservative native cost without scanning the full exponent (assumes the
/// worst case of all exponent bits set). Used to gate operand allocation
/// before the exact cost can be computed from the materialized exponent.
fn native_cost<R: Resources>(
    base_size: u64,
    exp_size: u64,
    mod_size: u64,
    exp_highp: &U256,
) -> Result<R::Native, SystemError> {
    let digits = core::cmp::max(1, core::cmp::max(base_size, mod_size).div_ceil(32));
    let bit_len = if exp_size <= 32 && exp_highp.is_zero() {
        0u64
    } else if exp_size <= 32 {
        exp_highp.bit_len() as u64
    } else {
        8u64.saturating_mul(exp_size.saturating_sub(32))
            .saturating_add(core::cmp::max(1, exp_highp.bit_len() as u64))
    };
    // Worst case: all bits set → squares = multiplies = bit_len - 1.
    let total_ops = 2u64.saturating_mul(bit_len.saturating_sub(1));
    let cost = modexp_native_from_ops(total_ops, digits);
    Ok(<R::Native as Computational>::from_computational(cost))
}
