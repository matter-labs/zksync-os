//! EIP-8282 builder execution requests: the end-of-block system calls of the
//! builder deposit and builder exit predeploys, request types `0x03` and
//! `0x04` of the EIP-7685 request bus.
//!
//! Both contracts are the EIP-7002 design with the same storage layout (slot
//! 0 the excess, 1 the count added in the block, 2 and 3 the queue head and
//! tail, records from slot 4 on) and differ in the record layout, the slots a
//! record takes and the per-block caps, so one drain serves both, as the
//! EIP-7002 and EIP-7251 parts do for their contracts: dequeue up to the cap,
//! oldest first, feed the records to the requests hasher, move the head (or
//! reset the queue when it empties), update the excess and reset the count.
//! The system call runs with infinite gas and its reads and writes are
//! accounted in the block access list like any other.
//!
//! [EIP-8282]: https://eips.ethereum.org/EIPS/eip-8282

use crate::bootloader::supported_ees::EthereumLikeSystem;
use core::fmt::Write;
use ruint::aliases::{B160, U256};
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::internal_error;
use zk_ee::logger_log;
use zk_ee::system::errors::system::SystemError;
use zk_ee::system::{
    AccountDataRequest, Computational, IOSubsystem, IOSubsystemExt, Resources, System,
};
use zk_ee::utils::{u256_to_usize_saturated, Bytes32};

pub const BUILDER_DEPOSIT_REQUEST_TYPE: u8 = 0x03;
pub const BUILDER_EXIT_REQUEST_TYPE: u8 = 0x04;

/// `0x0000bFF46984e3725691FA540a8C7589300D8282`
pub const BUILDER_DEPOSIT_CONTRACT_ADDRESS: B160 =
    B160::from_limbs([0x0a8c7589300d8282, 0x6984e3725691fa54, 0x0000bff4]);
/// `0x000064D678505ad48F8cCb093BC65613800E8282`
pub const BUILDER_EXIT_CONTRACT_ADDRESS: B160 =
    B160::from_limbs([0x3bc65613800e8282, 0x78505ad48f8ccb09, 0x000064d6]);

const SLOT_EXCESS: Bytes32 = Bytes32::ZERO;
const SLOT_COUNT: Bytes32 =
    Bytes32::from_hex("0000000000000000000000000000000000000000000000000000000000000001");
const SLOT_QUEUE_HEAD: Bytes32 =
    Bytes32::from_hex("0000000000000000000000000000000000000000000000000000000000000002");
const SLOT_QUEUE_TAIL: Bytes32 =
    Bytes32::from_hex("0000000000000000000000000000000000000000000000000000000000000003");
const QUEUE_OFFSET: U256 = U256::from_limbs([4, 0, 0, 0]);

/// The largest record: a builder deposit, in six slots
const MAX_SLOTS_PER_RECORD: usize = 6;

/// A request queue predeploy, as its system call sees it
pub struct RequestQueue {
    pub name: &'static str,
    pub address: B160,
    pub request_type: u8,
    pub max_per_block: usize,
    pub target_per_block: u64,
    pub slots_per_record: u64,
    /// Feeds the record bytes (the contract's return data for the record) to
    /// the requests hasher from the record's slots
    pub emit_record: fn(&[Bytes32], &mut dyn FnMut(&[u8])),
}

/// Builder deposit record: `pubkey (48) ++ withdrawal_credentials (32) ++
/// amount (8, little-endian) ++ signature (96)`, stored big-endian as
/// submitted across six slots: `pk[0:32]`, `pk[32:48] ++ wc[0:16]`,
/// `wc[16:32] ++ amount ++ sig[0:8]`, `sig[8:40]`, `sig[40:72]`, `sig[72:96]`
fn emit_deposit_record(slots: &[Bytes32], out: &mut dyn FnMut(&[u8])) {
    out(slots[0].as_u8_array_ref());
    out(slots[1].as_u8_array_ref());
    let third = slots[2].as_u8_array_ref();
    out(&third[..16]);
    // the amount is a big-endian `uint64` in storage, little-endian in the record
    let amount = u64::from_be_bytes(third[16..24].try_into().expect("8 bytes"));
    out(&amount.to_le_bytes());
    out(&third[24..]);
    out(slots[3].as_u8_array_ref());
    out(slots[4].as_u8_array_ref());
    out(&slots[5].as_u8_array_ref()[..24]);
}

/// Builder exit record: `source_address (20) ++ pubkey (48)`, stored across
/// three slots: the address (right-aligned), `pk[0:32]`, `pk[32:48]`
fn emit_exit_record(slots: &[Bytes32], out: &mut dyn FnMut(&[u8])) {
    out(&slots[0].as_u8_array_ref()[12..]);
    out(slots[1].as_u8_array_ref());
    out(&slots[2].as_u8_array_ref()[..16]);
}

