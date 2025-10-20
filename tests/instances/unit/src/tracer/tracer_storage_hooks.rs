#![cfg(test)]

//!
//! Basic test for the tracer storage read/write hooks.

use rig::alloy::consensus::TxEip2930;
use rig::alloy::primitives::{address, TxKind, U256};
use rig::forward_system::system::system::ForwardRunningSystem;
use rig::ruint::aliases::B160;
use rig::zk_ee::system::tracer::evm_tracer::NopEvmTracer;
use rig::zk_ee::{
    execution_environment_type::ExecutionEnvironmentType,
    system::{
        tracer::{evm_tracer::EvmTracer, Tracer},
        CallResult, ExecutionEnvironmentLaunchParams, SystemTypes,
    },
    utils::Bytes32,
};
use rig::Chain;

/// A struct to track tracer calls for storage operations
#[derive(Debug, Clone, Default)]
pub struct StorageTracerCalls {
    pub storage_reads: Vec<(bool, B160, Bytes32, Bytes32)>, // (is_transient, address, key, value)
    pub storage_writes: Vec<(bool, B160, Bytes32, Bytes32)>, // (is_transient, address, key, value)
}

/// Custom tracer that captures storage read and write calls
pub struct StorageOperationTracer {
    calls: StorageTracerCalls,
    evm_tracer: NopEvmTracer,
}

impl StorageOperationTracer {
    pub fn new() -> Self {
        Self {
            calls: Default::default(),
            evm_tracer: Default::default(),
        }
    }
}

impl Tracer<ForwardRunningSystem> for StorageOperationTracer {
    fn evm_tracer(&mut self) -> &mut impl EvmTracer<ForwardRunningSystem> {
        &mut self.evm_tracer
    }

    fn on_new_execution_frame(
        &mut self,
        _request: &ExecutionEnvironmentLaunchParams<ForwardRunningSystem>,
    ) {
    }

    fn after_execution_frame_completed(
        &mut self,
        _result: Option<(
            &<ForwardRunningSystem as SystemTypes>::Resources,
            &CallResult<ForwardRunningSystem>,
        )>,
    ) {
    }

    fn on_storage_read(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        is_transient: bool,
        address: B160,
        key: Bytes32,
        value: Bytes32,
    ) {
        self.calls.storage_reads.push((
            is_transient,
            address,
            Bytes32::from_array(key.as_u8_array()),
            Bytes32::from_array(value.as_u8_array()),
        ));
    }

    fn on_storage_write(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        is_transient: bool,
        address: B160,
        key: Bytes32,
        value: Bytes32,
    ) {
        self.calls.storage_writes.push((
            is_transient,
            address,
            Bytes32::from_array(key.as_u8_array()),
            Bytes32::from_array(value.as_u8_array()),
        ));
    }

    fn on_bytecode_change(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        _address: B160,
        _new_raw_bytecode: Option<&[u8]>,
        _new_internal_bytecode_hash: Bytes32,
        _new_observable_bytecode_length: u32,
    ) {
    }

    fn on_event(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        _address: &B160,
        _topics: &[Bytes32],
        _data: &[u8],
    ) {
    }

    fn begin_tx(&mut self, _calldata: &[u8]) {}

    fn finish_tx(&mut self) {}
}

#[test]
fn test_storage_hooks() {
    let mut chain = Chain::empty(None);
    let wallet = chain.random_signer();

    let contract_address = address!("1000000000000000000000000000000000000001");

    // Contract bytecode that performs storage operations:
    // PUSH1 42, PUSH1 0, SSTORE  (store 42 in slot 0 - should call on_storage_write)
    // PUSH1 0, SLOAD, POP        (load from slot 0 - should call on_storage_read)
    // PUSH1 42, PUSH1 0, TSTORE  (tstore 42 in slot 0 - should call on_storage_write)
    // PUSH1 0, TLOAD, POP        (tload from slot 0 - should call on_storage_read)
    let test_contract_bytecode = hex::decode("602a60005560005450602a60005D60005C50").unwrap();

    chain.set_balance(
        B160::from_be_bytes(wallet.address().into_array()),
        U256::from(1_000_000_000_000_000_u64),
    );
    chain.set_evm_bytecode(
        B160::from_be_bytes(contract_address.into_array()),
        &test_contract_bytecode,
    );

    // Create transaction to call the contract
    let encoded_tx = {
        let tx = TxEip2930 {
            chain_id: 37u64,
            nonce: 0,
            gas_price: 1000,
            gas_limit: 100_000,
            to: TxKind::Call(contract_address),
            value: Default::default(),
            input: Default::default(),
            access_list: Default::default(),
        };
        rig::utils::sign_and_encode_alloy_tx(tx, &wallet)
    };

    let mut tracer = StorageOperationTracer::new();

    let result = chain.run_block_with_extra_stats(vec![encoded_tx], None, None, &mut tracer);

    assert!(result.is_ok(), "Block execution should succeed");
    let (block_output, _, _) = result.unwrap();
    assert!(
        block_output.tx_results[0].is_ok(),
        "Transaction should succeed with correct tracer calls. Result: {:?}",
        block_output.tx_results[0]
    );

    assert_eq!(tracer.calls.storage_reads.len(), 2);
    assert_eq!(
        tracer.calls.storage_reads[0],
        (
            false,
            B160::from_be_bytes(contract_address.into_array()),
            Bytes32::zero(),
            Bytes32::from_hex("000000000000000000000000000000000000000000000000000000000000002a")
        )
    );
    assert_eq!(
        tracer.calls.storage_reads[1],
        (
            true,
            B160::from_be_bytes(contract_address.into_array()),
            Bytes32::zero(),
            Bytes32::from_hex("000000000000000000000000000000000000000000000000000000000000002a")
        )
    );

    assert_eq!(tracer.calls.storage_writes.len(), 2);
    assert_eq!(
        tracer.calls.storage_writes[0],
        (
            false,
            B160::from_be_bytes(contract_address.into_array()),
            Bytes32::zero(),
            Bytes32::from_hex("000000000000000000000000000000000000000000000000000000000000002a")
        )
    );
    assert_eq!(
        tracer.calls.storage_writes[1],
        (
            true,
            B160::from_be_bytes(contract_address.into_array()),
            Bytes32::zero(),
            Bytes32::from_hex("000000000000000000000000000000000000000000000000000000000000002a")
        )
    );
}
