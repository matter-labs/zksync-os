//! State consistency of a Glamsterdam block: the parent state root is carried
//! to the header's state root by the block access list alone, with no
//! transaction executed.
//!
//! [`verify_state_consistency`]:
//! 1. witnesses the target header and the parent header, checks that the
//!    latter is the parent and that the fee fields follow from it; witnesses
//!    the number of transactions and proves it against the transactions and
//!    receipts tries ([`StateConsistencyOutput::transaction_count`], see
//!    `tx_range::index_trie` for what that proves);
//! 2. witnesses the RLP encoding of the block access list, checks it against
//!    the header's `block_access_list_hash`, validates it in full (the
//!    encoding, the EIP-7928 ordering and uniqueness rules, the code size
//!    limit) while counting its entries, then walks it again: every address
//!    and every slot key gets a dense index (an interner) and its trie key
//!    (keccak), and the post-block value of every changed field is recorded
//!    in flat tables indexed by those indices ([`AccountAccess`],
//!    [`SlotAccess`]);
//! 3. applies the changes to the state with the stack MPT, sorted by trie
//!    key, recording the pre-block leaf of every account and slot in the
//!    access list as it comes out of the trie
//!    ([`StateConsistencyOutput::initial_accounts`],
//!    [`StateConsistencyOutput::initial_slots`]) and whether an account did not
//!    exist before the block ([`StateConsistencyOutput::fresh_accounts`]);
//! 4. compares the resulting root with the header's state root;
//! 5. reorders the recorded leaves by address index and slot position: the
//!    three initial value tables, in that layout, are what the transaction
//!    ranges witness, linked to this check by their hashes
//!    ([`StateConsistencyOutput::initial_values_hashes`]).
//!
//! The tables are sized from the counts of the validating pass, so nothing
//! grows afterwards (the proving-target allocator cannot reallocate): the key
//! interners' hash tables are allocated for exactly the accounts and slots of
//! the list, which may not exceed what the header's gas limit lets a block
//! touch (`max_accounts_in_block`, `max_slots_in_block`).

pub mod access_list;
mod apply;

use super::block_header::{BlobSchedule, GlamsterdamHeader};
use super::index_mask::IndexMask;
use super::oracle_queries::{
    ETHEREUM_BLOCK_ACCESS_LIST_BUFFER_DATA_QUERY_ID, ETHEREUM_BLOCK_ACCESS_LIST_BUFFER_LEN_QUERY_ID,
};
use super::tx_range::witness::witness_proven_transaction_count;
use super::{max_accounts_in_block, max_slots_in_block, GlamsterdamError};
use crate::bootloader::block_flow::ethereum::oracle_queries::{
    ETHEREUM_HISTORICAL_HEADER_BUFFER_DATA_QUERY_ID,
    ETHEREUM_HISTORICAL_HEADER_BUFFER_LEN_QUERY_ID, ETHEREUM_TARGET_HEADER_BUFFER_DATA_QUERY_ID,
    ETHEREUM_TARGET_HEADER_BUFFER_LEN_QUERY_ID,
};
use access_list::{AccessListCounts, BlockAccessListReader, Validation};
use alloc::vec::Vec;
use basic_system::system_implementation::ethereum_storage_model::interner::{
    AddressKey, Interner, SlotKey,
};

