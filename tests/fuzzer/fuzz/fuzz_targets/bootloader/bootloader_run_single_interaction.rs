#![no_main]
#![feature(allocator_api)]
#![allow(incomplete_features)]
#![feature(generic_const_exprs)]

use arbitrary::{Arbitrary, Result, Unstructured};
use basic_bootloader::bootloader::runner::RunnerMemoryBuffers;
use basic_bootloader::bootloader::BasicBootloader;
use common::mock_oracle_balance;
use libfuzzer_sys::fuzz_target;
use rig::forward_system::run::test_impl::{InMemoryPreimageSource, InMemoryTree, TxListSource};
use rig::forward_system::system::system::ForwardRunningSystem;
use rig::ruint::aliases::{B160, U256};
use system_hooks::addresses_constants::L1_MESSENGER_ADDRESS;
use system_hooks::HooksStorage;
use zk_ee::reference_implementations::{BaseResources, DecreasingNative};
use zk_ee::system::{Resource, System};
use zk_ee::system::tracer::NopTracer;

mod common;

#[derive(Debug)]
struct CallDataFuzz {
    raw: Box<[u8]>,
}

#[derive(Arbitrary, Debug)]
struct FuzzInput<'a> {
    // To run specific fuzz sub-test: #[arbitrary(value = 1)]
    // To exclude specific fuzz sub-tests: #[arbitrary(with = |u: &mut Unstructured| Ok(*u.choose(&[0,1]).unwrap()))]
    // To run all: #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=2))]
    #[arbitrary(with = |u: &mut Unstructured| u.int_in_range(0..=2))]
    selector: u8,

    from: [u8; 20],
    to: [u8; 20],

    amount: [u8; 32],

    // Note: different values for should_start_frame and should_close_frame
    // cause immediate panic for BasicBootloader::run_single_interaction
    #[arbitrary(value = false)]
    should_start_frame: bool,
    #[arbitrary(value = false)]
    should_close_frame: bool,

    calldata1: &'a [u8],

    calldata2: CallDataFuzz,
}

impl<'a> Arbitrary<'a> for CallDataFuzz {
    fn arbitrary(u: &mut Unstructured<'a>) -> Result<Self> {
        let SEND_TO_L1_SELECTOR = &[0x62, 0xf8, 0x4b, 0x24];
        let offset = U256::from_limbs([32 as u64, 0, 0, 0]);
        let v = <Vec<u8>>::arbitrary(u)?;

        let length = U256::from_limbs([v.len() as u64, 0, 0, 0]);

        let mut vv: Vec<u8> = Vec::new();

        vv.extend(SEND_TO_L1_SELECTOR);
        vv.extend(offset.to_be_bytes_vec());
        vv.extend(length.to_be_bytes_vec());
        vv.extend(v);

        Ok(CallDataFuzz { raw: vv.into() })
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
    pub const MAX_RETURN_BUFFER_SIZE: usize = 1 << 27; // 128 MB

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
                &mut NopTracer::default()
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
                &mut NopTracer::default()
            );
        }
        _ => (),
    }
}

fuzz_target!(|input: FuzzInput| {
    // call fuzzing in a separate function, so we can see its coverage
    fuzz(input);
});
