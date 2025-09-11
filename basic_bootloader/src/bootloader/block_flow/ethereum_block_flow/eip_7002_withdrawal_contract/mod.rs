// NOTE: we implement 7002 contract as non-solidity/non-EMV contract as:
// - there is no GAS opcode in the reference bytecode
// - whatever will be the gas supplied to the frame - it'll be sufficient to pop as up to upper bound of elements
// - and to be honest, putting bytecode into execution client is so-so idea, and instead consensus can be instead reached on implementation
// Bytecode for this contract will anyway exist for requests creation in transactions themselves

use core::fmt::Write;
use ruint::aliases::B160;
use ruint::aliases::U256;
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::internal_error;
use zk_ee::kv_markers::ExactSizeChain;
use zk_ee::system::errors::system::SystemError;
use zk_ee::system::logger::Logger;
use zk_ee::system::AccountDataRequest;
use zk_ee::system::Computational;
use zk_ee::system::IOSubsystemExt;
use zk_ee::system::Resources;
use zk_ee::system::System;
use zk_ee::system::{EthereumLikeTypes, IOSubsystem};
use zk_ee::utils::{u256_to_usize_saturated, Bytes32};

pub const WITHDRAWAL_REQUEST_EIP_7685_TYPE: u8 = 0x01;

pub const WITHDRAWAL_REQUEST_PREDEPLOY_ADDRESS: B160 =
    B160::from_limbs([0xd83579A64c007002, 0xEf480Eb55e80D19a, 0x00000961]);

const EXCESS_WITHDRAWAL_REQUESTS_STORAGE_SLOT: Bytes32 =
    Bytes32::from_hex("0000000000000000000000000000000000000000000000000000000000000000");
const WITHDRAWAL_REQUEST_COUNT_STORAGE_SLOT: Bytes32 =
    Bytes32::from_hex("0000000000000000000000000000000000000000000000000000000000000001");
const TARGET_WITHDRAWAL_REQUESTS_PER_BLOCK: usize = 2;

const WITHDRAWAL_REQUEST_QUEUE_HEAD_STORAGE_SLOT: Bytes32 =
    Bytes32::from_hex("0000000000000000000000000000000000000000000000000000000000000002");
const WITHDRAWAL_REQUEST_QUEUE_TAIL_STORAGE_SLOT: Bytes32 =
    Bytes32::from_hex("0000000000000000000000000000000000000000000000000000000000000003");
const WITHDRAWAL_REQUEST_QUEUE_STORAGE_OFFSET: U256 = U256::from_limbs([4, 0, 0, 0]);
const SLOTS_PER_REQUEST: U256 = U256::from_limbs([3, 0, 0, 0]);

const MAX_WITHDRAWAL_REQUESTS_PER_BLOCK: usize = 16;

// it's fully fixed
const WITHDRAWAL_REQUEST_SSZ_SERIALIZATION_LEN: usize = 20 + 48 + 8;