/// The keys of an access list are already distinct and sorted, so the
/// interners skip the last-key fast path
pub type AccessListInterner<K, A> = Interner<K, A, false>;

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
    /// The number of transactions of the block: the last leaf of the
    /// transactions and receipts tries is at `transaction_count - 1`
    pub transaction_count: u32,
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
    /// a length of zero for an account that had no leaf, by address index
    pub initial_accounts: Vec<u8, A>,
    /// Offset of the entry of every account in `initial_accounts`, by address
    /// index. Derived from the stream (an entry follows the previous one), so
    /// not part of what the ranges witness.
    pub initial_account_offsets: Vec<u32, A>,
    /// The accounts that had no leaf before the block, by address index
    pub fresh_accounts: IndexMask<A>,
    /// Pre-block leaves of the slots, as entries `length byte || RLP item` of
    /// the leaf value (the minimal big-endian integer), a length of zero for a
    /// slot without a leaf (a zero value), by position in `slots`
    pub initial_slots: Vec<u8, A>,
    /// Offset of the entry of every slot in `initial_slots`, by position;
    /// derived like `initial_account_offsets`
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

    /// `fresh_accounts` as the bytes that get hashed
    pub fn fresh_accounts_bytes(&self) -> &[u8] {
        self.fresh_accounts.bytes()
    }

    /// The three initial value tables as the ranges witness them, in the
    /// order they witness them (`initial_accounts`, `fresh_accounts`,
    /// `initial_slots`): the preimages of [`Self::initial_values_hashes`],
    /// what a range's oracle serves (see `tx_range::witness::InitialValues`)
    pub fn initial_value_tables(&self) -> [&[u8]; 3] {
        [
            self.initial_accounts_bytes(),
            self.fresh_accounts_bytes(),
            self.initial_slots_bytes(),
        ]
    }

    /// keccak of the three initial value tables, in the order the ranges
    /// witness them: what links a range's tables to this check
    pub fn initial_values_hashes(&self) -> [Bytes32; 3] {
        let mut hasher = Keccak256::new();
        self.initial_value_tables()
            .map(|table| keccak(&mut hasher, table))
    }

    /// Whether the account had no leaf before the block
    pub fn is_fresh(&self, address_index: u32) -> bool {
        self.fresh_accounts.get(address_index)
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

/// Reorders a stream of `length byte || item` entries, written in trie order,
/// by index: `offsets[i]` is the entry of index `i`, every entry exactly once.
/// Afterwards the stream holds the entries in index order and `offsets[i]` is
/// the position of the entry of index `i` in it.
fn reorder_by_index<A: Allocator>(stream: &mut Vec<u8, A>, offsets: &mut [u32], allocator: A) {
    let mut ordered = Vec::with_capacity_in(stream.len(), allocator);
    for offset in offsets.iter_mut() {
        let at = *offset as usize;
        let len = stream[at] as usize;
        *offset = ordered.len() as u32;
        ordered.extend_from_slice(&stream[at..at + 1 + len]);
    }
    debug_assert_eq!(ordered.len(), stream.len());
    *stream = ordered;
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

    // the number of transactions, against both index tries
    let transaction_count = witness_proven_transaction_count(
        oracle,
        &header.base.transactions_root,
        &header.base.receipts_root,
        allocator.clone(),
    )?;

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
    let counts = access_list::validate(encoding, Validation::Full)?;
    // the post-transaction phase is the last index a change can be at
    if counts.max_access_index > transaction_count + 1 {
        return Err(GlamsterdamError::AccessListValue);
    }
    // No more entries than the block's gas can touch: every table below (the key
    // interners' hash tables included) is allocated once at the exact count, so this
    // bounds the memory a block takes before anything is allocated.
    if counts.accounts > max_accounts_in_block(header.base.gas_limit) {
        return Err(GlamsterdamError::Capacity(
            "more accounts than the gas limit allows",
        ));
    }
    if counts.slots > max_slots_in_block(header.base.gas_limit) {
        return Err(GlamsterdamError::Capacity(
            "more slots than the gas limit allows",
        ));
    }
    let mut tables = Tables::with_counts(&counts, allocator.clone())?;
    collect_tables(encoding, &counts, &mut tables, &mut hasher)?;

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
    let mut fresh_accounts = IndexMask::zeroed(counts.accounts, allocator.clone())?;

    let computed_state_root = apply::apply_access_list(
        oracle,
        &parent_state_root,
        encoding,
        &mut tables,
        apply::InitialValues {
            accounts: &mut initial_accounts,
            account_offsets: &mut initial_account_offsets,
            fresh_accounts: &mut fresh_accounts,
            slots: &mut initial_slots,
            slot_offsets: &mut initial_slot_offsets,
        },
        counts.max_slots_of_account,
        &mut hasher,
        allocator.clone(),
    )?;
    if computed_state_root != header.base.state_root {
        return Err(GlamsterdamError::StateRootMismatch);
    }

    // the layout the ranges witness: by index, so no offsets travel with the streams
    reorder_by_index(
        &mut initial_accounts,
        &mut initial_account_offsets,
        allocator.clone(),
    );
    reorder_by_index(&mut initial_slots, &mut initial_slot_offsets, allocator);

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
        transaction_count,
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

/// How many times the number of keys the key interners' hash tables are sized for:
/// allocated once (the proving-target allocator cannot grow them), at a low load so
/// that lookups rarely probe past the first group. With the hash table reserving for
/// its own load factor on top, the largest block (about 66.6k accounts and 95.1k
/// slots at 200M gas, see `max_accounts_in_block`) takes 2^18 buckets per table.
pub const INTERNER_CAPACITY_FACTOR: usize = 2;

/// The tables filled by the access list walk
pub struct Tables<A: Allocator> {
    pub addresses: AccessListInterner<AddressKey, A>,
    pub address_hashes: Vec<(Bytes32, u32), A>,
    pub slot_keys: AccessListInterner<SlotKey, A>,
    pub slot_key_hashes: Vec<(Bytes32, u32), A>,
    pub accounts: Vec<AccountAccess, A>,
    pub slots: Vec<SlotAccess, A>,
}

impl<A: Allocator + Clone> Tables<A> {
    pub fn with_counts(counts: &AccessListCounts, allocator: A) -> Result<Self, GlamsterdamError> {
        if counts.accounts >= u32::MAX as usize / INTERNER_CAPACITY_FACTOR {
            return Err(GlamsterdamError::Capacity("accounts"));
        }
        if counts.slots >= u32::MAX as usize / INTERNER_CAPACITY_FACTOR {
            return Err(GlamsterdamError::Capacity("slots"));
        }
        Ok(Self {
            addresses: Interner::new_in(
                counts.accounts * INTERNER_CAPACITY_FACTOR,
                allocator.clone(),
            ),
            address_hashes: Vec::with_capacity_in(counts.accounts, allocator.clone()),
            // every slot entry may carry a distinct key
            slot_keys: Interner::new_in(counts.slots * INTERNER_CAPACITY_FACTOR, allocator.clone()),
            slot_key_hashes: Vec::with_capacity_in(counts.slots, allocator.clone()),
            accounts: Vec::with_capacity_in(counts.accounts, allocator.clone()),
            slots: Vec::with_capacity_in(counts.slots, allocator),
        })
    }
}

/// Walks a validated access list (see [`access_list::validate`]), interning
/// the keys and filling the tables; `counts` are the counts of the validation.
/// The ordering rules are not checked again; the index of every address is its
/// position, which is checked as the interner reports a repeated address.
pub fn collect_tables<A: Allocator + Clone>(
    encoding: &[u8],
    counts: &AccessListCounts,
    tables: &mut Tables<A>,
    hasher: &mut Keccak256,
) -> Result<(), GlamsterdamError> {
    let mut reader = BlockAccessListReader::new(encoding)?;
    while let Some(mut account) = reader.next_account()? {
        let address_bytes = account.address.to_be_bytes::<20>();
        let address_index = tables
            .addresses
            .intern(&AddressKey(account.address))
            .map_err(|_| GlamsterdamError::Capacity("addresses"))?;
        if address_index as usize != tables.accounts.len() {
            return Err(GlamsterdamError::AccessListOrdering("duplicate address"));
        }
        let address_hash = keccak(hasher, &address_bytes);
        tables.address_hashes.push((address_hash, address_index));

        let mut access = AccountAccess {
            flags: ACCOUNT_READ,
            slots_start: tables.slots.len() as u32,
            ..AccountAccess::default()
        };

        while let Some(mut slot) = account.next_slot_changes()? {
            let key_index = intern_slot_key(tables, &slot.slot, hasher)?;
            // the changes are ascending in the index: the last one is the post-block value
            let mut post_value = None;
            while let Some((_, value)) = slot.next_change()? {
                post_value = Some(value);
            }
            let Some(post_value) = post_value else {
                return Err(GlamsterdamError::AccessListOrdering("slot without changes"));
            };
            tables.slots.push(SlotAccess {
                address_index,
                key_index,
                flags: SLOT_WRITTEN,
                post_value: EncodingSlice::of(encoding, post_value),
            });
            access.flags |= ACCOUNT_STORAGE_WRITTEN;
        }
        while let Some(slot) = account.next_storage_read()? {
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

        while let Some((_, balance)) = account.next_balance_change()? {
            access.post_balance = EncodingSlice::of(encoding, balance);
            access.flags |= ACCOUNT_BALANCE_CHANGED;
        }
        while let Some((_, nonce)) = account.next_nonce_change()? {
            access.post_nonce = EncodingSlice::of(encoding, nonce);
            access.flags |= ACCOUNT_NONCE_CHANGED;
        }
        while let Some((_, code)) = account.next_code_change()? {
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
