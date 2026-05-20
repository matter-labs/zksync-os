# Wide Div Rem Advice Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the specialized mulmod oracle advice with a generic wide div_rem advice (512-bit dividend, 256-bit divisor) that returns only the quotient. Use it for MULMOD and ADDMOD (overflow case). Also change the existing 256-bit div_rem advice to return quotient-only (same principle).

**Architecture:** New oracle query `U256_WIDE_DIV_REM_ADVICE_QUERY_ID` takes pointers to `(dividend_lo, dividend_hi, divisor)`, returns 8 u64 limbs (quotient only). Existing `U256_DIV_REM_ADVICE_QUERY_ID` is also changed to return 4 limbs (quotient only, no remainder). In both cases the guest derives remainder via `r = dividend - q * divisor` and verifies `0 <= r < divisor`. `MulmodExt` is replaced by `WideDivRemExt` throughout the trait/impl chain.

**Tech Stack:** Rust (no_std compatible), ruint algorithms, u256 crate, libfuzzer-sys for fuzz targets.

**Spec:** `docs/superpowers/specs/2026-05-13-wide-div-rem-advice-design.md`

---

### Task 1: Replace query ID and params structs

**Files:**
- Modify: `zk_ee/src/oracle/query_ids.rs:50-54`
- Modify: `zk_ee/src/utils/u256_arithmetic_advice.rs` (full file)

- [ ] **Step 1: Replace mulmod query ID with wide div_rem**

In `zk_ee/src/oracle/query_ids.rs`, replace lines 50-54:

```rust
/// Query to get wide div_rem hint (quotient) for a 512-bit dividend and 256-bit divisor.
/// Guest sends pointers to dividend_lo, dividend_hi, and divisor; host returns q_lo and q_hi.
/// Guest derives remainder r = dividend - q*divisor and verifies 0 <= r < divisor.
pub const U256_WIDE_DIV_REM_ADVICE_QUERY_ID: u32 = ADVICE_SUBSPACE_MASK | 0x31; // 0x40050031
```

- [ ] **Step 2: Replace mulmod params with wide div_rem params**

Replace the mulmod params section in `zk_ee/src/utils/u256_arithmetic_advice.rs` (lines 13-23) with:

```rust
/// Params for U256 wide div_rem oracle query (512-bit dividend, 256-bit divisor).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct U256WideDivRemAdviceParamsGeneric<W> {
    pub dividend_lo_ptr: W,
    pub dividend_hi_ptr: W,
    pub divisor_ptr: W,
}

pub type U256WideDivRemAdviceParams = U256WideDivRemAdviceParamsGeneric<u32>;
pub type U256WideDivRemAdviceParams64 = U256WideDivRemAdviceParamsGeneric<u64>;
```

- [ ] **Step 3: Build to verify no downstream breakage yet**

Run: `cargo build -p zk_ee 2>&1 | tail -5`
Expected: compiles (downstream crates not checked yet)

- [ ] **Step 4: Commit**

```bash
git add zk_ee/src/oracle/query_ids.rs zk_ee/src/utils/u256_arithmetic_advice.rs
git commit -m "feat: replace mulmod query ID and params with wide div_rem"
```

---

### Task 2: Replace MulmodExt trait with WideDivRemExt

**Files:**
- Modify: `zk_ee/src/system/base_system_functions.rs:388-459`

- [ ] **Step 1: Replace MulmodExt with WideDivRemExt trait**

In `zk_ee/src/system/base_system_functions.rs`, replace the `MulmodExt` trait (lines 452-459) with:

```rust
pub trait WideDivRemExt {
    fn execute<O: IOOracle>(
        dividend_lo: &mut u256::U256,
        dividend_hi: &mut u256::U256,
        divisor: &mut u256::U256,
        oracle: &mut O,
    );
}
```

- [ ] **Step 2: Update SystemFunctionsExt associated type and method**

In `SystemFunctionsExt` (line 392), replace:

```rust
    type Mulmod: MulmodExt;
```

with:

```rust
    type WideDivRem: WideDivRemExt;
```

Replace the `u256_mulmod` method (lines 434-441) with:

```rust
    fn u256_wide_div_rem<O: IOOracle>(
        dividend_lo: &mut u256::U256,
        dividend_hi: &mut u256::U256,
        divisor: &mut u256::U256,
        oracle: &mut O,
    ) {
        Self::WideDivRem::execute(dividend_lo, dividend_hi, divisor, oracle)
    }
```