// NOTE: even though the spec says SSZ.encode (that is NOT a concatenation of element for the list), it actually appends nothing if there are no intercations
pub fn eip7002_system_part<S: EthereumLikeTypes>(
    system: &mut System<S>,
    requests_hasher: &mut impl crypto::sha256::Digest,
) -> Result<bool, SystemError>
where
    S::IO: IOSubsystemExt,
{
    let mut resources = S::Resources::from_native(
        <S::Resources as Resources>::Native::from_computational(u64::MAX),
    );

    let props = resources.with_infinite_ergs(|resources| {
        system.io.read_account_properties(
            ExecutionEnvironmentType::NoEE,
            resources,
            &WITHDRAWAL_REQUEST_PREDEPLOY_ADDRESS,
            AccountDataRequest::empty()
                .with_nonce()
                .with_observable_bytecode_len(),
        )
    })?;

    let is_contract = props.nonce.0 == 1 && props.observable_bytecode_len.0 > 0;
    if is_contract == false {
        return Err(SystemError::LeafDefect(internal_error!(
            "EIP-7002 withdrawal contract is not deployed"
        )));
    }

    // {
    // use zk_ee::memory::slice_vec::SliceVec;
    // use zk_ee::system::tracer::NopTracer;
    //     let mut resources = S::Resources::from_native(
    //         <S::Resources as Resources>::Native::from_computational(u64::MAX),
    //     );

    //     let props = resources.with_infinite_ergs(|resources| {
    //         system.io.read_account_properties(
    //             ExecutionEnvironmentType::NoEE,
    //             resources,
    //             &WITHDRAWAL_REQUEST_PREDEPLOY_ADDRESS,
    //             AccountDataRequest::empty()
    //                 .with_nonce()
    //                 .with_observable_bytecode_len()
    //                 .with_bytecode()
    //         )
    //     })?;

    //     use zk_ee::system::*;
    //     let mut interpreter = evm_interpreter::Interpreter::new(system).unwrap();
    //     let bytecode_raw = hex::decode("3373fffffffffffffffffffffffffffffffffffffffe1460cb5760115f54807fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff146101f457600182026001905f5b5f82111560685781019083028483029004916001019190604d565b909390049250505036603814608857366101f457346101f4575f5260205ff35b34106101f457600154600101600155600354806003026004013381556001015f35815560010160203590553360601b5f5260385f601437604c5fa0600101600355005b6003546002548082038060101160df575060105b5f5b8181146101835782810160030260040181604c02815460601b8152601401816001015481526020019060020154807fffffffffffffffffffffffffffffffff00000000000000000000000000000000168252906010019060401c908160381c81600701538160301c81600601538160281c81600501538160201c81600401538160181c81600301538160101c81600201538160081c81600101535360010160e1565b910180921461019557906002556101a0565b90505f6002555f6003555b5f54807fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff14156101cd57505f5b6001546002828201116101e25750505f6101e8565b01600290035b5f555f600155604c025ff35b5f5ffd").unwrap();
    //     let bytecode_ref: &'static [u8] = unsafe {
    //         core::mem::transmute(&bytecode_raw[..])
    //     };
    //     let bytecode = Bytecode::Decommitted { bytecode: bytecode_ref, unpadded_code_len: bytecode_raw.len() as u32, artifacts_len: 0, code_version: 0 };
    //     let resources = S::Resources::from_ergs_and_native(Ergs(30_000_000 * 256), <S::Resources as Resources>::Native::from_computational(u64::MAX),);
    //     let frame = ExecutionEnvironmentLaunchParams {
    //         external_call: ExternalCallRequest {
    //             available_resources: resources,
    //             ergs_to_pass: Ergs(30_000_000 * 256),
    //             caller: B160::from_str_radix("fffffffffffffffffffffffffffffffffffffffe", 16).unwrap(),
    //             callee: B160::from_str_radix("00000961Ef480Eb55e80D19ad83579A64c007002", 16).unwrap(),
    //             callers_caller: B160::ZERO,
    //             modifier: CallModifier::NoModifier,
    //             calldata: &[],
    //             nominal_token_value: U256::ZERO,
    //             call_scratch_space: None,
    //         },
    //         environment_parameters: EnvironmentParameters {
    //             bytecode,
    //             scratch_space_len: 0,
    //         }
    //     };
    //     let mut heap_buffer: Vec<u8> = Vec::with_capacity(1 << 26);
    //     let buffer: &'static mut [core::mem::MaybeUninit<u8>] = unsafe {
    //         core::mem::transmute(heap_buffer.spare_capacity_mut())
    //     };
    //     let heap = SliceVec::new(buffer);
    //     let result = interpreter.start_executing_frame(system, frame, heap, &mut NopTracer::default()).unwrap();
    //     match result {
    //         ExecutionEnvironmentPreemptionPoint::End(TransactionEndPoint::CompletedExecution(
    //             CompletedExecution {
    //                 return_values,
    //                 ..
    //             }
    //         )) => {
    //             let ReturnValues { returndata, return_scratch_space } = return_values;
    //             dbg!(hex::encode(returndata));
    //         },
    //         _ => panic!()
    //     }
    // }

    let queue_head_index = resources.with_infinite_ergs(|resources| {
        system.io.storage_read::<false>(
            ExecutionEnvironmentType::NoEE,
            resources,
            &WITHDRAWAL_REQUEST_PREDEPLOY_ADDRESS,
            &WITHDRAWAL_REQUEST_QUEUE_HEAD_STORAGE_SLOT,
        )
    })?;

    let queue_tail_index = resources.with_infinite_ergs(|resources| {
        system.io.storage_read::<false>(
            ExecutionEnvironmentType::NoEE,
            resources,
            &WITHDRAWAL_REQUEST_PREDEPLOY_ADDRESS,
            &WITHDRAWAL_REQUEST_QUEUE_TAIL_STORAGE_SLOT,
        )
    })?;

    let queue_head_index = U256::from_be_bytes(queue_head_index.as_u8_array());
    let queue_tail_index = U256::from_be_bytes(queue_tail_index.as_u8_array());

    let num_in_queue = queue_tail_index - queue_head_index;
    let num_dequeued = core::cmp::min(
        u256_to_usize_saturated(&num_in_queue),
        MAX_WITHDRAWAL_REQUESTS_PER_BLOCK,
    );

    if num_dequeued == 0 {
        // we do not even need to reset the queue pointers as it's a hard invariant
        assert!(queue_head_index.is_zero());
        assert!(queue_tail_index.is_zero());
        update_excess_withdrawal_requests_and_reset_count(system)?;
        return Ok(false);
    }

    requests_hasher.update([WITHDRAWAL_REQUEST_EIP_7685_TYPE]);

    let mut logger = system.get_logger();

    for i in 0..num_dequeued {
        let queue_storage_slot = WITHDRAWAL_REQUEST_QUEUE_STORAGE_OFFSET
            + ((queue_head_index + U256::from(i as u64)) * SLOTS_PER_REQUEST);
        let slot_0 = Bytes32::from_array(queue_storage_slot.to_be_bytes::<32>());
        let slot_1 = Bytes32::from_array((queue_storage_slot + U256::from(1)).to_be_bytes::<32>());
        let slot_2 = Bytes32::from_array((queue_storage_slot + U256::from(2)).to_be_bytes::<32>());

        let slot_0 = resources.with_infinite_ergs(|resources| {
            system.io.storage_read::<false>(
                ExecutionEnvironmentType::NoEE,
                resources,
                &WITHDRAWAL_REQUEST_PREDEPLOY_ADDRESS,
                &slot_0,
            )
        })?;
        let slot_1 = resources.with_infinite_ergs(|resources| {
            system.io.storage_read::<false>(
                ExecutionEnvironmentType::NoEE,
                resources,
                &WITHDRAWAL_REQUEST_PREDEPLOY_ADDRESS,
                &slot_1,
            )
        })?;
        let slot_2 = resources.with_infinite_ergs(|resources| {
            system.io.storage_read::<false>(
                ExecutionEnvironmentType::NoEE,
                resources,
                &WITHDRAWAL_REQUEST_PREDEPLOY_ADDRESS,
                &slot_2,
            )
        })?;

        let _ = logger.write_fmt(format_args!(
            "Processing EIP-7002 withdrawal queue element with:"
        ));

        let _ = logger.write_fmt(format_args!("\nAddress = "));
        let address = &slot_0.as_u8_array_ref()[12..];
        let _ = logger.log_data(address.iter().copied());
        requests_hasher.update(address);

        let pubkey_part_0 = slot_1.as_u8_array_ref();
        let pubkey_part_1 = &slot_2.as_u8_array_ref()[..16];

        requests_hasher.update(pubkey_part_0);
        requests_hasher.update(pubkey_part_1);
        let _ = logger.write_fmt(format_args!("\nPubkey = "));
        let _ = logger.log_data(ExactSizeChain::new(
            pubkey_part_0.iter().copied(),
            pubkey_part_1.iter().copied(),
        ));

        // NOTE: we need to bytereverse it
        let amount = &slot_2.as_u8_array_ref()[16..][..8];
        let amount = u64::from_be_bytes(amount.try_into().unwrap());
        let _ = logger.write_fmt(format_args!("\nAmount = {}\n", amount,));
        requests_hasher.update(amount.to_le_bytes());
    }

    let new_queue_head_index = queue_head_index + U256::from(num_dequeued as u64);
    if new_queue_head_index == queue_tail_index {
        let _ = logger.write_fmt(format_args!("EIP-7002 withdrawal queue is now empty\n"));

        resources.with_infinite_ergs(|resources| {
            system.io.storage_write::<false>(
                ExecutionEnvironmentType::NoEE,
                resources,
                &WITHDRAWAL_REQUEST_PREDEPLOY_ADDRESS,
                &WITHDRAWAL_REQUEST_QUEUE_HEAD_STORAGE_SLOT,
                &Bytes32::ZERO,
            )
        })?;

        resources.with_infinite_ergs(|resources| {
            system.io.storage_write::<false>(
                ExecutionEnvironmentType::NoEE,
                resources,
                &WITHDRAWAL_REQUEST_PREDEPLOY_ADDRESS,
                &WITHDRAWAL_REQUEST_QUEUE_TAIL_STORAGE_SLOT,
                &Bytes32::ZERO,
            )
        })?;
    } else {
        let value = Bytes32::from_array(new_queue_head_index.to_be_bytes::<32>());
        resources.with_infinite_ergs(|resources| {
            system.io.storage_write::<false>(
                ExecutionEnvironmentType::NoEE,
                resources,
                &WITHDRAWAL_REQUEST_PREDEPLOY_ADDRESS,
                &WITHDRAWAL_REQUEST_QUEUE_HEAD_STORAGE_SLOT,
                &value,
            )
        })?;
    }

    update_excess_withdrawal_requests_and_reset_count(system)?;

    Ok(true)
}

