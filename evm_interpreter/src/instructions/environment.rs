use super::*;
use native_resource_constants::*;

impl<S: EthereumLikeTypes> Interpreter<'_, S> {
    pub fn chainid(&mut self, system: &mut System<S>) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::BASE, CHAINID_NATIVE_COST)?;
        self.stack.push_u64(system.get_chain_id())?;
        Ok(())
    }

    pub fn coinbase(&mut self, system: &mut System<S>) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::BASE, COINBASE_NATIVE_COST)?;
        self.stack.push_b160(system.get_coinbase())?;
        Ok(())
    }

    pub fn timestamp(&mut self, system: &mut System<S>) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::BASE, TIMESTAMP_NATIVE_COST)?;
        self.stack.push_u64(system.get_timestamp())?;
        Ok(())
    }

    pub fn number(&mut self, system: &mut System<S>) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::BASE, NUMBER_NATIVE_COST)?;
        self.stack.push_u64(system.get_block_number())?;
        Ok(())
    }

    pub fn difficulty(&mut self, system: &mut System<S>) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::BASE, DIFFICULTY_NATIVE_COST)?;
        // Mix hash is the source of randomness, currently holding
        // the value of prevRandao.
        let value = U256::from_be_bytes(system.get_mix_hash()?.as_u8_array_ref());
        self.stack.push(&value)?;
        Ok(())
    }

    pub fn gaslimit(&mut self, system: &mut System<S>) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::BASE, GAS_NATIVE_COST)?;
        self.stack.push_u64(system.get_gas_limit())?;
        Ok(())
    }

    pub fn gasprice(&mut self, system: &mut System<S>) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::BASE, GASPRICE_NATIVE_COST)?;
        // gas_price returns ruint::aliases::U256, convert
        let price = U256::from(system.get_gas_price());
        self.stack.push(&price)?;
        Ok(())
    }

    pub fn basefee(&mut self, system: &mut System<S>) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::BASE, BASEFEE_NATIVE_COST)?;
        // basefee returns ruint::aliases::U256, convert
        let fee = U256::from(system.get_eip1559_basefee());
        self.stack.push(&fee)?;
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
            self.stack.push_b160(system.get_tx_origin())?;
            Ok(())
        }
    }

    pub fn blockhash(&mut self, system: &mut System<S>) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::BLOCKHASH, BLOCKHASH_NATIVE_COST)?;
        let block_number = self.stack.pop_1()?;
        let block_number = custom_u256_to_u64_saturated(block_number);
        let block_hash = U256::from_be_bytes(system.get_blockhash(block_number)?.as_u8_array_ref());
        self.stack.push(&block_hash)?;
        Ok(())
    }

    pub fn blobhash(&mut self, system: &mut System<S>) -> InstructionResult {
        self.gas
            .spend_gas_and_native(gas_constants::BLOBHASH, 100)?;
        let stack_top = self.stack.top_mut()?;
        if let Some(index) = custom_u256_try_to_usize(&*stack_top) {
            if let Some(blob_hash) = system.get_blob_hash(index) {
                *stack_top = U256::from_be_bytes(blob_hash.as_u8_array_ref());
            } else {
                U256::write_zero(stack_top);
            }
        } else {
            U256::write_zero(stack_top);
        }

        Ok(())
    }

    pub fn blobbasefee(&mut self, system: &mut System<S>) -> InstructionResult {
        self.gas.spend_gas_and_native(gas_constants::BASE, 100)?;
        // blob_base_fee returns ruint::aliases::U256, convert
        let fee = U256::from(system.get_blob_base_fee_per_gas());
        self.stack.push(&fee)
    }
}
