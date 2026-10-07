//! State consistency of a Glamsterdam block: the parent state root is carried
//! to the header's state root by the block access list alone, with no
//! transaction executed.
//!
//! [`verify_state_consistency`]:
//! 1. witnesses the target header and the parent header, checks that the
//!    latter is the parent and that the fee fields follow from it;
//! 2. witnesses the RLP encoding of the block access list, checks it against
//!    the header's `block_access_list_hash`, and walks it lazily: every address and
//!    every slot key gets a dense index (an interner) and its trie key
//!    (keccak), the EIP-7928 ordering and uniqueness rules are checked, and
//!    the post-block value of every changed field is recorded in flat tables
//!    indexed by those indices ([`AccountAccess`], [`SlotAccess`]);
//! 3. applies the changes to the state with the stack MPT, sorted by trie
//!    key, recording the pre-block leaf of every account and slot in the
//!    access list as it comes out of the trie
//!    ([`StateConsistencyOutput::initial_accounts`],
//!    [`StateConsistencyOutput::initial_slots`]) and whether an account did not
//!    exist before the block ([`StateConsistencyOutput::fresh_accounts`]);
//! 4. compares the resulting root with the header's state root.
//!
//! The tables are sized from a counting pass over the access list, so nothing
//! grows afterwards (the proving-target allocator cannot reallocate).

pub mod access_list;
mod apply;

use super::block_header::{BlobSchedule, GlamsterdamHeader};
use super::oracle_queries::{
    ETHEREUM_BLOCK_ACCESS_LIST_BUFFER_DATA_QUERY_ID, ETHEREUM_BLOCK_ACCESS_LIST_BUFFER_LEN_QUERY_ID,
};
use super::{GlamsterdamError, EIP_7954_MAX_CODE_SIZE};
use crate::bootloader::block_flow::ethereum::oracle_queries::{
    ETHEREUM_HISTORICAL_HEADER_BUFFER_DATA_QUERY_ID,
    ETHEREUM_HISTORICAL_HEADER_BUFFER_LEN_QUERY_ID, ETHEREUM_TARGET_HEADER_BUFFER_DATA_QUERY_ID,
    ETHEREUM_TARGET_HEADER_BUFFER_LEN_QUERY_ID,
};
use access_list::{
    AccessListCounts, AccessListSource, AccountSource, BlockAccessListReader, SlotSource,
};
use alloc::vec::Vec;
use basic_system::system_implementation::ethereum_storage_model::interner::{
    AddressKey, Interner, SlotKey,
};

/// The keys of an access list are already distinct and sorted, so the
/// interners skip the last-key fast path
pub type AccessListInterner<K, A> = Interner<K, A, false>;

// The tables that get hashed are byte streams, `u32` offsets and `u32` mask
// words: the words must have the same byte image on every target, so both
// must be little-endian.
const _: () = assert!(cfg!(target_endian = "little"));

/// The bytes of a slice of densely packed elements
fn packed_bytes<T>(elements: &[T]) -> &[u8] {
    // SAFETY: the element types are asserted above to have no padding, so every
    // byte is initialized, and the slice covers the same allocation
    unsafe {
        core::slice::from_raw_parts(
            elements.as_ptr().cast::<u8>(),
            core::mem::size_of_val(elements),
        )
    }
}

/// The bytes of a `u32` word slice, little-endian
fn word_bytes_mut(words: &mut [u32]) -> &mut [u8] {
    // SAFETY: `u32` has no padding, the slice covers the same allocation, and the
    // borrow is exclusive
    unsafe {
        core::slice::from_raw_parts_mut(
            words.as_mut_ptr().cast::<u8>(),
            core::mem::size_of_val(words),
        )
    }
}
use alloc::boxed::Box;
use core::alloc::Allocator;
use crypto::sha3::Keccak256;
use crypto::MiniDigest;
use zk_ee::oracle::memory_io::get_bytes_with_length_query;
use zk_ee::oracle::IOOracle;
use zk_ee::utils::{Bytes32, UsizeAlignedByteBox};

