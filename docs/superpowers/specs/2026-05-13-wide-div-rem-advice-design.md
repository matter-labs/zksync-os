# Wide Div Rem Advice

**Date**: 2026-05-13
**Status**: Draft

## Summary

Replace the specialized mulmod oracle advice with a generic wide div_rem advice that takes a 512-bit dividend and 256-bit divisor, returning only the quotient. The guest derives the remainder via subtraction. This covers mulmod, addmod-with-overflow, and any future wide-result operation with a single oracle query type.

## Motivation

The current mulmod advice is a special-purpose oracle query that receives `(a, b, m)` and returns `(q_lo, q_hi, r)`. This couples the oracle to one specific operation. A generic wide div_rem is more reusable and returns less data (no remainder — guest derives it).

## Oracle Interface

### New query

- **ID**: `U256_WIDE_DIV_REM_ADVICE_QUERY_ID` (replaces `U256_MULMOD_ADVICE_QUERY_ID`)
- **Params struct** (pointer-based, same pattern as existing div_rem):

```rust
#[repr(C)]
pub struct U256WideDivRemAdviceParamsGeneric<W> {
    pub dividend_lo_ptr: W,  // &U256 — low 256 bits
    pub dividend_hi_ptr: W,  // &U256 — high 256 bits
    pub divisor_ptr: W,      // &U256
}
```

- **Response**: 8 u64 limbs = `q_lo[4]` + `q_hi[4]` (512-bit quotient, no remainder)

### Existing query (updated to quotient-only)

`U256_DIV_REM_ADVICE_QUERY_ID` stays for 256-bit DIV/MOD opcodes, but its response changes from 8 limbs (q + r) to 4 limbs (q only). Guest derives remainder via `r = dividend - q * divisor`.

### Removed

- `U256_MULMOD_ADVICE_QUERY_ID`
- `U256MulmodAdviceParams` / `U256MulmodAdviceParams64`
- `verify_mulmod_hint`

## Guest-Side Verification

```
fn verify_wide_div_rem_hint(
    dividend_lo: &U256,
    dividend_hi: &U256,
    divisor: &U256,
    q_lo_limbs: [u64; 4],
    q_hi_limbs: [u64; 4],
) -> bool
```

Steps:
1. Compute `q * divisor` as 512-bit (two widening multiplies + cross-term addition)
2. Compute `r = dividend - q * divisor` via 512-bit subtraction
3. Check: no borrow (dividend >= q*divisor)
4. Check: r_hi == 0 (remainder fits in 256 bits)
5. Check: r_lo < divisor

Returns the 256-bit remainder on success (or just `bool` — caller reads r from the subtraction result).

## Opcode Integration

### MULMOD (uses wide div_rem)
1. Guest computes `a * b` → `(product_lo, product_hi)` via `widening_mul_assign_into`
2. Sends `(product_lo, product_hi, modulus)` to wide div_rem oracle
3. Oracle returns `(q_lo, q_hi)`
4. Guest runs `verify_wide_div_rem_hint`, gets remainder
5. Remainder is the mulmod result

### ADDMOD (uses wide div_rem only on overflow)
1. Guest computes `a + b` → `(sum, carry_bit)`
2. If no carry: guest does conditional subtract (current behavior, no oracle)
3. If carry: guest constructs dividend `(sum, U256::one())` and sends to wide div_rem oracle
4. Oracle returns `(q_lo, q_hi)`
5. Guest verifies, gets remainder

### DIV/MOD/SDIV/SMOD (unchanged)
Continue using existing 256-bit `U256_DIV_REM_ADVICE_QUERY_ID`.

## SystemFunctionsExt Changes

Replace `Mulmod` associated type with `WideDivRem`:

```rust
pub trait WideDivRemExt {
    fn execute<O: IOOracle>(
        dividend_lo: &mut U256,
        dividend_hi: &mut U256,
        divisor: &mut U256,
        oracle: &mut O,
    );
}
```

`basic_system` provides `WideDivRemImpl<USE_ADVICE>`:
- `USE_ADVICE=true`: oracle query + verification
- `USE_ADVICE=false`: software `ruint::algorithms::div` on 512-bit dividend

## Host-Side (callable_oracles)

In `ArithmeticQuery` and `NativeArithmeticQuery`:
- Replace `U256_MULMOD_ADVICE_QUERY_ID` handler with `U256_WIDE_DIV_REM_ADVICE_QUERY_ID`
- Read `(dividend_lo, dividend_hi, divisor)` from params struct
- Assemble 8-limb dividend `[lo[0..4], hi[0..4]]`
- Call `ruint::algorithms::div`
- Return quotient (8 limbs), no remainder

## Fuzzing

Update existing fuzz targets:
- `u256_mulmod` → `u256_wide_divrem`: test with arbitrary 512-bit dividends
- Positive test: correct quotient passes verification
- Negative tests: corrupt q_lo only, q_hi only, both
- Structured input generation: edge cases (dividend < divisor, exact division, max values, powers of two)

## Files Changed

- `zk_ee/src/oracle/query_ids.rs` — replace mulmod query ID with wide div_rem
- `zk_ee/src/utils/u256_arithmetic_advice.rs` — replace mulmod params with wide div_rem params
- `zk_ee/src/system/base_system_functions.rs` — replace `MulmodExt` trait with `WideDivRemExt`
- `basic_system/src/system_functions/u256_advice.rs` — replace mulmod impl with wide div_rem impl, update addmod
- `basic_system/src/system_functions/mod.rs` — wire `WideDivRem` associated type
- `evm_interpreter/src/instructions/arithmetic.rs` — update MULMOD and ADDMOD opcode handlers
- `callable_oracles/src/arithmetic/mod.rs` — replace mulmod handler with wide div_rem handler
- `tests/fuzzer/fuzz/fuzz_targets/precompiles_diff/u256_mulmod.rs` → `u256_wide_divrem.rs`
