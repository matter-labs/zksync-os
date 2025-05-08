use super::*;
use native_resource_constants::*;

impl<S: EthereumLikeTypes> Interpreter<'_, S> {
    pub fn pop(&mut self) -> InstructionResult {
<<<<<<< HEAD
        self.spend_gas_and_native(gas_constants::BASE, POP_NATIVE_COST)?;
        self.stack_reduce_one()
=======
        self.spend_gas(gas_constants::BASE)?;
        self.stack.stack_reduce_one()
>>>>>>> try for perf run
    }

    /// Introduce a new instruction which pushes the constant value 0 onto the stack
    pub fn push0(&mut self) -> InstructionResult {
        // EIP-3855: PUSH0 instruction
<<<<<<< HEAD
        self.spend_gas_and_native(gas_constants::BASE, PUSH0_NATIVE_COST)?;
        self.stack_push_one(U256::ZERO)
=======
        self.spend_gas(gas_constants::BASE)?;
        self.stack.push_1(&U256::ZERO)
>>>>>>> try for perf run
    }

    pub fn push<const N: usize>(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::VERYLOW, PUSH_NATIVE_COSTS[N])?;
        let start = self.instruction_pointer;

        let mut value = U256::ZERO;

        match self.bytecode.as_ref().get(start) {
            Some(src) => {
                // we read is as LE, and then bytereverse
                let to_copy =
                    core::cmp::min(N, self.bytecode.as_ref().len() - self.instruction_pointer);
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        src as *const u8,
                        value.as_limbs_mut().as_mut_ptr().cast::<u8>(),
                        to_copy,
                    );
                }
                crate::utils::bytereverse_u256(&mut value);
                value >>= (32 - N) * 8;
            }
            None => {
                // start is out of bounds of the bytecode buffer,
                // 0 will be pushed
            }
        }

        self.instruction_pointer += N;
        self.stack.push_1(&value)
    }

    pub fn dup<const N: usize>(&mut self) -> InstructionResult {
<<<<<<< HEAD
        self.spend_gas_and_native(gas_constants::VERYLOW, DUP_NATIVE_COST)?;
        self.stack_dup(N)
    }

    pub fn swap<const N: usize>(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::VERYLOW, SWAP_NATIVE_COST)?;
        self.stack_swap(N)
=======
        self.spend_gas(gas_constants::VERYLOW)?;
        self.stack.stack_dup(N)
    }

    pub fn swap<const N: usize>(&mut self) -> InstructionResult {
        self.spend_gas(gas_constants::VERYLOW)?;
        self.stack.stack_swap(N)
>>>>>>> try for perf run
    }
}