/// The account is in the access list, so its leaf is read (set for every account)
pub const ACCOUNT_READ: u8 = 1 << 0;
pub const ACCOUNT_BALANCE_CHANGED: u8 = 1 << 1;
pub const ACCOUNT_NONCE_CHANGED: u8 = 1 << 2;
pub const ACCOUNT_CODE_CHANGED: u8 = 1 << 3;
/// Some slot of the account is written
pub const ACCOUNT_STORAGE_WRITTEN: u8 = 1 << 4;
/// Some slot of the account is read and not written
pub const ACCOUNT_STORAGE_READ: u8 = 1 << 5;

pub const SLOT_READ: u8 = 1 << 0;
pub const SLOT_WRITTEN: u8 = 1 << 1;

/// A slice of the access list encoding: an RLP item as the witness holds it
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EncodingSlice {
    pub offset: u32,
    pub len: u32,
}

impl EncodingSlice {
    fn of(encoding: &[u8], item: &[u8]) -> Self {
        // `item` is a subslice of `encoding`
        Self {
            offset: (item.as_ptr() as usize - encoding.as_ptr() as usize) as u32,
            len: item.len() as u32,
        }
    }

    /// The bytes, in the encoding they were taken from
    pub fn get<'e>(&self, encoding: &'e [u8]) -> &'e [u8] {
        &encoding[self.offset as usize..][..self.len as usize]
    }
}

/// What the access list says about one account, by address index
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AccountAccess {
    /// `ACCOUNT_*` flags
    pub flags: u8,
    /// The account's slots: `slots[slots_start..slots_end]`, in access list order
    /// (written slots by key, then read slots by key)
    pub slots_start: u32,
    pub slots_end: u32,
    /// Post-block values as the RLP items of the witness (minimal big-endian
    /// integers, as the account leaf stores them); meaningful when the
    /// matching flag is set
    pub post_nonce: EncodingSlice,
    pub post_balance: EncodingSlice,
    pub post_code_hash: Bytes32,
    /// The post-block code
    pub post_code: EncodingSlice,
}

/// What the access list says about one (account, slot key) pair
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SlotAccess {
    pub address_index: u32,
    pub key_index: u32,
    /// `SLOT_*` flags
    pub flags: u8,
    /// Post-block value as the RLP item of the witness (a minimal big-endian
    /// integer, as the storage leaf stores it); meaningful for written slots
    pub post_value: EncodingSlice,
}

/// Everything [`verify_state_consistency`] learned about the block
pub struct StateConsistencyOutput<A: Allocator> {
    pub header: GlamsterdamHeader,
    pub header_hash: Bytes32,
    pub parent_header: GlamsterdamHeader,
    pub parent_state_root: Bytes32,
    /// The RLP encoding of the block access list, as witnessed
    pub access_list_encoding: UsizeAlignedByteBox<A>,
    pub counts: AccessListCounts,

    /// Address -> address index, in access list (ascending address) order
    pub addresses: AccessListInterner<AddressKey, A>,
    /// `(keccak(address), address index)`, sorted by the hash: the order the
    /// accounts trie is walked in
    pub address_hashes: Vec<(Bytes32, u32), A>,
    /// Slot key -> key index, in order of first appearance
    pub slot_keys: AccessListInterner<SlotKey, A>,
    /// `(keccak(slot key), key index)`, by key index
    pub slot_key_hashes: Vec<(Bytes32, u32), A>,

    /// By address index
    pub accounts: Vec<AccountAccess, A>,
    /// In access list order, grouped by account (see [`AccountAccess`])
    pub slots: Vec<SlotAccess, A>,