- [ ] **Step 3: Build zk_ee**

Run: `cargo build -p zk_ee 2>&1 | tail -5`
Expected: compiles (downstream will fail until wired up)

- [ ] **Step 4: Commit**

```bash
git add zk_ee/src/system/base_system_functions.rs
git commit -m "feat: replace MulmodExt with WideDivRemExt trait"
```

---

### Task 3: Make 256-bit div_rem quotient-only

**Files:**
- Modify: `basic_system/src/system_functions/u256_advice.rs` (verify + advice functions)
- Modify: `callable_oracles/src/arithmetic/mod.rs` (u256_div_rem_output)

- [ ] **Step 1: Update verify_div_rem_hint to derive remainder from quotient**

In `basic_system/src/system_functions/u256_advice.rs`, replace `verify_div_rem_hint` (lines 11-30) with:

```rust
#[must_use]
pub fn verify_div_rem_hint(
    dividend: &U256,
    divisor: &U256,
    q_limbs: [u64; 4],
) -> (bool, U256) {
    let mut prod_lo = U256::from_limbs(q_limbs);
    let mut prod_hi = U256::from_limbs(q_limbs);
    prod_lo.widening_mul_assign_into(&mut prod_hi, divisor);

    if !prod_hi.is_zero() {
        return (false, U256::zero());
    }

    // r = dividend - q * divisor
    let mut remainder = dividend.clone();
    let borrow = remainder.overflowing_sub_assign(&prod_lo);
    if borrow {
        return (false, U256::zero());
    }

    if remainder >= *divisor {
        return (false, U256::zero());
    }

    (true, remainder)
}
```

- [ ] **Step 2: Update u256_div_rem_with_advice to read quotient only**

Replace the oracle response reading and verification in `u256_div_rem_with_advice` (lines 152-163) with:

```rust
    let q_limbs = read_limbs_from_oracle_response(&mut it);

    let (valid, remainder) = verify_div_rem_hint(dividend_or_quotient, divisor_or_remainder, q_limbs);
    assert!(valid);

    *dividend_or_quotient = U256::from_limbs(q_limbs);
    *divisor_or_remainder = remainder;
```

- [ ] **Step 3: Update u256_div_rem_output in callable_oracles to return quotient only**

In `callable_oracles/src/arithmetic/mod.rs`, replace `u256_div_rem_output` (lines 72-82) with:

```rust
fn u256_div_rem_output(
    mut dividend: [u64; 4],
    mut divisor: [u64; 4],
) -> Box<dyn ExactSizeIterator<Item = usize> + 'static + Send + Sync> {
    ruint::algorithms::div(&mut dividend, &mut divisor);

    // Return quotient only (4 limbs), guest derives remainder
    let mut result = Vec::with_capacity(4);
    push_limbs(&mut result, &dividend);
    Box::new(UsizeSliceIteratorOwned::new(result.into_boxed_slice()))
}
```

- [ ] **Step 4: Build**

Run: `cargo build -p basic_system -p callable_oracles 2>&1 | tail -5`
Expected: compiles

- [ ] **Step 5: Commit**

```bash
git add basic_system/src/system_functions/u256_advice.rs callable_oracles/src/arithmetic/mod.rs
git commit -m "feat: make 256-bit div_rem advice return quotient only"
```

---

### Task 4: Replace mulmod with wide div_rem verification and advice

**Files:**
- Modify: `basic_system/src/system_functions/u256_advice.rs` (full rewrite of mulmod section)

- [ ] **Step 1: Replace verify_mulmod_hint with verify_wide_div_rem_hint**

In `basic_system/src/system_functions/u256_advice.rs`, replace `verify_mulmod_hint` (lines 32-72) with:

