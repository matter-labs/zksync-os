use super::*;
use native_resource_constants::*;

impl<S: EthereumLikeTypes> Interpreter<'_, S> {
    pub fn chainid(&mut self, system: &mut System<S>) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::BASE, CHAINID_NATIVE_COST)?;
        let result = U256::from(system.get_chain_id());
        self.stack.push(&result)?;
        Ok(())
    }

    pub fn coinbase(&mut self, system: &mut System<S>) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::BASE, COINBASE_NATIVE_COST)?;
        self.stack.push(&b160_to_u256(system.get_coinbase()))?;
        Ok(())
    }

    pub fn timestamp(&mut self, system: &mut System<S>) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::BASE, TIMESTAMP_NATIVE_COST)?;
        let result = U256::from(system.get_timestamp());
        self.stack.push(&result)?;
        Ok(())
    }

    pub fn number(&mut self, system: &mut System<S>) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::BASE, NUMBER_NATIVE_COST)?;
        let result = U256::from(system.get_block_number());
        self.stack.push(&result)?;
        Ok(())
    }

    pub fn difficulty(&mut self, system: &mut System<S>) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::BASE, DIFFICULTY_NATIVE_COST)?;
        // Mix hash is the source of randomness, currently holding
        // the value of prevRandao.
        self.stack.push(system.get_mix_hash())?;
        Ok(())
    }

    pub fn gaslimit(&mut self, system: &mut System<S>) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::BASE, GAS_NATIVE_COST)?;
        let result = U256::from(system.get_gas_limit());
        self.stack.push(&result)?;
        Ok(())
    }

    pub fn gasprice(&mut self, system: &mut System<S>) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::BASE, GASPRICE_NATIVE_COST)?;
        self.stack.push(&system.get_gas_price())?;
        Ok(())
    }

    pub fn basefee(&mut self, system: &mut System<S>) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::BASE, BASEFEE_NATIVE_COST)?;
        self.stack.push(&system.get_eip1559_basefee())?;
        Ok(())
    }

    pub fn origin(&mut self, system: &mut System<S>) -> InstructionResult {
        #[cfg(feature = "eip-7645")]
        {
            self.gas.spend_gas_and_native(0, ORIGIN_NATIVE_COST)?;
            return self.caller();
        }

        #[cfg(not(feature = "eip-7645"))]
        {
            self.gas
                .spend_gas_and_native(gas_constants::BASE, ORIGIN_NATIVE_COST)?;
            self.stack.push(&b160_to_u256(system.get_tx_origin()))?;
            Ok(())
        }
    }

    pub fn blockhash(&mut self, system: &mut System<S>) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::BLOCKHASH, BLOCKHASH_NATIVE_COST)?;
        let stack_top = self.stack.top_mut()?;
        let block_number = u256_to_u64_saturated(stack_top);
        *stack_top = system.get_blockhash(block_number);
        Ok(())
    }

    pub fn blobhash(&mut self, _system: &mut System<S>) -> InstructionResult {
        self.gas.spend_gas_and_native(gas_constants::VERYLOW, 40)?;
        let stack_top = self.stack.top_mut()?;
        U256::write_zero(stack_top);

        Ok(())
    }

    pub fn blobbasefee(&mut self, _system: &mut System<S>) -> InstructionResult {
        self.gas.spend_gas_and_native(gas_constants::BASE, 40)?;
        self.stack.push(&U256::one())
    }
}
