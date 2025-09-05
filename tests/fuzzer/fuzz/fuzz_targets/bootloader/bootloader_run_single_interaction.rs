#![no_main]
#![feature(allocator_api)]
#![allow(incomplete_features)]
#![feature(generic_const_exprs)]

use arbitrary::{Arbitrary, Result, Unstructured};
use basic_bootloader::bootloader::runner::RunnerMemoryBuffers;
use basic_bootloader::bootloader::BasicBootloader;
use common::mock_oracle_balance;
use common::{abi_push_bytes, abi_push_bytes32_array, enc_addr, enc_u16, enc_u256, enc_u32};
use libfuzzer_sys::fuzz_target;
use rig::forward_system::run::test_impl::{InMemoryPreimageSource, InMemoryTree, TxListSource};
use rig::forward_system::system::system::ForwardRunningSystem;
use rig::ruint::aliases::{B160, U256};
use system_hooks::addresses_constants::{
    CONTRACT_DEPLOYER_ADDRESS, L1_MESSENGER_ADDRESS, L2_BASE_TOKEN_ADDRESS,
};
use system_hooks::contract_deployer::{L2_COMPLEX_UPGRADER_ADDRESS, SET_EVM_BYTECODE_DETAILS};
use system_hooks::l1_messenger::SEND_TO_L1_SELECTOR;
use system_hooks::l2_base_token::{
    FINALIZE_ETH_WITHDRAWAL_SELECTOR, WITHDRAW_SELECTOR, WITHDRAW_WITH_MESSAGE_SELECTOR,
};
use system_hooks::HooksStorage;
use zk_ee::reference_implementations::{BaseResources, DecreasingNative};
use zk_ee::system::tracer::NopTracer;
use zk_ee::system::{Resource, System};

mod common;

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

fn cd_withdraw(addr: [u8; 20]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + 32);
    out.extend_from_slice(WITHDRAW_SELECTOR);
    out.extend_from_slice(&enc_addr(addr));
    out
}

fn cd_withdraw_with_message(addr: [u8; 20], msg: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(WITHDRAW_WITH_MESSAGE_SELECTOR);

    let mut head: Vec<[u8; 32]> = vec![enc_addr(addr)]; // arg0
    let mut tail = Vec::new();
    abi_push_bytes(&mut head, &mut tail, msg, 32 * 2); // arg1 offset

    for w in head {
        out.extend_from_slice(&w);
    }
    out.extend_from_slice(&tail);
    out
}

fn cd_finalize_eth_withdrawal(
    a0: U256,
    a1: U256,
    a2: u16,
    data: &[u8],
    arr: &[[u8; 32]],
) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(FINALIZE_ETH_WITHDRAWAL_SELECTOR);

    let mut head: Vec<[u8; 32]> = Vec::with_capacity(5);
    let mut tail = Vec::new();
    head.push(enc_u256(a0)); // uint256
    head.push(enc_u256(a1)); // uint256
    head.push(enc_u16(a2)); // uint16 (ABI-padded to 32)
    abi_push_bytes(&mut head, &mut tail, data, 32 * 5); // bytes
    abi_push_bytes32_array(&mut head, &mut tail, arr, 32 * 5); // bytes32[]

    for w in head {
        out.extend_from_slice(&w);
    }
    out.extend_from_slice(&tail);
    out
}

