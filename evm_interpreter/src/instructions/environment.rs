use super::*;
use native_resource_constants::*;

impl<S: EthereumLikeTypes> Interpreter<S> {
    pub fn chainid(&mut self, system: &mut System<S>) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::BASE, CHAINID_NATIVE_COST)?;
        let result = U256::from(system.get_chain_id());
        self.stack.push_1(&result)?;
        Ok(())
    }

    pub fn coinbase(&mut self, system: &mut System<S>) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::BASE, COINBASE_NATIVE_COST)?;
        self.stack.push_1(&b160_to_u256(system.get_coinbase()))?;
        Ok(())
    }

    pub fn timestamp(&mut self, system: &mut System<S>) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::BASE, TIMESTAMP_NATIVE_COST)?;
        let result = U256::from(system.get_timestamp());
        self.stack.push_1(&result)?;
        Ok(())
    }

    pub fn number(&mut self, system: &mut System<S>) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::BASE, NUMBER_NATIVE_COST)?;
        let result = U256::from(system.get_block_number());
        self.stack.push_1(&result)?;
        Ok(())
    }

    pub fn difficulty(&mut self) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::BASE, DIFFICULTY_NATIVE_COST)?;
        self.stack.push_1(&U256::zero())?;
        Ok(())
    }

    pub fn gaslimit(&mut self, system: &mut System<S>) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::BASE, GAS_NATIVE_COST)?;
        let result = U256::from(system.get_gas_limit());
        self.stack.push_1(&result)?;
        Ok(())
    }

    pub fn gasprice(&mut self, system: &mut System<S>) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::BASE, GASPRICE_NATIVE_COST)?;
        self.stack.push_1(&system.get_gas_price())?;
        Ok(())
    }

    pub fn basefee(&mut self, system: &mut System<S>) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::BASE, BASEFEE_NATIVE_COST)?;
        self.stack.push_1(&system.get_eip1559_basefee())?;
        Ok(())
    }

    pub fn origin(&mut self, system: &mut System<S>) -> InstructionResult {
        #[cfg(feature = "eip-7645")]
        {
            self.spend_gas_and_native(0, ORIGIN_NATIVE_COST)?;
            return self.caller();
        }

        #[cfg(not(feature = "eip-7645"))]
        {
            self.spend_gas_and_native(gas_constants::BASE, ORIGIN_NATIVE_COST)?;
            self.stack.push_1(&b160_to_u256(system.get_tx_origin()))?;
            Ok(())
        }
    }

    pub fn blockhash(&mut self, system: &mut System<S>) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::BLOCKHASH, BLOCKHASH_NATIVE_COST)?;
        let block_number = self.stack.pop_1()?;
        let block_number = u256_to_u64_saturated(&block_number);
        self.stack
            .push_unchecked(&system.get_blockhash(block_number));
        Ok(())
    }

    #[cfg(feature = "mock-eip-4844")]
    // Mocked for tests
    pub fn blobhash(&mut self, _system: &mut System<S>) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::VERYLOW, 40)?;
        self.stack.stack_reduce_one()?;
        self.stack.push_unchecked(&U256::zero());

        Ok(())
    }

    #[cfg(feature = "mock-eip-4844")]
    // Mocked for tests
    pub fn blobbasefee(&mut self, _system: &mut System<S>) -> InstructionResult {
        self.spend_gas_and_native(gas_constants::BASE, 40)?;
        self.stack.push_1(&U256::one())
    }
}