```rust
#[must_use]
pub fn verify_wide_div_rem_hint(
    dividend_lo: &U256,
    dividend_hi: &U256,
    divisor: &U256,
    q_lo_limbs: [u64; 4],
    q_hi_limbs: [u64; 4],
) -> (bool, U256) {
    // Compute q * divisor as 512-bit
    let mut qd_lo = U256::from_limbs(q_lo_limbs);
    let mut qd_mid = U256::from_limbs(q_lo_limbs);
    qd_lo.widening_mul_assign_into(&mut qd_mid, divisor);

    let mut qd_hi_lo = U256::from_limbs(q_hi_limbs);
    let mut qd_hi_hi = U256::from_limbs(q_hi_limbs);
    qd_hi_lo.widening_mul_assign_into(&mut qd_hi_hi, divisor);

    // Accumulate into (qd_lo, qd_mid) — the 512-bit product q*d
    let c1 = qd_mid.overflowing_add_assign(&qd_hi_lo);
    if c1 || !qd_hi_hi.is_zero() {
        return (false, U256::zero());
    }

    // Compute r = dividend - q*d (512-bit subtraction)
    let mut r_lo = dividend_lo.clone();
    let borrow_lo = r_lo.overflowing_sub_assign(&qd_lo);
    let mut r_hi = dividend_hi.clone();
    let borrow_mid = r_hi.overflowing_sub_assign(&qd_mid);
    let borrow_final = if borrow_lo {
        r_hi.overflowing_sub_assign(&U256::one())
    } else {
        false
    };

    // r must be non-negative (no borrow) and fit in 256 bits (r_hi == 0)
    if (borrow_mid | borrow_final) || !r_hi.is_zero() {
        return (false, U256::zero());
    }

    if r_lo >= *divisor {
        return (false, U256::zero());
    }

    (true, r_lo)
}
```

- [ ] **Step 2: Replace MulmodImpl with WideDivRemImpl**

Replace `MulmodImpl` struct and its `MulmodExt` impl (lines 75, 91-104) with:

```rust
pub struct WideDivRemImpl<const USE_ADVICE: bool>;

impl<const USE_ADVICE: bool> WideDivRemExt for WideDivRemImpl<USE_ADVICE> {
    fn execute<O: IOOracle>(
        dividend_lo: &mut U256,
        dividend_hi: &mut U256,
        divisor: &mut U256,
        oracle: &mut O,
    ) {
        if USE_ADVICE {
            u256_wide_div_rem_with_advice(dividend_lo, dividend_hi, divisor, oracle)
        } else {
            u256_wide_div_rem_software(dividend_lo, dividend_hi, divisor)
        }
    }
}
```

- [ ] **Step 3: Implement software fallback**

```rust
fn u256_wide_div_rem_software(
    dividend_lo: &mut U256,
    dividend_hi: &mut U256,
    divisor: &mut U256,
) {
    let mut product = [0u64; 8];
    product[..4].copy_from_slice(dividend_lo.as_limbs());
    product[4..].copy_from_slice(dividend_hi.as_limbs());
    let mut d = *divisor.as_limbs();
    ruint::algorithms::div(&mut product, &mut d);
    // Remainder is in d, store it in divisor
    *divisor = U256::from_limbs(d);
}
```

- [ ] **Step 4: Implement oracle advice path**

Replace `u256_mulmod_with_advice` (lines 167-221) with:

```rust
#[inline]
fn u256_wide_div_rem_with_advice<O: IOOracle>(
    dividend_lo: &mut U256,
    dividend_hi: &mut U256,
    divisor: &mut U256,
    oracle: &mut O,
) {
    assert!(!divisor.is_zero());

    #[cfg(target_pointer_width = "32")]
    let mut it = {
        use zk_ee::utils::u256_arithmetic_advice::U256WideDivRemAdviceParams;
        let params = U256WideDivRemAdviceParams {
            dividend_lo_ptr: (dividend_lo as *const U256).addr() as u32,
            dividend_hi_ptr: (dividend_hi as *const U256).addr() as u32,
            divisor_ptr: (divisor as *const U256).addr() as u32,
        };
        oracle
            .raw_query(
                U256_WIDE_DIV_REM_ADVICE_QUERY_ID,
                &((&params as *const U256WideDivRemAdviceParams).addr() as u32),
            )
            .expect("wide_div_rem oracle query failed")
    };

    #[cfg(target_pointer_width = "64")]
    let mut it = {
        use zk_ee::utils::u256_arithmetic_advice::U256WideDivRemAdviceParams64;
        let params = U256WideDivRemAdviceParams64 {
            dividend_lo_ptr: (dividend_lo as *const U256).addr() as u64,
            dividend_hi_ptr: (dividend_hi as *const U256).addr() as u64,
            divisor_ptr: (divisor as *const U256).addr() as u64,
        };
        oracle
            .raw_query(
                U256_WIDE_DIV_REM_ADVICE_QUERY_ID,
                &((&params as *const U256WideDivRemAdviceParams64).addr() as u64),
            )
            .expect("wide_div_rem oracle query failed")
    };

    let q_lo_limbs = read_limbs_from_oracle_response(&mut it);
    let q_hi_limbs = read_limbs_from_oracle_response(&mut it);

    let (valid, remainder) =
        verify_wide_div_rem_hint(dividend_lo, dividend_hi, divisor, q_lo_limbs, q_hi_limbs);
    assert!(valid);

    *divisor = remainder;
}
```