pub const BUILDER_DEPOSIT_QUEUE: RequestQueue = RequestQueue {
    name: "EIP-8282 builder deposit",
    address: BUILDER_DEPOSIT_CONTRACT_ADDRESS,
    request_type: BUILDER_DEPOSIT_REQUEST_TYPE,
    max_per_block: 64,
    target_per_block: 8,
    slots_per_record: 6,
    emit_record: emit_deposit_record,
};

pub const BUILDER_EXIT_QUEUE: RequestQueue = RequestQueue {
    name: "EIP-8282 builder exit",
    address: BUILDER_EXIT_CONTRACT_ADDRESS,
    request_type: BUILDER_EXIT_REQUEST_TYPE,
    max_per_block: 16,
    target_per_block: 2,
    slots_per_record: 3,
    emit_record: emit_exit_record,
};

/// The system call of the builder deposit contract; `true` when it returned requests
pub fn eip8282_builder_deposits_system_part<S: EthereumLikeSystem>(
    system: &mut System<S>,
    requests_hasher: &mut impl crypto::sha256::Digest,
) -> Result<bool, SystemError>
where
    S::IO: IOSubsystemExt,
{
    drain_request_queue(system, requests_hasher, &BUILDER_DEPOSIT_QUEUE)
}

/// The system call of the builder exit contract; `true` when it returned requests
pub fn eip8282_builder_exits_system_part<S: EthereumLikeSystem>(
    system: &mut System<S>,
    requests_hasher: &mut impl crypto::sha256::Digest,
) -> Result<bool, SystemError>
where
    S::IO: IOSubsystemExt,
{
    drain_request_queue(system, requests_hasher, &BUILDER_EXIT_QUEUE)
}

fn read_slot<S: EthereumLikeSystem>(
    system: &mut System<S>,
    resources: &mut S::Resources,
    address: &B160,
    slot: &Bytes32,
) -> Result<Bytes32, SystemError>
where
    S::IO: IOSubsystemExt,
{
    resources.with_infinite_ergs(|resources| {
        system
            .io
            .storage_read::<false>(ExecutionEnvironmentType::NoEE, resources, address, slot)
    })
}

fn write_slot<S: EthereumLikeSystem>(
    system: &mut System<S>,
    resources: &mut S::Resources,
    address: &B160,
    slot: &Bytes32,
    value: &Bytes32,
) -> Result<(), SystemError>
where
    S::IO: IOSubsystemExt,
{
    resources.with_infinite_ergs(|resources| {
        system.io.storage_write::<false>(
            ExecutionEnvironmentType::NoEE,
            resources,
            address,
            slot,
            value,
        )
    })?;
    Ok(())
}

fn u256_of(value: &Bytes32) -> U256 {
    U256::from_be_bytes(value.as_u8_array())
}

fn bytes32_of(value: U256) -> Bytes32 {
    Bytes32::from_array(value.to_be_bytes::<32>())
}

