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
        // Compute a * b -> (product_lo, product_hi)
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
