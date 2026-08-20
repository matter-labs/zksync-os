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
    interface_error, internal_error, out_of_ergs_error, out_of_native_resources,
    out_of_return_memory,
    system::{
        base_system_functions::ModExpErrors,
        errors::{subsystem::SubsystemError, system::SystemError},
        Computational, Ergs, ModExpInterfaceError, Resource,
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

/// The delegated modexp implementation may do one `quotient * modulus + remainder`
/// FMA before the exponent loop to reduce an oversized base into the modulus
/// range (`advice/bigint.rs`: `reduce_initially`).
///
/// That FMA is a schoolbook multiply whose delegation count scales with the
/// product of the operand word counts — `quotient_digits * modulus_digits` (the
/// two loops in `fma`), not `modulus_digits^2`. For an oversized base reduced by
/// a small modulus the quotient dominates, so charging `modulus_digits^2` would
/// undercharge. We therefore charge on the true `quotient_digits * modulus_digits`
/// product.
///
/// A regular square/multiply step does two FMAs in the current model, so we
/// account this standalone reduction as half of a normal per-op charge.
fn modexp_initial_reduction_native_cost(quotient_digits: u64, modulus_digits: u64) -> u64 {
    let digit_products = quotient_digits.saturating_mul(modulus_digits);
    MODEXP_PER_OP_OVERHEAD_NATIVE_COST
        .saturating_add(digit_products.saturating_mul(MODEXP_PER_OP_DIGIT_SQ_NATIVE_COST))
        .div_ceil(2)
}

fn strip_leading_zeroes(bytes: &[u8]) -> &[u8] {
    let first_nonzero = bytes
        .iter()
        .position(|&byte| byte != 0)
        .unwrap_or(bytes.len());
    &bytes[first_nonzero..]
}

/// Number of 32-byte words needed to represent the value, ignoring leading
/// zero bytes. This mirrors `BigUint::digits` in the delegated bigint
/// implementation (`advice/bigint.rs`), which counts significant 32-byte words.
fn significant_word_count(bytes: &[u8]) -> u64 {
    (strip_leading_zeroes(bytes).len() as u64).div_ceil(32)
}

/// Whether the delegated modexp implementation performs its standalone initial
/// reduction FMA before the exponent loop.
///
/// That implementation gates the reduction purely on the significant 32-byte
/// word counts of the operands (`advice/bigint.rs`: `if current.digits >=
/// modulus.digits`) and, once the gate passes, always performs the FMA
/// regardless of the operand values. We therefore compare word counts here too
/// (rather than the numeric values): a base that is smaller in value than the
/// modulus but occupies the same number of words is still reduced, and must be
/// charged accordingly.
///
/// Two operand values short-circuit the delegated path *before* the reduction
/// and so must not be charged for it:
/// - `modulus == 0`: the implementation returns an empty result immediately.
/// - `modulus == 1`: `modexp_inner` returns before parsing the base or calling
///   `modpow`, so `reduce_initially` never runs (`base^exp mod 1 == 0`).
fn requires_initial_reduction(base: &[u8], modulus: &[u8]) -> bool {
    let modulus = strip_leading_zeroes(modulus);
    // modulus == 0 (no significant bytes) or modulus == 1 (a single 0x01 byte):
    // the delegated path returns before any reduction.
    if modulus.is_empty() || modulus == [1u8] {
        return false;
    }
    let modulus_words = (modulus.len() as u64).div_ceil(32);
    significant_word_count(base) >= modulus_words
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
    if !resources.ergs().has_enough(&MODEXP_MINIMAL_COST_ERGS) {
        return Err(out_of_ergs_error!().into());
    }
    if !resources.native().has_enough(&minimal_native) {
        return Err(out_of_native_resources!().into());
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
    if !resources.ergs().has_enough(&ergs) {
        return Err(out_of_ergs_error!().into());
    }
    if !resources.native().has_enough(&conservative_native) {
        return Err(out_of_native_resources!().into());
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
    // `conservative_native` checked above is a true upper bound on this value
    // (worst-case exponent, and it charges the initial reduction whenever the
    // exact charge might), so this charge cannot fail after the gate passed.
    let native = native_cost_from_exp_data::<R>(&base, &exponent, &modulus)?;
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
            .map_err(|_| out_of_return_memory!())?;
    } else {
        dst.try_extend(core::iter::repeat_n(0, mod_len - output.len()).chain(output))
            .map_err(|_| out_of_return_memory!())?;
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
    base: &[u8],
    exp_data: &[u8],
    modulus: &[u8],
) -> Result<R::Native, SystemError> {
    // Per-op cost tracks the operands actually multiplied in the exponent loop.
    // After the initial reduction every square/multiply operand is reduced mod
    // `modulus` (operands are asserted `<= modulus.digits` in `advice/bigint.rs`),
    // so the per-op work is `modulus_digits^2`, not `max(base, modulus)_digits^2`.
    // The base size only affects the separate initial-reduction term below.
    let digits = core::cmp::max(1, significant_word_count(modulus));
    let (bit_len, popcount) = exp_bit_len_and_popcount(exp_data);
    let squares = bit_len.saturating_sub(1);
    let multiplies = popcount.saturating_sub(1);
    let reduction_cost = if requires_initial_reduction(base, modulus) {
        let modulus_digits = significant_word_count(modulus);
        // The honest quotient q = base / modulus occupies at most
        // `base_digits - modulus_digits + 1` words (guaranteed >= 1 here, since
        // `requires_initial_reduction` implies `base_digits >= modulus_digits`).
        let quotient_digits = significant_word_count(base)
            .saturating_sub(modulus_digits)
            .saturating_add(1);
        modexp_initial_reduction_native_cost(quotient_digits, modulus_digits)
    } else {
        0
    };
    let cost = modexp_native_from_ops(squares + multiplies, digits).saturating_add(reduction_cost);
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
    // Per-op cost is bounded by the modulus size (exponent-loop operands are
    // reduced mod `modulus`); the base only affects the reduction term below.
    // The header modulus word count upper-bounds the significant modulus digits
    // the exact charge uses, keeping this gate a true upper bound.
    let digits = core::cmp::max(1, mod_size.div_ceil(32));
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
    // Conservative upper bound on the exact reduction charge. The exact charge
    // (`native_cost_from_exp_data`) adds `quotient_digits * modulus_digits`,
    // where `quotient_digits <= base_digits` and `modulus_digits` are the
    // significant word counts. We cannot strip leading zeroes here without
    // materializing the operands, so we upper-bound both dimensions by the
    // header word counts (`base_size`/`mod_size`, which dominate the significant
    // counts) and charge the reduction whenever both operands are non-empty.
    // This keeps the gate a true upper bound (it also covers the `modulus == 1`
    // case, which the exact charge skips). `base_words * mod_words >=
    // quotient_digits * modulus_digits`.
    let reduction_cost = if mod_size != 0 && base_size != 0 {
        let modulus_words = core::cmp::max(1, mod_size.div_ceil(32));
        let base_words = core::cmp::max(1, base_size.div_ceil(32));
        modexp_initial_reduction_native_cost(base_words, modulus_words)
    } else {
        0
    };
    let cost = modexp_native_from_ops(total_ops, digits).saturating_add(reduction_cost);
    Ok(<R::Native as Computational>::from_computational(cost))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::alloc::Global;
    use zk_ee::oracle::IOOracle;
    use zk_ee::reference_implementations::{BaseResources, DecreasingNative};
    use zk_ee::system::{
        errors::{
            runtime::{FatalRuntimeError, RuntimeError},
            subsystem::SubsystemError,
        },
        NullLogger, Resources,
    };

    type TestResources = BaseResources<DecreasingNative>;

    struct DummyOracle;
    struct AlwaysFailDst;

    impl IOOracle for DummyOracle {
        type RawIterator<'a> = Box<dyn ExactSizeIterator<Item = usize> + 'static>;

        fn raw_query<
            'a,
            I: zk_ee::oracle::usize_serialization::UsizeSerializable
                + zk_ee::oracle::usize_serialization::UsizeDeserializable,
        >(
            &'a mut self,
            _query_type: u32,
            _input: &I,
        ) -> Result<Self::RawIterator<'a>, zk_ee::system::errors::internal::InternalError> {
            unreachable!("oracle should not be consulted on native targets");
        }
    }

    impl TryExtend<u8> for AlwaysFailDst {
        type Error = ();

        fn try_extend<I>(&mut self, _iter: I) -> Result<(), Self::Error>
        where
            I: IntoIterator<Item = u8>,
        {
            Err(())
        }
    }

    fn encode_len(len: usize) -> [u8; 32] {
        U256::from(len).to_be_bytes::<32>()
    }

    fn modexp_input(base: &[u8], exponent: &[u8], modulus: &[u8]) -> Vec<u8> {
        let mut input = Vec::with_capacity(96 + base.len() + exponent.len() + modulus.len());
        input.extend_from_slice(&encode_len(base.len()));
        input.extend_from_slice(&encode_len(exponent.len()));
        input.extend_from_slice(&encode_len(modulus.len()));
        input.extend_from_slice(base);
        input.extend_from_slice(exponent);
        input.extend_from_slice(modulus);
        input
    }

    fn assert_out_of_native(err: SubsystemError<ModExpErrors>) {
        assert!(matches!(
            err,
            SubsystemError::LeafRuntime(RuntimeError::FatalRuntimeError(
                FatalRuntimeError::OutOfNativeResources(_)
            ))
        ));
    }

    fn assert_out_of_return_memory(err: SubsystemError<ModExpErrors>) {
        assert!(matches!(
            err,
            SubsystemError::LeafRuntime(RuntimeError::FatalRuntimeError(
                FatalRuntimeError::OutOfReturnMemory(_)
            ))
        ));
    }

    #[test]
    fn returns_out_of_native_when_minimal_native_missing() {
        let input = modexp_input(&[2], &[1], &[3]);
        let mut output = Vec::new();
        let mut resources = TestResources::from_ergs_and_native(
            MODEXP_MINIMAL_COST_ERGS,
            DecreasingNative::from_computational(MODEXP_BASE_NATIVE_COST - 1),
        );

        let err = ModExpImpl::<false>::execute(
            &input,
            &mut output,
            &mut resources,
            &mut DummyOracle,
            &mut NullLogger,
            Global,
        )
        .expect_err("native, not ergs, should be exhausted");

        assert_out_of_native(err);
    }

    #[test]
    fn returns_out_of_native_when_conservative_native_gate_fails() {
        let base = vec![0x11; 64];
        let exponent = vec![0xff; 1];
        let modulus = vec![0x33; 64];
        let input = modexp_input(&base, &exponent, &modulus);
        let exp_highp = U256::from_be_slice(&exponent);
        let conservative_native = native_cost::<TestResources>(
            base.len() as u64,
            exponent.len() as u64,
            modulus.len() as u64,
            &exp_highp,
        )
        .expect("cost must be computable");
        let mut output = Vec::new();
        let mut resources = TestResources::from_ergs_and_native(
            Ergs::FORMAL_INFINITE,
            DecreasingNative::from_computational(conservative_native.as_u64() - 1),
        );

        let err = ModExpImpl::<false>::execute(
            &input,
            &mut output,
            &mut resources,
            &mut DummyOracle,
            &mut NullLogger,
            Global,
        )
        .expect_err("native conservative gate should fail first");

        assert_out_of_native(err);
    }

    #[test]
    fn returns_out_of_return_memory_when_output_does_not_fit() {
        let base = [2u8];
        let exponent = [2u8];
        let modulus = [1u8];
        let input = modexp_input(&base, &exponent, &modulus);
        let mut output = AlwaysFailDst;
        let mut resources = TestResources::FORMAL_INFINITE;

        let err = ModExpImpl::<false>::execute(
            &input,
            &mut output,
            &mut resources,
            &mut DummyOracle,
            &mut NullLogger,
            Global,
        )
        .expect_err("output buffer should be the failing resource");

        assert_out_of_return_memory(err);
    }

    #[test]
    fn exact_native_cost_accounts_for_initial_reduction() {
        let base = vec![0xff; 64]; // 2 words
        let exponent = [1u8];
        let modulus = vec![0x01; 32]; // 1 word, value != 1

        let native = native_cost_from_exp_data::<TestResources>(&base, &exponent, &modulus)
            .expect("cost must be computable");

        // quotient = base / modulus occupies at most base_digits - mod_digits + 1
        // = 2 - 1 + 1 = 2 words; the reduction FMA is quotient_digits * mod_digits.
        assert_eq!(
            native.as_u64(),
            MODEXP_BASE_NATIVE_COST + modexp_initial_reduction_native_cost(2, 1)
        );
    }

    #[test]
    fn exact_native_cost_scales_reduction_with_quotient_for_small_modulus() {
        // Oversized base reduced by a single-word modulus: the reduction FMA
        // scales with the quotient (~base_digits), not modulus_digits^2. With
        // exponent == 1 the exponent loop contributes no ops, so the reduction
        // is the only variable cost — this is the case the old modulus_digits^2
        // charge undercharged.
        let base = vec![0xff; 32 * 8]; // 8 words
        let exponent = [1u8];
        let modulus = vec![0x03]; // 1 word, value != 1

        let native = native_cost_from_exp_data::<TestResources>(&base, &exponent, &modulus)
            .expect("cost must be computable");

        // quotient_digits = 8 - 1 + 1 = 8, modulus_digits = 1.
        assert_eq!(
            native.as_u64(),
            MODEXP_BASE_NATIVE_COST + modexp_initial_reduction_native_cost(8, 1)
        );
    }

    #[test]
    fn exact_native_cost_skips_initial_reduction_for_modulus_one() {
        // `base^exp mod 1 == 0`: the delegated `modexp_inner` returns before
        // parsing the base or calling `modpow`, so no reduction runs and none
        // must be charged, however oversized the base is.
        let base = vec![0xff; 32 * 8];
        let exponent = [1u8];
        let modulus = [1u8];

        let native = native_cost_from_exp_data::<TestResources>(&base, &exponent, &modulus)
            .expect("cost must be computable");

        // exponent == 1 => no square/multiply ops, and modulus == 1 => no
        // reduction, so only the base cost remains.
        assert_eq!(native.as_u64(), MODEXP_BASE_NATIVE_COST);
    }

    #[test]
    fn exact_native_cost_skips_initial_reduction_when_base_has_fewer_words() {
        // Base occupies fewer 32-byte words than the modulus, so the delegated
        // implementation does not perform the initial reduction.
        let base = [1u8];
        let exponent = [1u8];
        let modulus = vec![0x01; 33]; // 2 words vs the base's 1 word

        let native = native_cost_from_exp_data::<TestResources>(&base, &exponent, &modulus)
            .expect("cost must be computable");

        assert_eq!(native.as_u64(), MODEXP_BASE_NATIVE_COST);
    }

    #[test]
    fn exact_native_cost_accounts_for_reduction_when_base_and_modulus_share_word_count() {
        // The base is numerically smaller than the modulus but occupies the same
        // number of 32-byte words, so the delegated implementation still runs the
        // initial reduction FMA (`current.digits >= modulus.digits`) and it must
        // be charged. Comparing values instead of word counts would undercharge
        // this common case (e.g. RSA verification with base < modulus).
        let base = [1u8];
        let exponent = [1u8];
        let modulus = [2u8];

        let native = native_cost_from_exp_data::<TestResources>(&base, &exponent, &modulus)
            .expect("cost must be computable");

        // quotient_digits = 1 - 1 + 1 = 1, modulus_digits = 1.
        assert_eq!(
            native.as_u64(),
            MODEXP_BASE_NATIVE_COST + modexp_initial_reduction_native_cost(1, 1)
        );
    }

    #[test]
    fn exact_native_cost_charges_ops_on_modulus_digits_not_base() {
        // After the initial reduction every square/multiply operates on a value
        // reduced mod `modulus`, so the per-op cost scales with modulus_digits^2,
        // not max(base, modulus)_digits^2. A large base with a small modulus and a
        // non-trivial exponent exercises this: the per-op charge must use the
        // single modulus word, while the base only feeds the initial reduction.
        let base = vec![0xff; 32 * 4]; // 4 words
        let exponent = [0x03u8]; // bits `11`: 1 square + 1 multiply
        let modulus = vec![0x03]; // 1 word, value != 1

        let native = native_cost_from_exp_data::<TestResources>(&base, &exponent, &modulus)
            .expect("cost must be computable");

        // 2 ops charged on modulus_digits = 1 (not base_digits = 4), plus the
        // quotient-scaled initial reduction (quotient_digits = 4 - 1 + 1 = 4).
        assert_eq!(
            native.as_u64(),
            modexp_native_from_ops(2, 1) + modexp_initial_reduction_native_cost(4, 1)
        );
    }

    #[test]
    fn conservative_native_upper_bounds_exact_native() {
        // The conservative gate must never under-approximate the exact charge,
        // even when the modulus has leading zero bytes that the exact
        // (materialized) charge strips away. Otherwise `resources.charge` could
        // fail after the conservative gate already passed.
        let cases: &[(&[u8], &[u8], &[u8])] = &[
            (&[0x07], &[0xff], &[0x00, 0x05]), // modulus strips to fewer words
            (&[0x01], &[0x01], &[0x02]),       // same word count, base < modulus
            (&[0xff; 64], &[0xff; 4], &[0x01; 32]),
            (&[0x00, 0x00, 0x01], &[0x01], &[0x00, 0x00, 0x03]),
            (&[0xff; 32 * 8], &[0x01], &[0x03]), // oversized base, single-word modulus
            (&[0xff; 32 * 8], &[0xff; 4], &[0x01]), // modulus == 1: exact skips reduction
            (&[0xff; 32 * 4], &[0xff; 8], &[0x00, 0x01]), // modulus == 1 with leading zero byte
        ];
        for (base, exp, modulus) in cases {
            let exact = native_cost_from_exp_data::<TestResources>(base, exp, modulus)
                .expect("exact cost must be computable")
                .as_u64();
            let exp_highp = U256::from_be_slice(exp);
            let conservative = native_cost::<TestResources>(
                base.len() as u64,
                exp.len() as u64,
                modulus.len() as u64,
                &exp_highp,
            )
            .expect("conservative cost must be computable")
            .as_u64();
            assert!(
                exact <= conservative,
                "exact {exact} exceeded conservative {conservative} for base={base:?} modulus={modulus:?}"
            );
        }
    }
}
