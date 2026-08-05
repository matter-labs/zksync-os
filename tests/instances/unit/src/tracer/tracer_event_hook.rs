#![cfg(test)]

//!
//! Test for the tracer event hooks.
//!
//! This test verifies that LOG operations correctly call `on_event`
//! in the tracer with the proper parameters.

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

/// A struct to track tracer calls for event operations
#[derive(Debug, Clone, Default)]
pub struct EventTracerCalls {
    pub events: Vec<(u8, B160, Vec<Bytes32>, Vec<u8>)>, // (ee_type, address, topics, data)
}

/// Custom tracer that captures event calls
pub struct EventOperationTracer {
    calls: EventTracerCalls,
    evm_tracer: NopEvmTracer,
}

impl EventOperationTracer {
    pub fn new() -> Self {
        Self {
            calls: Default::default(),
            evm_tracer: Default::default(),
        }
    }
}

impl Tracer<ForwardRunningSystem> for EventOperationTracer {
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
        _is_transient: bool,
        _address: B160,
        _key: Bytes32,
        _value: Bytes32,
    ) {
    }

    fn on_storage_write(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        _is_transient: bool,
        _address: B160,
        _key: Bytes32,
        _value: Bytes32,
    ) {
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
        ee_type: ExecutionEnvironmentType,
        address: &B160,
        topics: &[Bytes32],
        data: &[u8],
    ) {
        self.calls
            .events
            .push((ee_type as u8, *address, topics.to_vec(), data.to_vec()));
    }

    fn begin_tx(&mut self, _calldata: &[u8]) {}

    fn finish_tx(&mut self) {}
}

#[test]
fn test_event_hook() {
    let mut chain = Chain::empty(None);
    let wallet = chain.random_signer();

    let contract_address = address!("1000000000000000000000000000000000000001");

    // Contract bytecode that emits events:
    // First, store some data in memory: PUSH1 0x42, PUSH1 0x00, MSTORE  (store 0x42 at memory position 0)
    // LOG0: PUSH1 0x20, PUSH1 0x00, LOG0  (emit event with 32 bytes of data from memory, no topics)
    // LOG1: PUSH1 0x20, PUSH1 0x00, PUSH2 0x1234, LOG1  (emit event with 32 bytes of data, 1 topic: 0x1234)
    // STOP
    let test_contract_bytecode = hex::decode("604260005260206000A060206000611234a100").unwrap();

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

    let mut tracer = EventOperationTracer::new();

    let result = chain.run_block_with_extra_stats(vec![encoded_tx], None, None, None, &mut tracer);

    assert!(result.is_ok(), "Block execution should succeed");
    let (block_output, _, _) = result.unwrap();
    assert!(
        block_output.tx_results[0].is_ok(),
        "Transaction should succeed with correct tracer calls. Result: {:?}",
        block_output.tx_results[0]
    );

    // We should have exactly 2 events (LOG0 and LOG1)
    assert_eq!(
        tracer.calls.events.len(),
        2,
        "Should have captured exactly 2 events"
    );

    let contract_address = B160::from_be_bytes(contract_address.into_array());
    assert_eq!(
        tracer.calls.events[0],
        (
            1, // EVM
            contract_address,
            vec![],
            hex::decode("0000000000000000000000000000000000000000000000000000000000000042")
                .unwrap()
        )
    );
    assert_eq!(
        tracer.calls.events[1],
        (
            1, // EVM
            contract_address,
            vec![Bytes32::from_hex(
                "0000000000000000000000000000000000000000000000000000000000000020"
            )],
            vec![]
        )
    );
}
