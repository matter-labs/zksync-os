use super::*;
use crate::u256::*;
use native_resource_constants::*;

impl<S: EthereumLikeTypes> Interpreter<'_, S> {
    pub fn wrapped_add(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::VERYLOW, ADD_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        core::ops::AddAssign::add_assign(op2, op1);
        // op2 += op1;
        // *op2 = op1.wrapping_add(*op2);
        Ok(())
    }

    pub fn wrapping_mul(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::LOW, MUL_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        op2.wrapping_mul_assign(op1);
        Ok(())
    }

    pub fn wrapping_sub(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::VERYLOW, SUB_NATIVE_COST)?;
        let ([op1], op2) = self.pop_values_and_peek::<1>()?;
        *op2 = op1.wrapping_sub(*op2);
        Ok(())
    }

    pub fn div(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::LOW, DIV_NATIVE_COST)?;
        self.spend_gas(gas_constants::LOW)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        if op2.is_zero() {
            U256::write_zero(op2);
        } else {
            let mut tmp = U256::zero();
            U256::div_assign_with_remainder(op2, &mut tmp, op1);
        }
        Ok(())
    }

    pub fn sdiv(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::LOW, SDIV_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        i256_div(op1, op2);
        Ok(())
    }

    pub fn rem(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::LOW, MOD_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        if op2.is_zero() {
            U256::write_zero(op2);
        } else {
            let mut tmp = U256::zero();
            U256::div_assign_with_remainder(&mut tmp, op2, op1);
        }
        Ok(())
    }

    pub fn smod(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::LOW, SMOD_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        if op2.is_zero() == false {
            i256_mod(op1, op2)
        };
        Ok(())
    }

    pub fn addmod(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::MID, ADDMOD_NATIVE_COST)?;
        let ((op1, op2), op3) = self.stack.pop_2_and_peek_mut()?;
        todo!();
        // *op3 = op1.add_mod(*op2, *op3);
        Ok(())
    }

    pub fn mulmod(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::MID, MULMOD_NATIVE_COST)?;
        let ((op1, op2), op3) = self.stack.pop_2_and_peek_mut()?;
        todo!();
        // *op3 = mul_mod(op1, op2, *op3);
        Ok(())
    }

    pub fn eval_exp(&mut self) -> InstructionResult {
        todo!();
        let (&op1, &op2) = self.stack.pop_2()?;
        if let Some(cost) = exp_cost(&op2) {
            self.spend_gas_and_native(gas_cost, native_cost)?;
        } else {
            return Err(ExitCode::OutOfGas);
        }
        // self.stack.push_unchecked(&op1.pow(op2));

        Ok(())
    }

    pub fn sign_extend(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::LOW, SIGNEXTEND_NATIVE_COST)?;
        self.spend_gas(gas_constants::LOW)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        todo!();
        // if op1 < &U256::from(32) {
        //     // `low_u32` works since op1 < 32
        //     let bit_index = (8 * op1.as_limbs()[0] + 7) as usize;
        //     let bit = op2.bit(bit_index);
        //     let mask = (U256::from(1) << bit_index) - U256::from(1);
        //     *op2 = if bit { *op2 | !mask } else { *op2 & mask };
        // }
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
