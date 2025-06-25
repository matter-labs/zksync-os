use super::*;
use native_resource_constants::*;

impl<S: EthereumLikeTypes> Interpreter<'_, S> {
    pub fn lt(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::VERYLOW, LT_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        *op2 = if op1.lt(op2) {
            U256::from(1)
        } else {
            U256::ZERO
        };
        Ok(())
    }

    pub fn gt(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::VERYLOW, GT_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        *op2 = if op1.gt(op2) {
            U256::from(1)
        } else {
            U256::ZERO
        };
        Ok(())
    }

    pub fn slt(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::VERYLOW, SLT_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        *op2 = if i256_cmp(op1, op2) == core::cmp::Ordering::Less {
            U256::from(1)
        } else {
            U256::ZERO
        };
        Ok(())
    }

    pub fn sgt(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::VERYLOW, SGT_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        *op2 = if i256_cmp(op1, op2) == core::cmp::Ordering::Greater {
            U256::from(1)
        } else {
            U256::ZERO
        };
        Ok(())
    }

    pub fn eq(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::VERYLOW, EQ_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        *op2 = if op1.eq(op2) {
            U256::from(1)
        } else {
            U256::ZERO
        };
        Ok(())
    }

    pub fn iszero(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::VERYLOW, ISZERO_NATIVE_COST)?;
        let stack_top = self.stack.top_mut()?;
        *stack_top = if stack_top.is_zero() {
            U256::from(1)
        } else {
            U256::ZERO
        };
        Ok(())
    }
    pub fn bitand(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::VERYLOW, AND_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        core::ops::BitAndAssign::bitand_assign(op2, op1);
        Ok(())
    }
    pub fn bitor(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::VERYLOW, OR_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        core::ops::BitOrAssign::bitor_assign(op2, op1);
        Ok(())
    }
    pub fn bitxor(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::VERYLOW, XOR_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        core::ops::BitXorAssign::bitxor_assign(op2, op1);
        Ok(())
    }

    pub fn not(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::VERYLOW, NOT_NATIVE_COST)?;
        let op1 = self.stack.top_mut()?;
        *op1 = !*op1;
        Ok(())
    }

    pub fn byte(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::VERYLOW, BYTE_NATIVE_COST)?;
        let (offset, src) = self.stack.pop_1_and_peek_mut()?;

        if let Some(offset) = u256_try_to_usize_capped::<32>(offset) {
            let ret = src.byte(31 - offset);
            *src = U256::from(ret as u64);
        } else {
            *src = U256::ZERO;
        }

        Ok(())
    }

    pub fn shl(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::VERYLOW, SHL_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        core::ops::ShlAssign::shl_assign(op2, u256_to_usize_saturated(op1) as u32);
        Ok(())
    }

    pub fn shr(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::VERYLOW, SHR_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        core::ops::ShrAssign::shr_assign(op2, u256_to_usize_saturated(op1) as u32);
        Ok(())
    }

    pub fn sar(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::VERYLOW, SAR_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        let sign_bit = op2.bit(255);
        if let Some(shift) = u256_try_to_usize_capped::<256>(op1) {
            if sign_bit == false {
                core::ops::ShrAssign::shr_assign(op2, shift as u32);
            } else {
                // perform unsigned shift, then OR with mask
                core::ops::ShrAssign::shr_assign(op2, shift as u32);
                let (words, bits) = (shift / 64, shift % 64);
                unsafe {
                    for i in 0..words {
                        op2.as_limbs_mut()[3 - i] = u64::MAX;
                    }
                    if bits != 0 {
                        op2.as_limbs_mut()[3 - words] |= u64::MAX << (64 - bits);
                    }
                }
            }
        } else {
            // shift overflowed
            if sign_bit == false {
                // value is 0 or >=1, pushing 0
                *op2 = U256::ZERO;
            } else {
                // value is <0, pushing -1
                unsafe {
                    op2.as_limbs_mut().iter_mut().for_each(|el| *el = u64::MAX);
                }
            }
        }
        Ok(())
    }
}
