use super::*;
use crate::u256_helpers::*;
use native_resource_constants::*;
use zk_ee::system::{IOSubsystemExt, System, SystemFunctionsExt};

impl<S: EthereumLikeTypes> Interpreter<'_, S> {
    pub fn wrapped_add(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::VERYLOW, ADD_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        core::ops::AddAssign::add_assign(op2, op1);
        Ok(())
    }

    pub fn wrapping_mul(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::LOW, MUL_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        op2.wrapping_mul_assign(op1);
        Ok(())
    }

    pub fn wrapping_sub(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::VERYLOW, SUB_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        // Compute op1 - op2 and store in op2
        op2.overflowing_sub_assign_reversed(op1);
        Ok(())
    }

    pub fn div(&mut self, system: &mut System<S>) -> InstructionResult
    where
        S::IO: IOSubsystemExt,
    {
        self.gas
            .spend_gas_and_native(gas_constants::LOW, DIV_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_mut_and_peek()?;
        if !op2.is_zero() {
            S::SystemFunctionsExt::u256_div_rem(op1, op2, system.io.oracle());
            Clone::clone_from(op2, &*op1);
        }
        Ok(())
    }

    pub fn sdiv(&mut self, system: &mut System<S>) -> InstructionResult
    where
        S::IO: IOSubsystemExt,
    {
        self.gas
            .spend_gas_and_native(gas_constants::LOW, SDIV_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_mut_and_peek()?;
        i256_div(op1, op2, |a, b| {
            S::SystemFunctionsExt::u256_div_rem(a, b, system.io.oracle())
        });
        Ok(())
    }

    pub fn rem(&mut self, system: &mut System<S>) -> InstructionResult
    where
        S::IO: IOSubsystemExt,
    {
        self.gas
            .spend_gas_and_native(gas_constants::LOW, MOD_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_mut_and_peek()?;
        if !op2.is_zero() {
            S::SystemFunctionsExt::u256_div_rem(op1, op2, system.io.oracle());
        } else {
            U256::write_zero(op2);
        }
        Ok(())
    }

    pub fn smod(&mut self, system: &mut System<S>) -> InstructionResult
    where
        S::IO: IOSubsystemExt,
    {
        self.gas
            .spend_gas_and_native(gas_constants::LOW, SMOD_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_mut_and_peek()?;
        if !op2.is_zero() {
            i256_mod(op1, op2, |a, b| {
                S::SystemFunctionsExt::u256_div_rem(a, b, system.io.oracle())
            })
        };
        Ok(())
    }

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
        if *op1 >= *op3 {
            let mut m = op3.clone();
            S::SystemFunctionsExt::u256_div_rem(op1, &mut m, system.io.oracle());
            *op1 = m;
        }
        if *op2 >= *op3 {
            let mut m = op3.clone();
            S::SystemFunctionsExt::u256_div_rem(op2, &mut m, system.io.oracle());
            *op2 = m;
        }
        let carry = op1.overflowing_add_assign(op2);
        if carry || *op1 >= *op3 {
            op1.overflowing_sub_assign(op3);
        }
        core::mem::swap(op1, op3);
        Ok(())
    }

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
        // Fast path: modulus = 1 → result is always 0. EVM spec for MULMOD.
        if op3.is_one() {
            U256::write_zero(op3);
            return Ok(());
        }
        // Compute a * b -> (product_lo, product_hi)
        let mut product_lo = U256::from_limbs(*op1.as_limbs());
        let mut product_hi = U256::from_limbs(*op1.as_limbs());
        product_lo.widening_mul_assign_into(&mut product_hi, op2);
        // Fast path: modulus = 2^256 - 1. Skip the oracle-backed wide div_rem.
        if op3.as_limbs() == &[u64::MAX; 4] {
            reduce_mod_max(&mut product_lo, &product_hi, op3);
            return Ok(());
        }
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

    pub fn eval_exp(&mut self) -> InstructionResult {
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        if let Some((gas_cost, native_cost)) = exp_cost(&op2) {
            self.gas.spend_gas_and_native(gas_cost, native_cost)?;
        } else {
            return Err(ExitCode::EvmError(EvmError::OutOfGas));
        }
        let exp = op2.clone();
        U256::pow(op1, &exp, op2);
        Ok(())
    }

    pub fn sign_extend(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::LOW, SIGNEXTEND_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        if let Some(shift) = op1.try_to_usize_capped::<32>() {
            let bit_index = 8 * shift + 7;
            let bit = op2.bit(bit_index);
            let mut mask = U256::one();
            core::ops::ShlAssign::shl_assign(&mut mask, bit_index as u32);
            let one = U256::one();
            core::ops::SubAssign::sub_assign(&mut mask, &one);
            if bit {
                mask.not_mut();
                core::ops::BitOrAssign::bitor_assign(op2, &mask);
            } else {
                core::ops::BitAndAssign::bitand_assign(op2, &mask);
            }
        }

        Ok(())
    }
}

/// Reduce a 512-bit product modulo `2^256 - 1`, given the widening product as
/// `(product_lo, product_hi)`. Writes the 256-bit result into `out`.
///
/// Math: since `2^256 ≡ 1 (mod 2^256 - 1)`, we have
///   `lo + hi * 2^256 ≡ lo + hi (mod 2^256 - 1)`.
/// A single carry-aware add reduces, with a final normalization step that maps
/// `2^256 - 1 ≡ 0`.
#[inline]
fn reduce_mod_max(product_lo: &mut U256, product_hi: &U256, out: &mut U256) {
    let carry = product_lo.overflowing_add_assign(product_hi);
    if carry {
        // The wrapped 2^256 ≡ 1 (mod 2^256 - 1), so account for it by adding 1.
        // This cannot itself overflow: `lo + hi <= 2*(2^256 - 1) = 2^257 - 2`, so
        // when carry was set the wrapped value is `<= 2^256 - 2`.
        product_lo.overflowing_add_assign(&U256::ONE);
    }
    if product_lo.as_limbs() == &[u64::MAX; 4] {
        U256::write_zero(out);
    } else {
        core::mem::swap(out, product_lo);
    }
}

pub fn exp_cost(power: &U256) -> Option<(u64, u64)> {
    if power.is_zero() {
        Some((gas_constants::EXP, EXP_BASE_NATIVE_COST))
    } else {
        let gas_byte: u64 = 50;
        // 50 * 33 never overflows u64
        let num_bytes = log2floor(power) / 8 + 1;
        let gas_cost = gas_byte
            .checked_mul(num_bytes)?
            .checked_add(gas_constants::EXP)?;
        let native_cost =
            EXP_BASE_NATIVE_COST.checked_add(EXP_PER_BYTE_NATIVE_COST.checked_mul(num_bytes)?)?;
        Some((gas_cost, native_cost))
    }
}

#[cfg(test)]
mod tests {
    use super::reduce_mod_max;
    use ruint::aliases::U256 as HostU256;
    use u256::U256;

    fn run_reduce_mod_max(a: HostU256, b: HostU256) -> HostU256 {
        let a_u: U256 = a.into();
        let b_u: U256 = b.into();
        let mut lo = U256::from_limbs(*a_u.as_limbs());
        let mut hi = U256::from_limbs(*a_u.as_limbs());
        lo.widening_mul_assign_into(&mut hi, &b_u);
        let mut out = U256::from_limbs([0u64; 4]);
        reduce_mod_max(&mut lo, &hi, &mut out);
        out.into()
    }

    fn assert_matches_reference(a: HostU256, b: HostU256) {
        let actual = run_reduce_mod_max(a, b);
        let expected = a.mul_mod(b, HostU256::MAX);
        assert_eq!(actual, expected, "(a, b) = ({:#x}, {:#x})", a, b);
    }

    #[test]
    fn reduce_mod_max_matches_ruint_reference() {
        let max = HostU256::MAX;
        let zero = HostU256::ZERO;
        let one = HostU256::from(1u64);
        let two = HostU256::from(2u64);
        let half_max = HostU256::from_limbs([u64::MAX, u64::MAX, 0, 0]);
        let high_limb = HostU256::from_limbs([0, 0, 0, 0xdead_beef_cafe_babe]);
        let mixed = HostU256::from_limbs([
            0x0123_4567_89ab_cdef,
            0xfedc_ba98_7654_3210,
            0x0011_2233_4455_6677,
            0x7f00_0000_0000_0000,
        ]);

        for a in [zero, one, two, half_max, high_limb, mixed, max] {
            for b in [zero, one, two, half_max, high_limb, mixed, max] {
                assert_matches_reference(a, b);
            }
        }
    }
}
