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

pub const CONSOLIDATION_REQUEST_EIP_7685_TYPE: u8 = 0x02;

pub const CONSOLIDATION_REQUEST_PREDEPLOY_ADDRESS: B160 =
    B160::from_limbs([0x8B00f3a590007251, 0xc7CE488642fb579F, 0x0000BBdD]);

const EXCESS_CONSOLIDATION_REQUESTS_STORAGE_SLOT: Bytes32 = Bytes32::ZERO;
const CONSOLIDATION_REQUEST_COUNT_STORAGE_SLOT: Bytes32 =
    Bytes32::from_hex("0000000000000000000000000000000000000000000000000000000000000001");
const CONSOLIDATION_REQUEST_QUEUE_HEAD_STORAGE_SLOT: Bytes32 =
    Bytes32::from_hex("0000000000000000000000000000000000000000000000000000000000000002");
const CONSOLIDATION_REQUEST_QUEUE_TAIL_STORAGE_SLOT: Bytes32 =
    Bytes32::from_hex("0000000000000000000000000000000000000000000000000000000000000003");
const CONSOLIDATION_REQUEST_QUEUE_STORAGE_OFFSET: U256 = U256::from_limbs([4, 0, 0, 0]);
const SLOTS_PER_REQUEST: U256 = U256::from_limbs([4, 0, 0, 0]);

const TARGET_CONSOLIDATION_REQUESTS_PER_BLOCK: usize = 1;
const MAX_CONSOLIDATION_REQUESTS_PER_BLOCK: usize = 2;

// it's fully fixed
const CONSOLIDATION_REQUEST_SSZ_SERIALIZATION_LEN: usize = 20 + 48 + 48;

// NOTE: even though the spec says SSZ.encode (that is NOT a concatenation of element for the list), it actually appends nothing if there are no intercations
pub fn eip7251_system_part<S: EthereumLikeTypes>(
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
            &CONSOLIDATION_REQUEST_PREDEPLOY_ADDRESS,
            AccountDataRequest::empty()
                .with_nonce()
                .with_observable_bytecode_len(),
        )
    })?;

    let is_contract = props.nonce.0 == 1 && props.observable_bytecode_len.0 > 0;
    if is_contract == false {
        return Err(SystemError::LeafDefect(internal_error!(
            "EIP-7251 consolidation contract is not deployed"
        )));
    }

    let queue_head_index = resources.with_infinite_ergs(|resources| {
        system.io.storage_read::<false>(
            ExecutionEnvironmentType::NoEE,
            resources,
            &CONSOLIDATION_REQUEST_PREDEPLOY_ADDRESS,
            &CONSOLIDATION_REQUEST_QUEUE_HEAD_STORAGE_SLOT,
        )
    })?;

    let queue_tail_index = resources.with_infinite_ergs(|resources| {
        system.io.storage_read::<false>(
            ExecutionEnvironmentType::NoEE,
            resources,
            &CONSOLIDATION_REQUEST_PREDEPLOY_ADDRESS,
            &CONSOLIDATION_REQUEST_QUEUE_TAIL_STORAGE_SLOT,
        )
    })?;

    let queue_head_index = U256::from_be_bytes(queue_head_index.as_u8_array());
    let queue_tail_index = U256::from_be_bytes(queue_tail_index.as_u8_array());

    let num_in_queue = queue_tail_index - queue_head_index;
    let num_dequeued = core::cmp::min(
        u256_to_usize_saturated(&num_in_queue),
        MAX_CONSOLIDATION_REQUESTS_PER_BLOCK,
    );

    if num_dequeued == 0 {
        // we do not even need to reset the queue pointers as it's a hard invariant
        assert!(queue_head_index.is_zero());
        assert!(queue_tail_index.is_zero());
        update_excess_consolidation_requests_and_reset_count(system)?;
        return Ok(false);
    }

    requests_hasher.update([CONSOLIDATION_REQUEST_EIP_7685_TYPE]);

    let mut logger = system.get_logger();

    for i in 0..num_dequeued {
        let queue_storage_slot = CONSOLIDATION_REQUEST_QUEUE_STORAGE_OFFSET
            + ((queue_head_index + U256::from(i as u64)) * SLOTS_PER_REQUEST);
        let slot_0 = Bytes32::from_array(queue_storage_slot.to_be_bytes::<32>());
        let slot_1 = Bytes32::from_array((queue_storage_slot + U256::from(1)).to_be_bytes::<32>());
        let slot_2 = Bytes32::from_array((queue_storage_slot + U256::from(2)).to_be_bytes::<32>());
        let slot_3 = Bytes32::from_array((queue_storage_slot + U256::from(3)).to_be_bytes::<32>());

        let slot_0 = resources.with_infinite_ergs(|resources| {
            system.io.storage_read::<false>(
                ExecutionEnvironmentType::NoEE,
                resources,
                &CONSOLIDATION_REQUEST_PREDEPLOY_ADDRESS,
                &slot_0,
            )
        })?;
        let slot_1 = resources.with_infinite_ergs(|resources| {
            system.io.storage_read::<false>(
                ExecutionEnvironmentType::NoEE,
                resources,
                &CONSOLIDATION_REQUEST_PREDEPLOY_ADDRESS,
                &slot_1,
            )
        })?;
        let slot_2 = resources.with_infinite_ergs(|resources| {
            system.io.storage_read::<false>(
                ExecutionEnvironmentType::NoEE,
                resources,
                &CONSOLIDATION_REQUEST_PREDEPLOY_ADDRESS,
                &slot_2,
            )
        })?;
        let slot_3 = resources.with_infinite_ergs(|resources| {
            system.io.storage_read::<false>(
                ExecutionEnvironmentType::NoEE,
                resources,
                &CONSOLIDATION_REQUEST_PREDEPLOY_ADDRESS,
                &slot_3,
            )
        })?;

        let _ = logger.write_fmt(format_args!(
            "Processing EIP-7251 consolidation queue element with:"
        ));

        let _ = logger.write_fmt(format_args!("\nAddress = "));
        let address = &slot_0.as_u8_array_ref()[12..];
        let _ = logger.log_data(address.iter().copied());
        requests_hasher.update(address);

        let source_pubkey_part_0 = slot_1.as_u8_array_ref();
        let source_pubkey_part_1 = &slot_2.as_u8_array_ref()[..16];

        requests_hasher.update(source_pubkey_part_0);
        requests_hasher.update(source_pubkey_part_1);
        let _ = logger.write_fmt(format_args!("\nSource pubkey = "));
        let _ = logger.log_data(ExactSizeChain::new(
            source_pubkey_part_0.iter().copied(),
            source_pubkey_part_1.iter().copied(),
        ));

        let target_pubkey_part_0 = &slot_2.as_u8_array_ref()[16..];
        let target_pubkey_part_1 = slot_3.as_u8_array_ref();

        requests_hasher.update(target_pubkey_part_0);
        requests_hasher.update(target_pubkey_part_1);
        let _ = logger.write_fmt(format_args!("\nTarget pubkey = "));
        let _ = logger.log_data(ExactSizeChain::new(
            target_pubkey_part_0.iter().copied(),
            target_pubkey_part_1.iter().copied(),
        ));

        let _ = logger.write_fmt(format_args!("\n"));
    }

    let new_queue_head_index = queue_head_index + U256::from(num_dequeued as u64);
    if new_queue_head_index == queue_tail_index {
        let _ = logger.write_fmt(format_args!("EIP-7251 consolidation queue is now empty\n"));

        resources.with_infinite_ergs(|resources| {
            system.io.storage_write::<false>(
                ExecutionEnvironmentType::NoEE,
                resources,
                &CONSOLIDATION_REQUEST_PREDEPLOY_ADDRESS,
                &CONSOLIDATION_REQUEST_QUEUE_HEAD_STORAGE_SLOT,
                &Bytes32::ZERO,
            )
        })?;

        resources.with_infinite_ergs(|resources| {
            system.io.storage_write::<false>(
                ExecutionEnvironmentType::NoEE,
                resources,
                &CONSOLIDATION_REQUEST_PREDEPLOY_ADDRESS,
                &CONSOLIDATION_REQUEST_QUEUE_TAIL_STORAGE_SLOT,
                &Bytes32::ZERO,
            )
        })?;
    } else {
        let value = Bytes32::from_array(new_queue_head_index.to_be_bytes::<32>());
        resources.with_infinite_ergs(|resources| {
            system.io.storage_write::<false>(
                ExecutionEnvironmentType::NoEE,
                resources,
                &CONSOLIDATION_REQUEST_PREDEPLOY_ADDRESS,
                &CONSOLIDATION_REQUEST_QUEUE_HEAD_STORAGE_SLOT,
                &value,
            )
        })?;
    }

    update_excess_consolidation_requests_and_reset_count(system)?;

    Ok(true)
}