/// The system call of a request queue contract (its `read_requests` path with
/// empty calldata), as the contract runs it
pub fn drain_request_queue<S: EthereumLikeSystem>(
    system: &mut System<S>,
    requests_hasher: &mut impl crypto::sha256::Digest,
    queue: &RequestQueue,
) -> Result<bool, SystemError>
where
    S::IO: IOSubsystemExt,
{
    let mut resources = S::Resources::from_native(
        <S::Resources as Resources>::Native::from_computational(u64::MAX),
    );
    let address = queue.address;

    let props = resources.with_infinite_ergs(|resources| {
        system.io.read_account_properties(
            ExecutionEnvironmentType::NoEE,
            resources,
            &address,
            AccountDataRequest::empty()
                .with_nonce()
                .with_observable_bytecode_len(),
        )
    })?;
    if props.nonce.0 != 1 || props.observable_bytecode_len.0 == 0 {
        return Err(SystemError::LeafDefect(internal_error!(
            "an EIP-8282 builder request contract is not deployed"
        )));
    }

    let tail = u256_of(&read_slot(
        system,
        &mut resources,
        &address,
        &SLOT_QUEUE_TAIL,
    )?);
    let head = u256_of(&read_slot(
        system,
        &mut resources,
        &address,
        &SLOT_QUEUE_HEAD,
    )?);
    // `tail - head` as the contract computes it (the queue keeps `head <= tail`)
    let in_queue = tail.wrapping_sub(head);
    let dequeued = u256_to_usize_saturated(&in_queue).min(queue.max_per_block);

    let mut logger = system.get_logger();
    logger_log!(
        logger,
        "{} system call: {dequeued} request(s) dequeued\n",
        queue.name
    );

    if dequeued > 0 {
        requests_hasher.update([queue.request_type]);
        let slots_per_record = U256::from(queue.slots_per_record);
        let mut slots = [Bytes32::ZERO; MAX_SLOTS_PER_RECORD];
        for i in 0..dequeued {
            let base = QUEUE_OFFSET + (head + U256::from(i as u64)) * slots_per_record;
            for (j, slot) in slots
                .iter_mut()
                .enumerate()
                .take(queue.slots_per_record as usize)
            {
                let key = bytes32_of(base + U256::from(j as u64));
                *slot = read_slot(system, &mut resources, &address, &key)?;
            }
            (queue.emit_record)(&slots[..queue.slots_per_record as usize], &mut |bytes| {
                requests_hasher.update(bytes)
            });
        }
    }

    // the head moves past the dequeued records; an emptied queue is reset
    let new_head = head + U256::from(dequeued as u64);
    if new_head == tail {
        write_slot(
            system,
            &mut resources,
            &address,
            &SLOT_QUEUE_HEAD,
            &Bytes32::ZERO,
        )?;
        write_slot(
            system,
            &mut resources,
            &address,
            &SLOT_QUEUE_TAIL,
            &Bytes32::ZERO,
        )?;
    } else {
        write_slot(
            system,
            &mut resources,
            &address,
            &SLOT_QUEUE_HEAD,
            &bytes32_of(new_head),
        )?;
    }

    // the excess: zero when the inhibitor is set, else what the block added
    // beyond the target on top of it; then the count is reset
    let excess = read_slot(system, &mut resources, &address, &SLOT_EXCESS)?;
    let count = u256_of(&read_slot(system, &mut resources, &address, &SLOT_COUNT)?);
    let new_excess = if excess == Bytes32::MAX {
        U256::ZERO
    } else {
        (u256_of(&excess) + count).saturating_sub(U256::from(queue.target_per_block))
    };
    write_slot(
        system,
        &mut resources,
        &address,
        &SLOT_EXCESS,
        &bytes32_of(new_excess),
    )?;
    write_slot(
        system,
        &mut resources,
        &address,
        &SLOT_COUNT,
        &Bytes32::ZERO,
    )?;

    Ok(dequeued > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn predeploy_addresses() {
        assert_eq!(
            format!("{:040x}", BUILDER_DEPOSIT_CONTRACT_ADDRESS.as_uint()),
            "0000bff46984e3725691fa540a8c7589300d8282"
        );
        assert_eq!(
            format!("{:040x}", BUILDER_EXIT_CONTRACT_ADDRESS.as_uint()),
            "000064d678505ad48f8ccb093bc65613800e8282"
        );
    }

    #[test]
    fn record_layouts() {
        let mut deposit = [Bytes32::ZERO; 6];
        for (i, slot) in deposit.iter_mut().enumerate() {
            *slot = Bytes32::from_array([i as u8 + 1; 32]);
        }
        // the amount, big-endian, in bytes 16..24 of the third slot
        deposit[2].as_u8_array_mut()[16..24].copy_from_slice(&0x0102030405060708u64.to_be_bytes());
        let mut record = Vec::new();
        emit_deposit_record(&deposit, &mut |bytes| record.extend_from_slice(bytes));
        assert_eq!(record.len(), 184);
        assert_eq!(
            &record[..48],
            &[[1u8; 32].as_slice(), &[2u8; 16]].concat()[..]
        );
        assert_eq!(
            &record[48..80],
            &[[2u8; 16].as_slice(), &[3u8; 16]].concat()[..]
        );
        assert_eq!(&record[80..88], &0x0102030405060708u64.to_le_bytes());
        assert_eq!(&record[88..96], &[3u8; 8]);
        assert_eq!(
            &record[96..160],
            &[[4u8; 32].as_slice(), &[5u8; 32]].concat()[..]
        );
        assert_eq!(&record[160..], &[6u8; 24]);

        let mut exit = [Bytes32::ZERO; 3];
        exit[0].as_u8_array_mut()[12..].copy_from_slice(&[0xaa; 20]);
        exit[1] = Bytes32::from_array([0xbb; 32]);
        exit[2].as_u8_array_mut()[..16].copy_from_slice(&[0xcc; 16]);
        let mut record = Vec::new();
        emit_exit_record(&exit, &mut |bytes| record.extend_from_slice(bytes));
        assert_eq!(record.len(), 68);
        assert_eq!(&record[..20], &[0xaa; 20]);
        assert_eq!(&record[20..52], &[0xbb; 32]);
        assert_eq!(&record[52..], &[0xcc; 16]);
    }
}