    /// Pre-block leaves of the accounts, as entries `length byte || RLP item`
    /// of the leaf value (the list `[nonce, balance, storage_root, code_hash]`),
    /// a length of zero for an account that had no leaf. In trie order; see
    /// `initial_account_offsets`.
    pub initial_accounts: Vec<u8, A>,
    /// Offset of the entry of every account in `initial_accounts`, by address index
    pub initial_account_offsets: Vec<u32, A>,
    /// `1` where the account had no leaf before the block, by address index:
    /// one byte per account in `u32` words (little-endian), zero padded to
    /// the last word, so the mask has the same bytes on every target
    pub fresh_accounts: Box<[u32], A>,
    /// Pre-block leaves of the slots, as entries `length byte || RLP item` of
    /// the leaf value (the minimal big-endian integer), a length of zero for a
    /// slot without a leaf (a zero value). In trie order; see
    /// `initial_slot_offsets`.
    pub initial_slots: Vec<u8, A>,
    /// Offset of the entry of every slot in `initial_slots`, aligned with `slots`
    pub initial_slot_offsets: Vec<u32, A>,

    /// The state root after the access list is applied; equals the header's
    pub computed_state_root: Bytes32,
}

impl<A: Allocator> StateConsistencyOutput<A> {
    /// `initial_accounts` as the bytes that get hashed
    pub fn initial_accounts_bytes(&self) -> &[u8] {
        &self.initial_accounts
    }

    /// `initial_slots` as the bytes that get hashed
    pub fn initial_slots_bytes(&self) -> &[u8] {
        &self.initial_slots
    }

    /// `fresh_accounts` as bytes: one per account, then zero padding to a word
    pub fn fresh_accounts_bytes(&self) -> &[u8] {
        packed_bytes(&self.fresh_accounts)
    }

    /// Whether the account had no leaf before the block
    pub fn is_fresh(&self, address_index: u32) -> bool {
        self.fresh_accounts_bytes()[address_index as usize] != 0
    }

    /// The RLP item of the pre-block leaf of an account, `None` without a leaf
    pub fn initial_account(&self, address_index: u32) -> Option<&[u8]> {
        stream_entry(
            &self.initial_accounts,
            self.initial_account_offsets[address_index as usize],
        )
    }

    /// The RLP item of the pre-block leaf of the slot at `position` in `slots`,
    /// `None` without a leaf (a zero value)
    pub fn initial_slot(&self, position: u32) -> Option<&[u8]> {
        stream_entry(
            &self.initial_slots,
            self.initial_slot_offsets[position as usize],
        )
    }
}

/// The item of a `length byte || item` entry, `None` for a length of zero
fn stream_entry(stream: &[u8], offset: u32) -> Option<&[u8]> {
    let offset = offset as usize;
    let len = stream[offset] as usize;
    (len != 0).then(|| &stream[offset + 1..offset + 1 + len])
}

fn keccak(hasher: &mut Keccak256, data: &[u8]) -> Bytes32 {
    hasher.update(data);
    hasher.finalize_reset().into()
}