fn update_excess_consolidation_requests_and_reset_count<S: EthereumLikeTypes>(
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
            &CONSOLIDATION_REQUEST_PREDEPLOY_ADDRESS,
            &EXCESS_CONSOLIDATION_REQUESTS_STORAGE_SLOT,
        )
    })?;

    if previous_excess == Bytes32::MAX {
        previous_excess = Bytes32::ZERO;
    }

    let count = resources.with_infinite_ergs(|resources| {
        system.io.storage_read::<false>(
            ExecutionEnvironmentType::NoEE,
            resources,
            &CONSOLIDATION_REQUEST_PREDEPLOY_ADDRESS,
            &CONSOLIDATION_REQUEST_COUNT_STORAGE_SLOT,
        )
    })?;

    let base_count = U256::from_be_bytes(previous_excess.as_u8_array())
        + U256::from_be_bytes(count.as_u8_array());

    let (mut maybe_new_excess, uf) =
        base_count.overflowing_sub(U256::from(TARGET_CONSOLIDATION_REQUESTS_PER_BLOCK as u64));
    if uf {
        maybe_new_excess = U256::ZERO;
    }

    let new_excess = Bytes32::from_array(maybe_new_excess.to_be_bytes::<32>());
    resources.with_infinite_ergs(|resources| {
        system.io.storage_write::<false>(
            ExecutionEnvironmentType::NoEE,
            resources,
            &CONSOLIDATION_REQUEST_PREDEPLOY_ADDRESS,
            &EXCESS_CONSOLIDATION_REQUESTS_STORAGE_SLOT,
            &new_excess,
        )
    })?;

    // reset count
    resources.with_infinite_ergs(|resources| {
        system.io.storage_write::<false>(
            ExecutionEnvironmentType::NoEE,
            resources,
            &CONSOLIDATION_REQUEST_PREDEPLOY_ADDRESS,
            &CONSOLIDATION_REQUEST_COUNT_STORAGE_SLOT,
            &Bytes32::ZERO,
        )
    })?;

    Ok(())
}
