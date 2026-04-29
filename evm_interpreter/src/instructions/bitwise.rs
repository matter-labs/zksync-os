use super::*;
use native_resource_constants::*;

#[inline(always)]
fn arithmetic_shr_in_place(value: &mut U256, shift: usize) {
    let shift = shift.min(256);
    let is_negative = value.bit(255);

    if shift == 256 {
        if is_negative {
            let mut all_ones = U256::zero();
            all_ones.not_mut();
            *value = all_ones;
        } else {
            U256::write_zero(value);
        }
        return;
    }

    if shift == 0 {
        return;
    }

    *value >>= shift as u32;
    if is_negative {
        let mut mask = U256::zero();
        mask.not_mut();
        mask <<= (256 - shift) as u32;
        core::ops::BitOrAssign::bitor_assign(value, &mask);
    }
}

impl<S: EthereumLikeTypes> Interpreter<'_, S> {
    pub fn lt(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::VERYLOW, LT_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        if op1.lt(op2) {
            U256::write_one(op2);
        } else {
            U256::write_zero(op2);
        }
        Ok(())
    }

    pub fn gt(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::VERYLOW, GT_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        if op1.gt(op2) {
            U256::write_one(op2);
        } else {
            U256::write_zero(op2);
        }
        Ok(())
    }

    pub fn slt(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::VERYLOW, SLT_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        if i256_cmp(op1, op2) == core::cmp::Ordering::Less {
            U256::write_one(op2);
        } else {
            U256::write_zero(op2);
        }
        Ok(())
    }

    pub fn sgt(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::VERYLOW, SGT_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        if i256_cmp(op1, op2) == core::cmp::Ordering::Greater {
            U256::write_one(op2);
        } else {
            U256::write_zero(op2);
        }
        Ok(())
    }

    pub fn eq(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::VERYLOW, EQ_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        if op1.eq(op2) {
            U256::write_one(op2);
        } else {
            U256::write_zero(op2);
        }
        Ok(())
    }

    pub fn iszero(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::VERYLOW, ISZERO_NATIVE_COST)?;
        let stack_top = self.stack.top_mut()?;
        if stack_top.is_zero() {
            U256::write_one(stack_top);
        } else {
            U256::write_zero(stack_top);
        }
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
        op1.not_mut();
        Ok(())
    }

    pub fn byte(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::VERYLOW, BYTE_NATIVE_COST)?;
        let (offset, src) = self.stack.pop_1_and_peek_mut()?;

        if let Some(offset) = custom_u256_try_to_usize_capped::<32>(offset) {
            let ret = src.byte(31 - offset);
            U256::write_zero(src);
            src.as_limbs_mut()[0] = ret as u64;
        } else {
            U256::write_zero(src);
        }

        Ok(())
    }

    pub fn shl(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::VERYLOW, SHL_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        match custom_u256_try_to_usize(op1) {
            None => U256::write_zero(op2),
            Some(shift) => {
                if shift >= 256 {
                    U256::write_zero(op2);
                } else {
                    *op2 <<= shift as u32;
                }
            }
        }
        Ok(())
    }

    pub fn shr(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::VERYLOW, SHR_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        match custom_u256_try_to_usize(op1) {
            None => U256::write_zero(op2),
            Some(shift) => {
                if shift >= 256 {
                    U256::write_zero(op2);
                } else {
                    *op2 >>= shift as u32;
                }
            }
        }
        Ok(())
    }

    pub fn sar(&mut self) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::VERYLOW, SAR_NATIVE_COST)?;
        let (op1, op2) = self.stack.pop_1_and_peek_mut()?;
        let shift = custom_u256_to_usize_saturated(op1);
        arithmetic_shr_in_place(op2, shift);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::arithmetic_shr_in_place;
    use ruint::aliases::U256 as HostU256;
    use u256::U256;

    fn assert_sar_matches_host(input: HostU256, shift: usize) {
        let mut actual: U256 = input.into();
        arithmetic_shr_in_place(&mut actual, shift);
        let actual_host: HostU256 = actual.into();
        assert_eq!(actual_host, input.arithmetic_shr(shift));
    }

    #[test]
    fn sar_matches_host_reference_for_boundary_cases() {
        let positive = HostU256::from_limbs([
            0x0123_4567_89ab_cdef,
            0xfedc_ba98_7654_3210,
            0x0011_2233_4455_6677,
            0x7f00_0000_0000_0000,
        ]);
        let negative = HostU256::MAX - HostU256::from(41u64);
        let min_int = HostU256::from_limbs([0, 0, 0, 0x8000_0000_0000_0000]);
        let minus_one = HostU256::MAX;
        let zero = HostU256::ZERO;

        for value in [positive, negative, min_int, minus_one, zero] {
            for shift in [0usize, 1, 128, 255, 256, 257] {
                assert_sar_matches_host(value, shift);
            }
        }
    }
}
