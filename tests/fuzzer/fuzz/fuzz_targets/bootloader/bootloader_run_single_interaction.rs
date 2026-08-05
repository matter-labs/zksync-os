#![no_main]
#![feature(allocator_api)]
#![allow(incomplete_features)]

use arbitrary::{Arbitrary, Result, Unstructured};
use basic_bootloader::bootloader::runner::RunnerMemoryBuffers;
use basic_bootloader::bootloader::transaction_flow::zk::process_l1_transaction::transfer_from_treasury;
use basic_bootloader::bootloader::transaction_flow::zk::ZkTransactionFlowOnlyEOA;
use basic_bootloader::bootloader::BasicBootloader;
use common::mock_oracle_balance;
use common::{abi_push_bytes, abi_push_bytes32_array, enc_addr, enc_u16, enc_u256, enc_u32};
use libfuzzer_sys::fuzz_target;
use rig::forward_system::run::test_impl::{InMemoryPreimageSource, InMemoryTree};
use rig::forward_system::system::system_types::ForwardRunningSystem;
use rig::ruint::aliases::{B160, U256};
use system_hooks::addresses_constants::{
    CONTRACT_DEPLOYER_ADDRESS, L1_MESSENGER_ADDRESS, L2_BASE_TOKEN_ADDRESS,
    SET_BYTECODE_ON_ADDRESS_HOOK,
};
use zk_ee::common_structs::system_hooks::HooksStorage;
use zk_ee::reference_implementations::{BaseResources, DecreasingNative};
use zk_ee::system::tracer::NopTracer;
use zk_ee::system::validator::NopTxValidator;
use zk_ee::system::validator::TxValidator;
use zk_ee::system::{Resource, System};
mod common;

// sendToL1(bytes) - 62f84b24
const SEND_TO_L1_SELECTOR: &[u8] = &[0x62, 0xf8, 0x4b, 0x24];

// setBytecodeDetailsEVM(address,bytes32,uint32,bytes32,uint32)
const SET_EVM_BYTECODE_DETAILS: &[u8] = &[0x23, 0x1b, 0x39, 0x57];

#[derive(Debug)]
struct CallDataFuzz {
    raw: Box<[u8]>,
}

#[derive(Debug)]
struct FuzzInput<'a> {
    // To run specific fuzz sub-test: #[arbitrary(value = 1)]
    // To exclude specific fuzz sub-tests: #[arbitrary(with = |u: &mut Unstructured| Ok(*u.choose(&[0,1]).unwrap()))]
    // To run all: #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=4))]
    selector: u8,

    from: [u8; 20],
    to: [u8; 20],

    amount: [u8; 32],

    calldata1: &'a [u8],

    calldata2: CallDataFuzz,
}

fn cd_set_bytecode_details_evm(
    addr: [u8; 20],
    bytecode_hash: [u8; 32],
    bytecode_len: u32,
    observable_bytecode_hash: [u8; 32],
    observable_bytecode_len: u32,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + 32 * 4);
    out.extend_from_slice(SET_EVM_BYTECODE_DETAILS);
    out.extend_from_slice(&enc_addr(addr));
    out.extend_from_slice(&bytecode_hash);
    out.extend_from_slice(&enc_u32(bytecode_len));
    out.extend_from_slice(&observable_bytecode_hash);
    out.extend_from_slice(&enc_u32(observable_bytecode_len));
    out
}

impl<'a> Arbitrary<'a> for FuzzInput<'a> {
    fn arbitrary(u: &mut Unstructured<'a>) -> Result<Self> {
        // Which branch we’ll execute later
        // TODO: contract_deployer is disabled, as it's easy to make it panic
        // by submitting a hash with no preimage.
        // We need a smarter generator for it.
        let selector = u.int_in_range(0..=3)?;

        // Base fields
        let mut from: [u8; 20] = Arbitrary::arbitrary(u)?;
        let to: [u8; 20] = Arbitrary::arbitrary(u)?;
        let amount: [u8; 32] = Arbitrary::arbitrary(u)?;
        let calldata1: &'a [u8] = Arbitrary::arbitrary(u)?;

        // For deployer: 90% of the time use the upgrader address as `from`, 10% random
        if selector == 3 {
            let bias: u8 = Arbitrary::arbitrary(u)?; // 0..=255
            if (bias as usize) < (u8::MAX as usize * 9) / 10 {
                from = CONTRACT_DEPLOYER_ADDRESS.to_be_bytes();
            }
        }

        // Build calldata2 for the chosen branch
        let calldata2_raw: Vec<u8> = match selector {
            3 => {
                // contract_deployer: setBytecodeDetailsEVM(address,bytes32,uint32,bytes32,uint32)
                // function extended by the following parameter: observable_bytecode_len
                let addr: [u8; 20] = Arbitrary::arbitrary(u)?;
                let bytecode_hash: [u8; 32] = Arbitrary::arbitrary(u)?;
                let bytecode_len: u32 = Arbitrary::arbitrary(u)?;
                let observable_bytecode_hash: [u8; 32] = Arbitrary::arbitrary(u)?;
                let observable_bytecode_len: u32 = Arbitrary::arbitrary(u)?;
                cd_set_bytecode_details_evm(
                    addr,
                    bytecode_hash,
                    bytecode_len,
                    observable_bytecode_hash,
                    observable_bytecode_len,
                )
            }
            2 => {
                // l1_messenger: sendToL1(bytes)
                let payload: Vec<u8> = Arbitrary::arbitrary(u)?;
                let mut vv = Vec::new();
                vv.extend_from_slice(SEND_TO_L1_SELECTOR);
                vv.extend_from_slice(&enc_u256(U256::from(32)));
                vv.extend_from_slice(&enc_u256(U256::from(payload.len() as u64)));
                vv.extend_from_slice(&payload);
                vv
            }
            _ => {
                // fallback: small random bytes so branch 1 etc can still do something
                let v: Vec<u8> = Arbitrary::arbitrary(u)?;
                v
            }
        };

        Ok(FuzzInput {
            selector,
            from,
            to,
            amount,
            calldata1,
            calldata2: CallDataFuzz {
                raw: calldata2_raw.into_boxed_slice(),
            },
        })
    }
}

