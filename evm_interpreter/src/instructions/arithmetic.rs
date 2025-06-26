use super::*;
use crate::u256::*;
use native_resource_constants::*;

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
        let _ = op2.overflowing_sub_assign_reversed(op1);
        Ok(())
    }

    pub fn div(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::LOW, DIV_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_mut_and_peek()?;
        if op2.is_zero() == false {
            // we will mangle op1, but we do not care
            U256::div_rem(op1, op2);
            Clone::clone_from(op2, &*op1);
        }
        Ok(())
    }

    pub fn sdiv(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::LOW, SDIV_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_mut_and_peek()?;
        i256_div(op1, op2);
        Ok(())
    }

    pub fn rem(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::LOW, MOD_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_mut_and_peek()?;
        if op2.is_zero() == false {
            // we will mangle op1, but we do not care
            U256::div_rem(op1, op2);
        }
        Ok(())
    }

    pub fn smod(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::LOW, SMOD_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_mut_and_peek()?;
        if op2.is_zero() == false {
            i256_mod(op1, op2)
        };
        Ok(())
    }

    pub fn addmod(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::MID, ADDMOD_NATIVE_COST)?;
        let ((op1, op2), op3) = self.stack.pop_2_mut_and_peek()?;
        U256::add_mod(op1, op2, op3);
        Ok(())
    }

    pub fn mulmod(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::MID, MULMOD_NATIVE_COST)?;
        let ((op1, op2), op3) = self.stack.pop_2_mut_and_peek()?;
        U256::mul_mod(op1, op2, op3);
        Ok(())
    }

    pub fn eval_exp(&mut self) -> InstructionResult {
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        if let Some((gas_cost, native_cost)) = exp_cost(&op2) {
            self.gas.spend_gas_and_native(gas_cost, native_cost)?;
        } else {
            return Err(ExitCode::OutOfGas);
        }

        let exp: U256 = op2.clone();
        U256::pow(op1, &exp, op2);

        Ok(())
    }

    pub fn sign_extend(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::LOW, SIGNEXTEND_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        if let Some(shift) = u256_try_to_usize_capped::<32>(op1) {
            let bit_index = 8 * shift + 7;
            let bit = op2.bit(bit_index);
            let mut mask = U256::one();
            core::ops::ShlAssign::shl_assign(&mut mask, bit_index as u32);
            core::ops::SubAssign::sub_assign(&mut mask, &U256::one());
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