- [ ] **Step 5: Update imports**

At the top of the file, replace:
- `U256_MULMOD_ADVICE_QUERY_ID` → `U256_WIDE_DIV_REM_ADVICE_QUERY_ID`
- `MulmodExt` → `WideDivRemExt`
- Remove `U256MulmodAdviceParams` / `U256MulmodAdviceParams64` imports (now used inline with `use` in cfg blocks)

- [ ] **Step 6: Build basic_system**

Run: `cargo build -p basic_system 2>&1 | tail -5`
Expected: compiles

- [ ] **Step 7: Commit**

```bash
git add basic_system/src/system_functions/u256_advice.rs
git commit -m "feat: implement wide div_rem verification and advice"
```

---

### Task 5: Wire WideDivRem in basic_system and update opcode handlers

**Files:**
- Modify: `basic_system/src/system_functions/mod.rs:63-70`
- Modify: `evm_interpreter/src/instructions/arithmetic.rs:89-106`
- Modify: `evm_interpreter/src/interpreter.rs:164`

- [ ] **Step 1: Update associated type wiring**

In `basic_system/src/system_functions/mod.rs`, replace line 69:

```rust
    type Mulmod = u256_advice::MulmodImpl<USE_ADVICE>;
```

with:

```rust
    type WideDivRem = u256_advice::WideDivRemImpl<USE_ADVICE>;
```

- [ ] **Step 2: Update MULMOD opcode handler**

In `evm_interpreter/src/instructions/arithmetic.rs`, replace the `mulmod` method (lines 97-106) with:

```rust
    pub fn mulmod(&mut self, system: &mut System<S>) -> InstructionResult
    where
        S::IO: IOSubsystemExt,
    {
        self.gas
            .spend_gas_and_native(gas_constants::MID, MULMOD_NATIVE_COST)?;
        let ((op1, op2), op3) = self.stack.pop_2_mut_and_peek()?;
        if op3.is_zero() {
            return Ok(());
        }
        // Compute a * b → (product_lo, product_hi)
        let mut product_lo = U256::from_limbs(*op1.as_limbs());
        let mut product_hi = U256::from_limbs(*op1.as_limbs());
        product_lo.widening_mul_assign_into(&mut product_hi, op2);
        // Wide div_rem: divisor (op3) receives remainder
        S::SystemFunctionsExt::u256_wide_div_rem(
            &mut product_lo,
            &mut product_hi,
            op3,
            system.io.oracle(),
        );
        // op3 now holds the remainder = mulmod result
        Ok(())
    }
```

- [ ] **Step 3: Update ADDMOD opcode handler**

Replace the `addmod` method (lines 89-95) with:

```rust
    pub fn addmod(&mut self, system: &mut System<S>) -> InstructionResult
    where
        S::IO: IOSubsystemExt,
    {
        self.gas
            .spend_gas_and_native(gas_constants::MID, ADDMOD_NATIVE_COST)?;
        let ((op1, op2), op3) = self.stack.pop_2_mut_and_peek()?;
        if op3.is_zero() {
            return Ok(());
        }
        let carry = op1.overflowing_add_assign(op2);
        if carry {
            // a + b overflowed: use wide div_rem with 257-bit dividend
            let mut hi = U256::one();
            S::SystemFunctionsExt::u256_wide_div_rem(
                op1,
                &mut hi,
                op3,
                system.io.oracle(),
            );
            // op3 now holds the remainder
        } else {
            // No overflow: simple conditional subtract
            if op1 >= op3 {
                U256::div_rem(op1, op3);
                // op3 now holds the remainder
            } else {
                // op1 < op3: result is op1, move it to op3
                core::mem::swap(op1, op3);
            }
        }
        Ok(())
    }
```

- [ ] **Step 4: Update addmod dispatch to pass system**

In `evm_interpreter/src/interpreter.rs`, line 164, change:

```rust
                    opcodes::ADDMOD => self.addmod(),
```

to:

```rust
                    opcodes::ADDMOD => self.addmod(system),
```

- [ ] **Step 5: Build all affected crates**

Run: `cargo build -p evm_interpreter -p basic_system 2>&1 | tail -5`
Expected: compiles