fn cd_set_bytecode_details_evm(
    addr: [u8; 20],
    bytecode_hash: [u8; 32],
    bytecode_len: u32,
    observable_bytecode_hash: [u8; 32],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + 32 * 4);
    out.extend_from_slice(SET_EVM_BYTECODE_DETAILS);
    out.extend_from_slice(&enc_addr(addr));
    out.extend_from_slice(&bytecode_hash);
    out.extend_from_slice(&enc_u32(bytecode_len));
    out.extend_from_slice(&observable_bytecode_hash);
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
        if selector == 4 {
            let bias: u8 = Arbitrary::arbitrary(u)?; // 0..=255
            if (bias as usize) < (u8::MAX as usize * 9) / 10 {
                from = L2_COMPLEX_UPGRADER_ADDRESS.to_be_bytes();
            }
        }

        // Build calldata2 for the chosen branch
        let calldata2_raw: Vec<u8> = match selector {
            3 => {
                // L2 base token
                match u.int_in_range(0..=2)? {
                    0 => {
                        // withdraw(address)
                        let a: [u8; 20] = Arbitrary::arbitrary(u)?;
                        cd_withdraw(a)
                    }
                    1 => {
                        // withdrawWithMessage(address,bytes)
                        let a: [u8; 20] = Arbitrary::arbitrary(u)?;
                        // cap message length to keep runs quick
                        let mut m: Vec<u8> = Arbitrary::arbitrary(u)?;
                        if m.len() > 512 {
                            m.truncate(512);
                        }
                        cd_withdraw_with_message(a, &m)
                    }
                    _ => {
                        // finalizeEthWithdrawal(uint256,uint256,uint16,bytes,bytes32[])
                        let a0 = U256::from_be_bytes(<[u8; 32]>::arbitrary(u)?);
                        let a1 = U256::from_be_bytes(<[u8; 32]>::arbitrary(u)?);
                        let a2: u16 = Arbitrary::arbitrary(u)?;
                        let mut data: Vec<u8> = Arbitrary::arbitrary(u)?;
                        if data.len() > 512 {
                            data.truncate(512);
                        }
                        let n: usize = (u8::arbitrary(u)? % 4) as usize; // up to 3 elements
                        let mut arr = Vec::with_capacity(n);
                        for _ in 0..n {
                            arr.push(<[u8; 32]>::arbitrary(u)?);
                        }
                        cd_finalize_eth_withdrawal(a0, a1, a2, &data, &arr)
                    }
                }
            }
            4 => {
                // contract_deployer: setBytecodeDetailsEVM(address,bytes32,uint32,bytes32)
                let addr: [u8; 20] = Arbitrary::arbitrary(u)?;
                let bytecode_hash: [u8; 32] = Arbitrary::arbitrary(u)?;
                let bytecode_len: u32 = Arbitrary::arbitrary(u)?;
                let observable_bytecode_hash: [u8; 32] = Arbitrary::arbitrary(u)?;
                cd_set_bytecode_details_evm(
                    addr,
                    bytecode_hash,
                    bytecode_len,
                    observable_bytecode_hash,
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

    let mut system = System::<
        ForwardRunningSystem<InMemoryTree, InMemoryPreimageSource, TxListSource>,
    >::init_from_oracle(mock_oracle_balance(from, amount))
    .expect("Failed to initialize the mock system");
    let mut system_functions = HooksStorage::new_in(system.get_allocator());
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
            let _ = BasicBootloader::mint_token(&mut system, &amount, &from, &mut inf_resources);
        }
        1 => {
            // Fuzz-test run_single_interaction
            let calldata = input.calldata1;

            let _ = BasicBootloader::run_single_interaction(
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
            );
        }
        2 => {
            // Fuzz-test l1_messenger hook
            system_functions.add_l1_messenger();

            let amount = U256::from_be_bytes([0; 32]);

            let calldata = &input.calldata2.raw;

            let _ = BasicBootloader::run_single_interaction(
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
            );
        }
        3 => {
            // Fuzz-test l2_base_token hook
            system_functions.add_l2_base_token();

            let amount = U256::from_be_bytes([0; 32]);

            let calldata = &input.calldata2.raw;

            let _ = BasicBootloader::run_single_interaction(
                &mut system,
                &mut system_functions,
                memories,
                calldata,
                &from,
                &L2_BASE_TOKEN_ADDRESS,
                inf_resources,
                &amount,
                true,
                &mut NopTracer::default(),
            );
        }
        4 => {
            // Fuzz-test contract_deployer hook
            system_functions.add_contract_deployer();

            let amount = U256::from_be_bytes([0; 32]);

            let calldata = &input.calldata2.raw;

            let _ = BasicBootloader::run_single_interaction(
                &mut system,
                &mut system_functions,
                memories,
                calldata,
                &from,
                &CONTRACT_DEPLOYER_ADDRESS,
                inf_resources,
                &amount,
                true,
                &mut NopTracer::default(),
            );
        }
        _ => (),
    }
}

fuzz_target!(|input: FuzzInput| {
    // call fuzzing in a separate function, so we can see its coverage
    fuzz(input);
});