fn fuzz(input: FuzzInput) {
    let from = B160::from_be_bytes(input.from);
    let to = B160::from_be_bytes(input.to);
    let amount = U256::from_be_bytes(input.amount);
    let selector = input.selector;

    let (metadata, oracle) = mock_oracle_balance(from, amount);

    let mut system =
        System::<ForwardRunningSystem>::init_from_metadata_and_oracle(metadata, oracle)
            .expect("Failed to initialize the mock system");
    let mut system_functions = HooksStorage::new_in(system.get_allocator());

    system_hooks::add_precompiles(&mut system_functions).expect("Should add precompiles");

    system_hooks::add_l1_messenger(&mut system_functions).expect("Should add l1_messenger");
    system_hooks::add_set_bytecode_on_address_hook(&mut system_functions)
        .expect("Should add set_bytecode_on_address_hook");
    system_hooks::add_interop_root_reporter(&mut system_functions)
        .expect("Should add interop_root_reporter");

    let mut inf_resources = <BaseResources<DecreasingNative> as Resource>::FORMAL_INFINITE;
    pub const MAX_HEAP_BUFFER_SIZE: usize = 1 << 27; // 128 MB
    pub const MAX_RETURN_BUFFER_SIZE: usize = 1 << 28; // 256 MB

    let mut heaps = Box::new_uninit_slice_in(MAX_HEAP_BUFFER_SIZE, system.get_allocator());
    let mut return_data = Box::new_uninit_slice_in(MAX_RETURN_BUFFER_SIZE, system.get_allocator());

    let memories = RunnerMemoryBuffers {
        heaps: &mut heaps,
        return_data: &mut return_data,
    };

    match selector {
        0 => {
            let _ = transfer_from_treasury::<ForwardRunningSystem>(
                &mut system,
                &amount,
                &from,
                &mut inf_resources,
                false,
            );
        }
        1 => {
            // Fuzz-test run_single_interaction
            let calldata = input.calldata1;

            let _ = BasicBootloader::<_, ZkTransactionFlowOnlyEOA<ForwardRunningSystem>>::run_single_interaction(
                &mut system,
                &mut system_functions,
                memories,
                calldata,
                &from,
                &to,
                inf_resources,
                &amount,
                true,
                &mut NopTracer::default(),
                &mut NopTxValidator::default(),
            );
        }
        2 => {
            // Fuzz-test l1_messenger hook

            let amount = U256::from_be_bytes([0; 32]);

            let calldata = &input.calldata2.raw;

            let _ = BasicBootloader::<_, ZkTransactionFlowOnlyEOA<ForwardRunningSystem>>::run_single_interaction(
                &mut system,
                &mut system_functions,
                memories,
                calldata,
                &from,
                &L1_MESSENGER_ADDRESS,
                inf_resources,
                &amount,
                true,
                &mut NopTracer::default(),
                &mut NopTxValidator::default(),
            );
        }
        3 => {
            let amount = U256::from_be_bytes([0; 32]);

            let calldata = &input.calldata2.raw;

            let _ = BasicBootloader::<_, ZkTransactionFlowOnlyEOA<ForwardRunningSystem>>::run_single_interaction(
                &mut system,
                &mut system_functions,
                memories,
                calldata,
                &from,
                &SET_BYTECODE_ON_ADDRESS_HOOK,
                inf_resources,
                &amount,
                true,
                &mut NopTracer::default(),
                &mut NopTxValidator::default(),
            );
        }
        _ => (),
    }
}

fuzz_target!(|input: FuzzInput| {
    // call fuzzing in a separate function, so we can see its coverage
    fuzz(input);
});