/// Witnesses the header, the parent header and the block access list, and
/// verifies that the access list carries the parent's state root to the
/// header's. See the module documentation.
pub fn verify_state_consistency<O: IOOracle, A: Allocator + Clone>(
    oracle: &mut O,
    blob_schedule: &BlobSchedule,
    max_block_gas_limit: u64,
    allocator: A,
) -> Result<StateConsistencyOutput<A>, GlamsterdamError> {
    let mut hasher = Keccak256::new();

    // the target header
    let header_encoding = get_bytes_with_length_query::<_, (), _>(
        oracle,
        ETHEREUM_TARGET_HEADER_BUFFER_LEN_QUERY_ID,
        ETHEREUM_TARGET_HEADER_BUFFER_DATA_QUERY_ID,
        (),
        allocator.clone(),
    )
    .map_err(|_| GlamsterdamError::Oracle)?
    .ok_or(GlamsterdamError::HeaderEncoding)?;
    let header_hash = keccak(&mut hasher, header_encoding.as_slice());
    let header = GlamsterdamHeader::parse(header_encoding.as_slice())
        .map_err(|_| GlamsterdamError::HeaderEncoding)?;
    drop(header_encoding);
    if header.base.gas_limit > max_block_gas_limit {
        return Err(GlamsterdamError::GasLimitTooHigh);
    }

    // the parent header, at history depth 0
    let parent_encoding = get_bytes_with_length_query::<_, u32, _>(
        oracle,
        ETHEREUM_HISTORICAL_HEADER_BUFFER_LEN_QUERY_ID,
        ETHEREUM_HISTORICAL_HEADER_BUFFER_DATA_QUERY_ID,
        0u32,
        allocator.clone(),
    )
    .map_err(|_| GlamsterdamError::Oracle)?
    .ok_or(GlamsterdamError::ParentHeaderEncoding)?;
    let parent_hash = keccak(&mut hasher, parent_encoding.as_slice());
    let parent_header = GlamsterdamHeader::parse(parent_encoding.as_slice())
        .map_err(|_| GlamsterdamError::ParentHeaderEncoding)?;
    drop(parent_encoding);
    header.check_against_parent(&parent_header, &parent_hash, blob_schedule)?;
    let parent_state_root = parent_header.base.state_root;

    // the block access list
    let access_list_encoding = get_bytes_with_length_query::<_, (), _>(
        oracle,
        ETHEREUM_BLOCK_ACCESS_LIST_BUFFER_LEN_QUERY_ID,
        ETHEREUM_BLOCK_ACCESS_LIST_BUFFER_DATA_QUERY_ID,
        (),
        allocator.clone(),
    )
    .map_err(|_| GlamsterdamError::Oracle)?
    .ok_or(GlamsterdamError::AccessListEncoding)?;
    let encoding = access_list_encoding.as_slice();
    if keccak(&mut hasher, encoding) != header.block_access_list_hash {
        return Err(GlamsterdamError::AccessListHash);
    }
    let counts = access_list::count(&mut BlockAccessListReader::new(encoding)?)?;
    let mut tables = Tables::with_counts(&counts, allocator.clone())?;
    collect(
        &mut BlockAccessListReader::new(encoding)?,
        encoding,
        &counts,
        &mut tables,
        &mut hasher,
    )?;

    // The streams take at most one longest entry per element; the offset tables
    // and the mask are written by index in the trie walk (see `apply`).
    let mut initial_accounts = Vec::with_capacity_in(
        counts.accounts * apply::MAX_ACCOUNT_ENTRY_LEN,
        allocator.clone(),
    );
    let mut initial_account_offsets = Vec::with_capacity_in(counts.accounts, allocator.clone());
    let mut initial_slots =
        Vec::with_capacity_in(counts.slots * apply::MAX_SLOT_ENTRY_LEN, allocator.clone());
    let mut initial_slot_offsets = Vec::with_capacity_in(counts.slots, allocator.clone());
    // one byte per account in zeroed words (the padding is hashed too)
    let mut fresh_accounts: Box<[u32], A> = {
        let zeroed = Box::new_zeroed_slice_in(counts.accounts.div_ceil(4), allocator.clone());
        // SAFETY: zero is a valid `u32`
        unsafe { zeroed.assume_init() }
    };

    let computed_state_root = apply::apply_access_list(
        oracle,
        &parent_state_root,
        encoding,
        &mut tables,
        apply::InitialValues {
            accounts: &mut initial_accounts,
            account_offsets: &mut initial_account_offsets,
            fresh_accounts: &mut word_bytes_mut(&mut fresh_accounts)[..counts.accounts],
            slots: &mut initial_slots,
            slot_offsets: &mut initial_slot_offsets,
        },
        counts.max_slots_of_account,
        &mut hasher,
        allocator,
    )?;
    if computed_state_root != header.base.state_root {
        return Err(GlamsterdamError::StateRootMismatch);
    }

    let Tables {
        addresses,
        address_hashes,
        slot_keys,
        slot_key_hashes,
        accounts,
        slots,
    } = tables;

    Ok(StateConsistencyOutput {
        header,
        header_hash,
        parent_header,
        parent_state_root,
        access_list_encoding,
        counts,
        addresses,
        address_hashes,
        slot_keys,
        slot_key_hashes,
        accounts,
        slots,
        initial_accounts,
        initial_account_offsets,
        fresh_accounts,
        initial_slots,
        initial_slot_offsets,
        computed_state_root,
    })
}