- [ ] **Step 6: Commit**

```bash
git add basic_system/src/system_functions/mod.rs evm_interpreter/src/instructions/arithmetic.rs evm_interpreter/src/interpreter.rs
git commit -m "feat: wire WideDivRem for MULMOD and ADDMOD opcodes"
```

---

### Task 6: Update callable_oracles host-side handlers

**Files:**
- Modify: `callable_oracles/src/arithmetic/mod.rs`

- [ ] **Step 1: Replace u256_mulmod_output with u256_wide_div_rem_output**

Replace the `u256_mulmod_output` function (lines 89-104) with:

```rust
fn u256_wide_div_rem_output(
    dividend_lo: [u64; 4],
    dividend_hi: [u64; 4],
    mut divisor: [u64; 4],
) -> Box<dyn ExactSizeIterator<Item = usize> + 'static + Send + Sync> {
    let mut dividend = [0u64; 8];
    dividend[..4].copy_from_slice(&dividend_lo);
    dividend[4..].copy_from_slice(&dividend_hi);

    ruint::algorithms::div(&mut dividend, &mut divisor);

    // Return quotient only (8 limbs), no remainder
    let mut result = Vec::with_capacity(8);
    push_limbs(&mut result, &dividend);
    Box::new(UsizeSliceIteratorOwned::new(result.into_boxed_slice()))
}
```

- [ ] **Step 2: Update imports**

Replace `U256_MULMOD_ADVICE_QUERY_ID` with `U256_WIDE_DIV_REM_ADVICE_QUERY_ID` and `U256MulmodAdviceParams` / `U256MulmodAdviceParams64` with `U256WideDivRemAdviceParams` / `U256WideDivRemAdviceParams64` in the import block.

- [ ] **Step 3: Update ArithmeticQuery handler**

Replace the mulmod `if` block in `ArithmeticQuery::process_buffered_query` (lines 210-221) with:

```rust
        if query_id == U256_WIDE_DIV_REM_ADVICE_QUERY_ID {
            let arg_ptr = extract_single_ptr(query);
            assert!(arg_ptr.is_multiple_of(4));
            const { assert!(core::mem::align_of::<U256WideDivRemAdviceParams>() <= 4) }
            const { assert!(core::mem::size_of::<U256WideDivRemAdviceParams>().is_multiple_of(4)) }
            let params: U256WideDivRemAdviceParams =
                unsafe { read_struct(memory, arg_ptr as u32) }.unwrap();
            let dividend_lo = read_u256_from_guest(memory, params.dividend_lo_ptr);
            let dividend_hi = read_u256_from_guest(memory, params.dividend_hi_ptr);
            let divisor = read_u256_from_guest(memory, params.divisor_ptr);
            return u256_wide_div_rem_output(dividend_lo, dividend_hi, divisor);
        }
```

- [ ] **Step 4: Update NativeArithmeticQuery handler**

Replace the mulmod `if` block in `NativeArithmeticQuery::process_buffered_query` (lines 259-266) with:

```rust
        if query_id == U256_WIDE_DIV_REM_ADVICE_QUERY_ID {
            let arg_ptr = extract_single_ptr(query);
            let params: U256WideDivRemAdviceParams64 = read_host_struct(arg_ptr as u64);
            let dividend_lo = read_u256_from_host(params.dividend_lo_ptr);
            let dividend_hi = read_u256_from_host(params.dividend_hi_ptr);
            let divisor = read_u256_from_host(params.divisor_ptr);
            return u256_wide_div_rem_output(dividend_lo, dividend_hi, divisor);
        }
```

- [ ] **Step 5: Update supported_query_ids for both structs**

In both `ArithmeticQuery` and `NativeArithmeticQuery`, replace `U256_MULMOD_ADVICE_QUERY_ID` with `U256_WIDE_DIV_REM_ADVICE_QUERY_ID` in the `supported_query_ids()` vec.

- [ ] **Step 6: Build and test**

Run: `cargo build -p callable_oracles 2>&1 | tail -5`
Expected: compiles

Run: `cargo test -p callable_oracles 2>&1 | tail -30`
Expected: existing mulmod tests will fail (updated in next task)

- [ ] **Step 7: Commit**

```bash
git add callable_oracles/src/arithmetic/mod.rs
git commit -m "feat: replace mulmod oracle handler with wide div_rem"
```

---

### Task 7: Update callable_oracles tests