fn update_excess_withdrawal_requests_and_reset_count<S: EthereumLikeTypes>(
    system: &mut System<S>,
) -> Result<(), SystemError>
where
    S::IO: IOSubsystemExt,
{
    let mut resources = S::Resources::from_native(
        <S::Resources as Resources>::Native::from_computational(u64::MAX),
    );

    let mut previous_excess = resources.with_infinite_ergs(|resources| {
        system.io.storage_read::<false>(
            ExecutionEnvironmentType::NoEE,
            resources,
            &WITHDRAWAL_REQUEST_PREDEPLOY_ADDRESS,
            &EXCESS_WITHDRAWAL_REQUESTS_STORAGE_SLOT,
        )
    })?;

    if previous_excess == Bytes32::MAX {
        previous_excess = Bytes32::ZERO;
    }

    let count = resources.with_infinite_ergs(|resources| {
        system.io.storage_read::<false>(
            ExecutionEnvironmentType::NoEE,
            resources,
            &WITHDRAWAL_REQUEST_PREDEPLOY_ADDRESS,
            &WITHDRAWAL_REQUEST_COUNT_STORAGE_SLOT,
        )
    })?;

    let base_count = U256::from_be_bytes(previous_excess.as_u8_array())
        + U256::from_be_bytes(count.as_u8_array());

    let (mut maybe_new_excess, uf) =
        base_count.overflowing_sub(U256::from(TARGET_WITHDRAWAL_REQUESTS_PER_BLOCK as u64));
    if uf {
        maybe_new_excess = U256::ZERO;
    }

    let new_excess = Bytes32::from_array(maybe_new_excess.to_be_bytes::<32>());
    resources.with_infinite_ergs(|resources| {
        system.io.storage_write::<false>(
            ExecutionEnvironmentType::NoEE,
            resources,
            &WITHDRAWAL_REQUEST_PREDEPLOY_ADDRESS,
            &EXCESS_WITHDRAWAL_REQUESTS_STORAGE_SLOT,
            &new_excess,
        )
    })?;

    // reset count
    resources.with_infinite_ergs(|resources| {
        system.io.storage_write::<false>(
            ExecutionEnvironmentType::NoEE,
            resources,
            &WITHDRAWAL_REQUEST_PREDEPLOY_ADDRESS,
            &WITHDRAWAL_REQUEST_COUNT_STORAGE_SLOT,
            &Bytes32::ZERO,
        )
    })?;

    Ok(())
}
