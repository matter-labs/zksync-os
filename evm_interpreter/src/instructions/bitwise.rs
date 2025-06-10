use super::*;
use native_resource_constants::*;

impl<S: EthereumLikeTypes> Interpreter<'_, S> {
    pub fn lt(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::VERYLOW, LT_NATIVE_COST)?;
        let ([op1], op2) = self.pop_values_and_peek::<1>()?;
        *op2 = if op1.lt(op2) {
            U256::from(1)
        } else {
            U256::ZERO
        };
        Ok(())
    }

    pub fn gt(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::VERYLOW, GT_NATIVE_COST)?;
        let ([op1], op2) = self.pop_values_and_peek::<1>()?;
        *op2 = if op1.gt(op2) {
            U256::from(1)
        } else {
            U256::ZERO
        };
        Ok(())
    }

    pub fn slt(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::VERYLOW, SLT_NATIVE_COST)?;
        let ([op1], op2) = self.pop_values_and_peek::<1>()?;
        *op2 = if i256_cmp(op1, *op2) == core::cmp::Ordering::Less {
            U256::from(1)
        } else {
            U256::ZERO
        };
        Ok(())
    }

    pub fn sgt(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::VERYLOW, SGT_NATIVE_COST)?;
        let ([op1], op2) = self.pop_values_and_peek::<1>()?;
        *op2 = if i256_cmp(op1, *op2) == core::cmp::Ordering::Greater {
            U256::from(1)
        } else {
            U256::ZERO
        };
        Ok(())
    }

    pub fn eq(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::VERYLOW, EQ_NATIVE_COST)?;
        let ([op1], op2) = self.pop_values_and_peek::<1>()?;
        *op2 = if op1.eq(op2) {
            U256::from(1)
        } else {
            U256::ZERO
        };
        Ok(())
    }

    pub fn iszero(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::VERYLOW, ISZERO_NATIVE_COST)?;
        let op1 = self.stack_peek()?;
        *op1 = if *op1 == U256::ZERO {
            U256::from(1)
        } else {
            U256::ZERO
        };
        Ok(())
    }
    pub fn bitand(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::VERYLOW, AND_NATIVE_COST)?;
        let ([op1], op2) = self.pop_values_and_peek::<1>()?;
        *op2 = op1.bitand(*op2);
        Ok(())
    }
    pub fn bitor(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::VERYLOW, OR_NATIVE_COST)?;
        let ([op1], op2) = self.pop_values_and_peek::<1>()?;
        *op2 = op1.bitor(*op2);
        Ok(())
    }
    pub fn bitxor(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::VERYLOW, XOR_NATIVE_COST)?;
        let ([op1], op2) = self.pop_values_and_peek::<1>()?;
        *op2 = op1.bitxor(*op2);
        Ok(())
    }

    pub fn not(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::VERYLOW, NOT_NATIVE_COST)?;
        let op1 = self.stack_peek()?;
        *op1 = !*op1;
        Ok(())
    }

    pub fn byte(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::VERYLOW, BYTE_NATIVE_COST)?;
        let ([offset], src) = self.pop_values_and_peek::<1>()?;

        let ret = if offset < U256::from(32) {
            src.byte(31 - u256_to_usize_saturated(&offset))
        } else {
            0
        };

        *src = U256::from(ret);
        Ok(())
    }

    pub fn shl(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::VERYLOW, SHL_NATIVE_COST)?;
        let ([op1], op2) = self.pop_values_and_peek::<1>()?;
        *op2 <<= u256_to_usize_saturated(&op1);
        Ok(())
    }

    pub fn shr(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::VERYLOW, SHR_NATIVE_COST)?;
        let ([op1], op2) = self.pop_values_and_peek::<1>()?;
        *op2 >>= u256_to_usize_saturated(&op1);
        Ok(())
    }

    pub fn sar(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::VERYLOW, SAR_NATIVE_COST)?;
        let ([op1], op2) = self.pop_values_and_peek::<1>()?;

        let value_sign = i256_sign::<true>(op2);

        *op2 = if *op2 == U256::ZERO || op1 >= U256::from(256) {
            match value_sign {
                // value is 0 or >=1, pushing 0
                Sign::Plus | Sign::Zero => U256::ZERO,
                // value is <0, pushing -1
                Sign::Minus => two_compl(U256::from(1)),
            }
        } else {
            let shift = usize::try_from(op1).unwrap();

            match value_sign {
                Sign::Plus | Sign::Zero => *op2 >> shift,
                Sign::Minus => {
                    let shifted = ((op2.overflowing_sub(U256::from(1)).0) >> shift)
                        .overflowing_add(U256::from(1))
                        .0;
                    two_compl(shifted)
                }
            }
        };
        Ok(())
    }
}