**Files:**
- Modify: `callable_oracles/src/arithmetic/mod.rs` (test module)

- [ ] **Step 1: Update existing div_rem test for quotient-only response**

Update `u256_div_rem_via_native_query` to expect 4 limbs (quotient only) instead of 8:

```rust
    #[test]
    fn u256_div_rem_via_native_query() {
        let dividend = [10u64, 0, 0, 0];
        let divisor = [3u64, 0, 0, 0];
        let params = U256DivRemAdviceParams64 {
            dividend_ptr: dividend.as_ptr().addr() as u64,
            divisor_ptr: divisor.as_ptr().addr() as u64,
        };
        let output: Vec<usize> = NativeArithmeticQuery
            .process_buffered_query(
                U256_DIV_REM_ADVICE_QUERY_ID,
                vec![(&params as *const U256DivRemAdviceParams64).addr()],
                &DummyMemorySource,
            )
            .collect();
        // Quotient only: 4 limbs
        assert_eq!(output, vec![3, 0, 0, 0]);
    }
```

- [ ] **Step 2: Replace mulmod tests with wide div_rem tests**

Replace `u256_mulmod_via_native_query` and `u256_mulmod_large_values` tests with:

```rust
    #[test]
    fn u256_wide_div_rem_via_native_query() {
        // 35 / 6: q=5, r=5
        let dividend_lo = [35u64, 0, 0, 0];
        let dividend_hi = [0u64, 0, 0, 0];
        let divisor = [6u64, 0, 0, 0];
        let params = U256WideDivRemAdviceParams64 {
            dividend_lo_ptr: dividend_lo.as_ptr().addr() as u64,
            dividend_hi_ptr: dividend_hi.as_ptr().addr() as u64,
            divisor_ptr: divisor.as_ptr().addr() as u64,
        };
        let output: Vec<usize> = NativeArithmeticQuery
            .process_buffered_query(
                U256_WIDE_DIV_REM_ADVICE_QUERY_ID,
                vec![(&params as *const U256WideDivRemAdviceParams64).addr()],
                &DummyMemorySource,
            )
            .collect();
        // Quotient only: 8 limbs
        assert_eq!(output.len(), 8);
        assert_eq!(output[0], 5); // q_lo limb 0
        assert_eq!(&output[1..], &[0, 0, 0, 0, 0, 0, 0]); // rest zero
    }

    #[test]
    fn u256_wide_div_rem_large_dividend() {
        // a = 2^128, b = 2^128 → product = 2^256
        // product / (2^128 + 1): q = 2^128 - 1, r = 1
        let dividend_lo = [0u64, 0, 0, 0]; // low 256 bits of 2^256 = 0
        let dividend_hi = [1u64, 0, 0, 0]; // high 256 bits of 2^256 = 1
        let divisor = [1u64, 0, 1, 0]; // 2^128 + 1
        let params = U256WideDivRemAdviceParams64 {
            dividend_lo_ptr: dividend_lo.as_ptr().addr() as u64,
            dividend_hi_ptr: dividend_hi.as_ptr().addr() as u64,
            divisor_ptr: divisor.as_ptr().addr() as u64,
        };
        let output: Vec<usize> = NativeArithmeticQuery
            .process_buffered_query(
                U256_WIDE_DIV_REM_ADVICE_QUERY_ID,
                vec![(&params as *const U256WideDivRemAdviceParams64).addr()],
                &DummyMemorySource,
            )
            .collect();
        assert_eq!(output.len(), 8);
        // q = 2^128 - 1 = [u64::MAX, u64::MAX, 0, 0]
        assert_eq!(output[0], u64::MAX as usize);
        assert_eq!(output[1], u64::MAX as usize);
        assert_eq!(&output[2..8], &[0, 0, 0, 0, 0, 0]);
    }
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p callable_oracles 2>&1 | tail -30`
Expected: all tests pass

- [ ] **Step 3: Commit**

```bash
git add callable_oracles/src/arithmetic/mod.rs
git commit -m "test: update callable_oracles tests for wide div_rem"
```

---

### Task 8: Update fuzz targets

**Files:**
- Modify: `tests/fuzzer/fuzz/fuzz_targets/precompiles_diff/u256_divrem.rs`
- Rename: `tests/fuzzer/fuzz/fuzz_targets/precompiles_diff/u256_mulmod.rs` → `u256_wide_divrem.rs`
- Modify: `tests/fuzzer/fuzz/Cargo.toml`