/// The tables filled by the access list walk
pub(super) struct Tables<A: Allocator> {
    pub addresses: AccessListInterner<AddressKey, A>,
    pub address_hashes: Vec<(Bytes32, u32), A>,
    pub slot_keys: AccessListInterner<SlotKey, A>,
    pub slot_key_hashes: Vec<(Bytes32, u32), A>,
    pub accounts: Vec<AccountAccess, A>,
    pub slots: Vec<SlotAccess, A>,
}

impl<A: Allocator + Clone> Tables<A> {
    fn with_counts(counts: &AccessListCounts, allocator: A) -> Result<Self, GlamsterdamError> {
        if counts.accounts >= u32::MAX as usize {
            return Err(GlamsterdamError::Capacity("accounts"));
        }
        if counts.slots >= u32::MAX as usize {
            return Err(GlamsterdamError::Capacity("slots"));
        }
        Ok(Self {
            addresses: Interner::new_in(counts.accounts, allocator.clone()),
            address_hashes: Vec::with_capacity_in(counts.accounts, allocator.clone()),
            // every slot entry may carry a distinct key
            slot_keys: Interner::new_in(counts.slots, allocator.clone()),
            slot_key_hashes: Vec::with_capacity_in(counts.slots, allocator.clone()),
            accounts: Vec::with_capacity_in(counts.accounts, allocator.clone()),
            slots: Vec::with_capacity_in(counts.slots, allocator),
        })
    }
}

