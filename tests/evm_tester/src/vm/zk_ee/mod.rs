use crate::test::case::transaction::encode_transaction;
use crate::utils::*;
use alloy::primitives::*;
use std::cmp::min;
use std::str::FromStr;
use zk_ee::utils::u256_to_u64_saturated;
use zk_ee::utils::Bytes32;
use zksync_os_basic_bootloader::bootloader::constants::MAX_BLOCK_GAS_LIMIT;
use zksync_os_basic_bootloader::bootloader::errors::BootloaderSubsystemError;
use zksync_os_rig::zksync_os_api::helpers;
use zksync_os_rig::zksync_os_interface::error::InvalidTransaction;
use zksync_os_rig::zksync_os_interface::types::{BlockOutput, TxOutput};
use zksync_os_rig::BlockContext;
use zksync_os_rig::Chain;

use crate::test::case::transaction::Transaction;

// mod transaction;

#[derive(Clone, Default)]
pub struct ZKsyncOSEVMContext {
    pub chain_id: u64,
    pub coinbase: Address,
    pub block_number: u128,
    pub block_timestamp: u128,
    pub block_gas_limit: U256,
    pub block_difficulty: B256,
    pub base_fee: U256,
    pub tx_origin: Address,
    pub mix_hash: U256,
}

///
/// The VM execution result.
///
#[derive(Debug, Clone, Default)]
pub struct ZKsyncOSTxExecutionResult {
    /// The VM snapshot execution result.
    pub return_data: Vec<u8>,
    pub exception: bool,
    /// The number of gas used.
    pub gas: U256,
    pub address_deployed: Option<Address>,
}

///
/// The ZKsync OS interface.
///
pub struct ZKsyncOS {
    pub chain: Chain,
}

impl ZKsyncOS {
    pub fn new() -> Self {
        let chain = Chain::empty(Some(1));
        Self { chain }
    }

    pub fn execute_transactions(
        &mut self,
        transactions: Vec<Transaction>,
        system_context: ZKsyncOSEVMContext,
    ) -> anyhow::Result<Vec<ZKsyncOSTxExecutionResult>, String> {
        let encoded_txs: Vec<Vec<u8>> = transactions
            .iter()
            .map(|transaction| encode_transaction(transaction, &system_context))
            .collect();

        let block_gas_limit: u64 = system_context
            .block_gas_limit
            .try_into()
            .expect("Block gas limit overflowed u64");
        // Override block gas limit
        let gas_limit = min(block_gas_limit, MAX_BLOCK_GAS_LIMIT);

        if system_context.block_number > 0 {
            self.chain
                .set_last_block_number(system_context.block_number as u64 - 1)
        }

        let context = BlockContext {
            eip1559_basefee: ruint::Uint::from_str(&system_context.base_fee.to_string())
                .expect("Invalid basefee"),
            native_price: ruint::aliases::U256::from(1),
            gas_per_pubdata: Default::default(),
            timestamp: system_context.block_timestamp as u64,
            gas_limit,
            pubdata_limit: u64::MAX,
            coinbase: ruint::Bits::try_from_be_slice(system_context.coinbase.as_slice())
                .expect("Invalid coinbase"),
            mix_hash: system_context.mix_hash,
        };

        let result = self
            .chain
            .run_block_no_panic(encoded_txs, Some(context), None, true);

        self.get_block_execution_result(result)
    }

    fn get_block_execution_result(
        &mut self,
        result: Result<BlockOutput, BootloaderSubsystemError>,
    ) -> anyhow::Result<Vec<ZKsyncOSTxExecutionResult>, String> {
        match result {
            Ok(result) => {
                let mut results = vec![];
                for tx_result in result.tx_results {
                    let r = Self::get_transaction_execution_result(tx_result)?;
                    results.push(r)
                }
                Ok(results)
            }
            Err(err) => Err(format!("{err:?}")),
        }
    }

    fn get_transaction_execution_result(
        tx_result: Result<TxOutput, InvalidTransaction>,
    ) -> anyhow::Result<ZKsyncOSTxExecutionResult, String> {
        match tx_result {
            Ok(tx_output) => {
                let mut execution_result = ZKsyncOSTxExecutionResult::default();

                execution_result.gas = U256::from(tx_output.gas_used);
                // TODO events

                match &tx_output.execution_result {
                    zksync_os_rig::zksync_os_interface::types::ExecutionResult::Success(
                        execution_output,
                    ) => match execution_output {
                        zksync_os_rig::zksync_os_interface::types::ExecutionOutput::Call(data) => {
                            execution_result.return_data = data.clone();
                        }
                        zksync_os_rig::zksync_os_interface::types::ExecutionOutput::Create(
                            data,
                            address,
                        ) => {
                            execution_result.return_data = data.clone();
                            execution_result.address_deployed = Some(*address);
                        }
                    },
                    zksync_os_rig::zksync_os_interface::types::ExecutionResult::Revert(vec) => {
                        execution_result.exception = true;
                        execution_result.return_data = vec.clone();
                    }
                }
                Ok(execution_result)
            }
            Err(tx_err) => Err(format!("{tx_err:?}")),
        }
    }

    ///
    /// Returns the balance of the specified address.
    ///
    pub fn get_balance(&mut self, address: Address) -> U256 {
        let properties = self.chain.get_account_properties(&address_to_b160(address));
        helpers::get_balance(&properties)
    }

    ///
    /// Changes the balance of the specified address.
    ///
    pub fn set_balance(&mut self, address: Address, value: U256) {
        self.chain.set_balance(address_to_b160(address), value);
    }

    ///
    /// Returns the nonce of the specified address.
    ///
    pub fn get_nonce(&mut self, address: Address) -> U256 {
        let properties = self.chain.get_account_properties(&address_to_b160(address));
        U256::from(helpers::get_nonce(&properties))
    }

    ///
    /// Changes the nonce of the specified address.
    ///
    pub fn set_nonce(&mut self, address: Address, nonce: U256) {
        let nonce = u256_to_u64_saturated(&nonce);
        self.chain
            .set_account_properties(address_to_b160(address), None, Some(nonce), None)
    }

    pub fn get_storage_slot(&mut self, address: Address, key: U256) -> Option<B256> {
        self.chain
            .get_storage_slot(address_to_b160(address), key)
            .map(|v| bytes32_to_b256(v.clone()))
    }

    pub fn set_storage_slot(&mut self, address: Address, key: U256, value: B256) {
        let address = address_to_b160(address);
        let value = ruint::aliases::B256::from_be_bytes(value.0);
        self.chain.set_storage_slot(address, key, value);
    }

    pub fn set_predeployed_evm_contract(&mut self, address: Address, bytecode: Bytes, nonce: U256) {
        self.chain.set_account_properties(
            address_to_b160(address),
            None,
            Some(u256_to_u64_saturated(&nonce)),
            Some(bytecode.0.to_vec()),
        )
    }

    pub fn get_code(&mut self, address: Address) -> Option<Vec<u8>> {
        let properties = self.chain.get_account_properties(&address_to_b160(address));

        if properties.bytecode_hash == Bytes32::zero() {
            None
        } else {
            Some(helpers::get_code(
                &mut self.chain.preimage_source,
                &properties,
            ))
        }
    }
}

pub fn b256_to_bytes32(input: B256) -> Bytes32 {
    Bytes32::from_array(input.0)
}

pub fn u256_to_bytes32(input: U256) -> Bytes32 {
    Bytes32::from_array(input.to_be_bytes())
}

pub fn bytes32_to_b256(input: Bytes32) -> B256 {
    B256::from_slice(&input.as_u8_array())
}