- [ ] **Step 0: Update u256_divrem fuzz target for quotient-only verify**

Update `u256_divrem.rs` to use the new `(bool, U256)` return from `verify_div_rem_hint`. Replace all calls from:

```rust
assert!(verify_div_rem_hint(&dividend, &divisor, q_limbs, r_limbs));
```

to:

```rust
let (valid, remainder) = verify_div_rem_hint(&dividend, &divisor, q_limbs);
assert!(valid);
assert_eq!(*remainder.as_limbs(), r_limbs, "remainder mismatch vs ruint");
```

And update negative tests similarly — pass only `bad_q` (no `bad_r` needed since remainder is derived):

```rust
    if input.bad_q != q_limbs {
        let (valid, _) = verify_div_rem_hint(&dividend, &divisor, input.bad_q);
        assert!(!valid, "verification accepted bad quotient");
    }
```

Remove the `bad_r` field and all `bad_r`-specific negative tests from the `Input` struct — with quotient-only, bad remainder is not a separate test vector anymore.

- [ ] **Step 1: Rename and rewrite fuzz target**

Delete `u256_mulmod.rs`, create `u256_wide_divrem.rs`:

```rust
#![no_main]

use arbitrary::Arbitrary;
use basic_system::system_functions::u256_advice::verify_wide_div_rem_hint;
use libfuzzer_sys::fuzz_target;
use u256::U256;

#[derive(Arbitrary, Debug, Clone, Copy)]
enum ValueKind {
    Random,
    Zero,
    One,
    Max,
    PowerOfTwo,
    Small,
}

#[derive(Arbitrary, Debug, Clone, Copy)]
enum BadHintKind {
    Random,
    OffByOneQLo,
    OffByOneQHi,
    SwapQLoAndQHi,
}

#[derive(Arbitrary, Debug)]
struct Input {
    dividend_lo_kind: ValueKind,
    dividend_hi_kind: ValueKind,
    divisor_kind: ValueKind,
    bad_hint_kind: BadHintKind,
    dividend_lo_seed: [u64; 4],
    dividend_hi_seed: [u64; 4],
    divisor_seed: [u64; 4],
    bad_seed_lo: [u64; 4],
    bad_seed_hi: [u64; 4],
    power_shift: u8,
}

fn shape_value(seed: [u64; 4], kind: ValueKind, power_shift: u8) -> [u64; 4] {
    match kind {
        ValueKind::Random => seed,
        ValueKind::Zero => [0; 4],
        ValueKind::One => [1, 0, 0, 0],
        ValueKind::Max => [u64::MAX; 4],
        ValueKind::PowerOfTwo => {
            let bit = (power_shift as usize) % 256;
            let limb = bit / 64;
            let shift = bit % 64;
            let mut v = [0u64; 4];
            v[limb] = 1u64 << shift;
            v
        }
        ValueKind::Small => {
            let small = (seed[0] % 255) + 1;
            [small, 0, 0, 0]
        }
    }
}

fn wrapping_inc(limbs: [u64; 4]) -> [u64; 4] {
    let mut v = limbs;
    let (val, overflow) = v[0].overflowing_add(1);
    v[0] = val;
    if overflow {
        v[1] = v[1].wrapping_add(1);
    }
    v
}

fn fuzz(input: Input) {
    let div_lo = shape_value(input.dividend_lo_seed, input.dividend_lo_kind, input.power_shift);
    let div_hi = shape_value(input.dividend_hi_seed, input.dividend_hi_kind, input.power_shift);
    let divisor_raw = shape_value(input.divisor_seed, input.divisor_kind, input.power_shift);

    if divisor_raw == [0; 4] {
        return;
    }

    // Compute correct quotient using ruint
    let mut dividend = [0u64; 8];
    dividend[..4].copy_from_slice(&div_lo);
    dividend[4..].copy_from_slice(&div_hi);
    let mut d = divisor_raw;
    ruint::algorithms::div(&mut dividend, &mut d);

    let q_lo_limbs: [u64; 4] = dividend[..4].try_into().unwrap();
    let q_hi_limbs: [u64; 4] = dividend[4..].try_into().unwrap();

    let dividend_lo = U256::from_limbs(div_lo);
    let dividend_hi = U256::from_limbs(div_hi);
    let divisor = U256::from_limbs(divisor_raw);

    // Positive: correct hint must pass
    let (valid, remainder) =
        verify_wide_div_rem_hint(&dividend_lo, &dividend_hi, &divisor, q_lo_limbs, q_hi_limbs);
    assert!(valid);

    // Compare remainder against ruint
    assert_eq!(*remainder.as_limbs(), d, "remainder mismatch");

    // Construct bad hints
    let (bad_q_lo, bad_q_hi) = match input.bad_hint_kind {
        BadHintKind::Random => (input.bad_seed_lo, input.bad_seed_hi),
        BadHintKind::OffByOneQLo => (wrapping_inc(q_lo_limbs), q_hi_limbs),
        BadHintKind::OffByOneQHi => (q_lo_limbs, wrapping_inc(q_hi_limbs)),
        BadHintKind::SwapQLoAndQHi => (q_hi_limbs, q_lo_limbs),
    };

    // Negative: bad q_lo only
    if bad_q_lo != q_lo_limbs {
        let (valid, _) =
            verify_wide_div_rem_hint(&dividend_lo, &dividend_hi, &divisor, bad_q_lo, q_hi_limbs);
        assert!(!valid, "verification accepted bad q_lo");
    }

    // Negative: bad q_hi only
    if bad_q_hi != q_hi_limbs {
        let (valid, _) =
            verify_wide_div_rem_hint(&dividend_lo, &dividend_hi, &divisor, q_lo_limbs, bad_q_hi);
        assert!(!valid, "verification accepted bad q_hi");
    }

    // Negative: both bad
    if bad_q_lo != q_lo_limbs || bad_q_hi != q_hi_limbs {
        let (valid, _) =
            verify_wide_div_rem_hint(&dividend_lo, &dividend_hi, &divisor, bad_q_lo, bad_q_hi);
        assert!(!valid, "verification accepted bad hint");
    }
}

fuzz_target!(|input: Input| {
    fuzz(input);
});
```