/// Walks the access list, checking the EIP-7928 ordering and uniqueness rules,
/// interning the keys and filling the tables. `encoding` is the buffer the
/// source reads, which the code changes are located in.
fn collect<'a, A: Allocator + Clone, S: AccessListSource<'a>>(
    source: &mut S,
    encoding: &[u8],
    counts: &AccessListCounts,
    tables: &mut Tables<A>,
    hasher: &mut Keccak256,
) -> Result<(), GlamsterdamError> {
    use GlamsterdamError::AccessListOrdering;

    let mut previous_address = None;
    while let Some(mut account) = source.next_account()? {
        let address = account.address();
        let address_bytes = address.to_be_bytes::<20>();
        if let Some(previous) = previous_address {
            if address_bytes <= previous {
                return Err(AccessListOrdering("addresses not strictly ascending"));
            }
        }
        previous_address = Some(address_bytes);
        let address_index = tables
            .addresses
            .intern(&AddressKey(address))
            .map_err(|_| GlamsterdamError::Capacity("addresses"))?;
        // ascending addresses are distinct, so the index is the position
        if address_index as usize != tables.accounts.len() {
            return Err(AccessListOrdering("duplicate address"));
        }
        let address_hash = keccak(hasher, &address_bytes);
        tables.address_hashes.push((address_hash, address_index));

        let mut access = AccountAccess {
            flags: ACCOUNT_READ,
            slots_start: tables.slots.len() as u32,
            ..AccountAccess::default()
        };

        let mut previous_slot = None;
        while let Some(mut slot) = account.next_slot_changes()? {
            let slot_key = slot.slot();
            if let Some(previous) = previous_slot {
                if slot_key.as_u8_array_ref() <= &previous {
                    return Err(AccessListOrdering("written slots not strictly ascending"));
                }
            }
            previous_slot = Some(*slot_key.as_u8_array_ref());
            let key_index = intern_slot_key(tables, &slot_key, hasher)?;
            let mut previous_index = None;
            let mut post_value = None;
            while let Some((index, value)) = slot.next_change()? {
                if previous_index.is_some_and(|previous| index <= previous) {
                    return Err(AccessListOrdering("slot changes not strictly ascending"));
                }
                previous_index = Some(index);
                post_value = Some(value);
            }
            let Some(post_value) = post_value else {
                return Err(AccessListOrdering("slot without changes"));
            };
            tables.slots.push(SlotAccess {
                address_index,
                key_index,
                flags: SLOT_WRITTEN,
                post_value: EncodingSlice::of(encoding, post_value),
            });
            access.flags |= ACCOUNT_STORAGE_WRITTEN;
        }
        let written_end = tables.slots.len();

        // read slots: ascending, and none of them written (both lists are
        // ascending, so one pointer over the written ones suffices)
        let mut previous_slot = None;
        let mut written = access.slots_start as usize;
        while let Some(slot) = account.next_storage_read()? {
            if let Some(previous) = previous_slot {
                if slot.as_u8_array_ref() <= &previous {
                    return Err(AccessListOrdering("read slots not strictly ascending"));
                }
            }
            previous_slot = Some(*slot.as_u8_array_ref());
            while written < written_end
                && tables
                    .slot_keys
                    .key(tables.slots[written].key_index)
                    .0
                    .as_u8_array_ref()
                    < slot.as_u8_array_ref()
            {
                written += 1;
            }
            if written < written_end
                && tables.slot_keys.key(tables.slots[written].key_index).0 == slot
            {
                return Err(AccessListOrdering("slot both written and read"));
            }
            let key_index = intern_slot_key(tables, &slot, hasher)?;
            tables.slots.push(SlotAccess {
                address_index,
                key_index,
                flags: SLOT_READ,
                post_value: EncodingSlice::default(),
            });
            access.flags |= ACCOUNT_STORAGE_READ;
        }
        access.slots_end = tables.slots.len() as u32;

        let mut previous_index = None;
        while let Some((index, balance)) = account.next_balance_change()? {
            if previous_index.is_some_and(|previous| index <= previous) {
                return Err(AccessListOrdering("balance changes not strictly ascending"));
            }
            previous_index = Some(index);
            access.post_balance = EncodingSlice::of(encoding, balance);
            access.flags |= ACCOUNT_BALANCE_CHANGED;
        }
        let mut previous_index = None;
        while let Some((index, nonce)) = account.next_nonce_change()? {
            if previous_index.is_some_and(|previous| index <= previous) {
                return Err(AccessListOrdering("nonce changes not strictly ascending"));
            }
            previous_index = Some(index);
            access.post_nonce = EncodingSlice::of(encoding, nonce);
            access.flags |= ACCOUNT_NONCE_CHANGED;
        }
        let mut previous_index = None;
        while let Some((index, code)) = account.next_code_change()? {
            if previous_index.is_some_and(|previous| index <= previous) {
                return Err(AccessListOrdering("code changes not strictly ascending"));
            }
            previous_index = Some(index);
            if code.len() > EIP_7954_MAX_CODE_SIZE {
                return Err(GlamsterdamError::CodeSizeExceeded);
            }
            access.post_code_hash = keccak(hasher, code);
            access.post_code = EncodingSlice::of(encoding, code);
            access.flags |= ACCOUNT_CODE_CHANGED;
        }

        tables.accounts.push(access);
    }

    debug_assert_eq!(tables.accounts.len(), counts.accounts);
    debug_assert_eq!(tables.slots.len(), counts.slots);
    Ok(())
}

/// The index of a slot key, hashing it on first appearance
fn intern_slot_key<A: Allocator + Clone>(
    tables: &mut Tables<A>,
    slot: &Bytes32,
    hasher: &mut Keccak256,
) -> Result<u32, GlamsterdamError> {
    let key_index = tables
        .slot_keys
        .intern(&SlotKey(*slot))
        .map_err(|_| GlamsterdamError::Capacity("slot keys"))?;
    if key_index as usize == tables.slot_key_hashes.len() {
        let hash = keccak(hasher, slot.as_u8_array_ref());
        tables.slot_key_hashes.push((hash, key_index));
    }
    Ok(key_index)
}