- [ ] **Step 2: Update Cargo.toml**

Replace the `precompiles_diff_u256_mulmod` bin entry with:

```toml
[[bin]]
name = "precompiles_diff_u256_wide_divrem"
path = "fuzz_targets/precompiles_diff/u256_wide_divrem.rs"
test = false
doc = false
bench = false
```

- [ ] **Step 3: Build and smoke test**

Run:
```bash
cd tests/fuzzer && \
ZKSYNC_USE_CUDA_STUBS=true RUST_MIN_STACK=16777216 \
cargo fuzz build precompiles_diff_u256_wide_divrem 2>&1 | tail -5
```
Expected: compiles

Run:
```bash
ZKSYNC_USE_CUDA_STUBS=true RUST_MIN_STACK=16777216 \
cargo fuzz run precompiles_diff_u256_wide_divrem -- -max_total_time=10 2>&1 | tail -3
```
Expected: zero failures

- [ ] **Step 4: Commit**

```bash
git add tests/fuzzer/fuzz/fuzz_targets/precompiles_diff/ tests/fuzzer/fuzz/Cargo.toml
git commit -m "test: replace mulmod fuzz target with wide div_rem"
```

---

### Task 9: Full workspace build and integration test

**Files:** None (verification only)

- [ ] **Step 1: Build workspace**

Run: `cargo build 2>&1 | tail -10`
Expected: compiles (ignore CUDA errors if no GPU; use `ZKSYNC_USE_CUDA_STUBS=true`)

- [ ] **Step 2: Run workspace tests**

Run: `cargo test --workspace 2>&1 | tail -20`
Expected: all pass

- [ ] **Step 3: Run callable_oracles tests specifically**

Run: `cargo test -p callable_oracles 2>&1 | tail -20`
Expected: all pass including new wide div_rem tests

- [ ] **Step 4: Run both fuzz targets**

```bash
cd tests/fuzzer && \
ZKSYNC_USE_CUDA_STUBS=true RUST_MIN_STACK=16777216 \
cargo fuzz run precompiles_diff_u256_divrem -- -max_total_time=10 2>&1 | tail -3
```

```bash
ZKSYNC_USE_CUDA_STUBS=true RUST_MIN_STACK=16777216 \
cargo fuzz run precompiles_diff_u256_wide_divrem -- -max_total_time=10 2>&1 | tail -3
```

Expected: zero failures on both

- [ ] **Step 5: Clippy**

Run: `cargo clippy --all -- -D warnings 2>&1 | tail -10`
Expected: no warnings

- [ ] **Step 6: Final commit if any fixups**

```bash
git add -A && git commit -m "chore: fixups from integration testing"
```
