//! The storage model of a transaction range: every account and slot the block
//! touches is known up front from the block access list, with its pre-block
//! leaf from the pre-execution tables, so there is no cache to fill from the
//! oracle. The model keeps, per account and per slot, the state at the start
//! of the running access index and a chain of versions pushed by the index's
//! accesses and writes into append-only arenas; a frame rollback walks the
//! versions the frame pushed and cuts the arenas back, and the end of every
//! index checks that the head versions are exactly the changes the access
//! list declares for it (a symmetric difference of zero), after which they
//! become the next index's starting state and the arenas are dropped.
//!
//! Gas: the account accesses and writes are priced by EIP-8038 and the state
//! creation by EIP-8037 (state gas, see `StateGasResources`); every account
//! and slot is cold at the start of an index. The slots go through the
//! `StorageAccessPolicy` (`GlamsterdamStorageAccessCostModel`); bytecodes
//! still come from the preimage queries.

use super::super::index_mask::IndexMask;
use super::super::state_consistency::access_list::BlockAccessListReader;
use super::super::state_consistency::access_list::{validate, AccessListCounts, Validation};
use super::super::state_consistency::{collect_tables, Tables, SLOT_WRITTEN};
use super::super::GlamsterdamError;
use super::witness::{BlockAccessIndexRange, InitialValues};
use crate::bootloader::transaction::rlp_encoded::rlp::minimal_rlp_parser::Rlp;
use alloc::vec::Vec;
use basic_system::cost_constants::blake2s_native_cost;
use basic_system::system_functions::keccak256::keccak256_native_cost;
use basic_system::system_implementation::caches::storage_access_policy::StorageAccessPolicy;
use basic_system::system_implementation::ethereum_storage_model::caches::preimage::{
    BytecodeKeccakPreimagesStorage, PreimageRequestForUnknownLength,
};
use basic_system::system_implementation::ethereum_storage_model::caches::EMPTY_STRING_KECCAK_HASH;
use basic_system::system_implementation::ethereum_storage_model::interner::{AddressKey, SlotKey};
use basic_system::system_implementation::flat_storage_model::cost_constants::{
    KNOWN_TO_BE_WARM_PROPERTIES_ACCESS_COST_GAS, WARM_ACCOUNT_CACHE_ACCESS_NATIVE_COST,
    WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST,
};
use core::alloc::Allocator;
use crypto::sha3::Keccak256;
use crypto::MiniDigest;
use evm_interpreter::v3::gas::{
    ACCOUNT_WRITE, COLD_ACCOUNT_ACCESS, COLD_STORAGE_ACCESS, NEW_ACCOUNT, WARM_ACCESS,
};
use ruint::aliases::{B160, U256};
use storage_models::common_structs::snapshottable_io::SnapshottableIo;
use storage_models::common_structs::{PreimageCacheModel, StorageModel};
use zk_ee::common_structs::{PreimageType, WarmStorageKey};
use zk_ee::execution_environment_type::ExecutionEnvironmentType;
use zk_ee::memory::stack_trait::Stack;
use zk_ee::oracle::IOOracle;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::system::errors::system::SystemError;
use zk_ee::system::{
    AccountData, AccountDataRequest, BalanceError, BalanceSubsystemError, Computational,
    DeconstructionSubsystemError, IOResultKeeper, Maybe, NonceError, NonceSubsystemError,
    StateGasResources, StorageDiff, EIP7702_DELEGATION_MARKER,
};
use zk_ee::types_config::EthereumIOTypesConfig;
use zk_ee::utils::{Bytes32, UsizeAlignedByteBox};
use zk_ee::{interface_error, internal_error, wrap_error};

type Result<T, E = SystemError> = core::result::Result<T, E>;

/// An account at the start of the current access index, by address index: what
/// every index starts from. The state of the running index lives in versions
/// ([`AccountVersion`]) chained from `head`; nothing here is ever rolled back.
#[derive(Clone, Copy, Debug)]
struct AccountBase {
    nonce: u64,
    balance: U256,
    /// Never zero: the hash of the empty string for an account without code
    code_hash: Bytes32,
    /// One past the index of the latest version of the running index, zero when the
    /// account is untouched in it (so cold)
    head: u32,
    /// Accessed at some point of the range, in a reverted frame or not: the
    /// aggregation checks that the access list is exact
    ever_accessed: bool,
}

/// A value an account took in the running index: pushed by every change (and by
/// the first access, which copies the base), chained to the one before through
/// `previous`, so that a frame rollback is a walk over the versions it pushed
#[derive(Clone, Copy, Debug)]
struct AccountVersion {
    nonce: u64,
    balance: U256,
    code_hash: Bytes32,
    /// The account this is a version of
    account: u32,
    /// The head before this version (one past its index, zero for none)
    previous: u32,
    /// Code was deployed in this index (EIP-6780)
    deployed: bool,
    /// Self-destructed in this index
    deconstruct: bool,
}

/// A slot at the start of the current access index, by slot position; see
/// [`AccountBase`]
#[derive(Clone, Copy, Debug)]
struct SlotBase {
    start: Bytes32,
    /// One past the index of the latest version of the running index, zero when
    /// the slot is untouched in it (so cold)
    head: u32,
    /// Had no leaf before the block (reads of it are "new slot" reads)
    is_new: bool,
    /// A cold write extra was charged, and not rolled back, in an earlier index of
    /// the range: taken from the head version when an index ends, so a charge a frame
    /// rolled back never reaches it (within the index the flag lives in the versions)
    write_charged_before: bool,
    /// Accessed at some point of the range
    ever_accessed: bool,
}

/// A value a slot took in the running index; see [`AccountVersion`]
#[derive(Clone, Copy, Debug)]
struct SlotVersion {
    value: Bytes32,
    position: u32,
    previous: u32,
    /// A cold write extra was charged in this index at or before this version
    write_charged: bool,
}

/// The current values of an account, from its head version or its base
#[derive(Clone, Copy, Debug)]
struct AccountView {
    nonce: u64,
    balance: U256,
    code_hash: Bytes32,
    deployed: bool,
    deconstruct: bool,
}

/// Elements per page of a [`Versions`] arena
const VERSIONS_PAGE: usize = 128;
/// Pages a [`Versions`] arena can hold: with the transaction gas cap, an index
/// pushes at most about 160k slot versions (a warm `SSTORE` costs 106 gas) and
/// far fewer account versions
const VERSIONS_MAX_PAGES: usize = 2048;

/// An append-only arena of versions with random access: pages of a fixed size,
/// allocated as the arena grows and kept for the next index, never moved (the
/// proving-target allocator cannot reallocate). A frame rollback truncates it.
struct Versions<T: Copy, A: Allocator + Clone> {
    pages: Vec<Vec<T, A>, A>,
    len: usize,
    allocator: A,
}

impl<T: Copy, A: Allocator + Clone> Versions<T, A> {
    fn new_in(allocator: A) -> Self {
        Self {
            pages: Vec::with_capacity_in(VERSIONS_MAX_PAGES, allocator.clone()),
            len: 0,
            allocator,
        }
    }

    #[inline(always)]
    fn len(&self) -> u32 {
        self.len as u32
    }

    /// Appends a version and returns one past its index: the new head
    #[inline(always)]
    fn push(&mut self, version: T) -> Result<u32, InternalError> {
        let page = self.len / VERSIONS_PAGE;
        if page == self.pages.len() {
            if page == VERSIONS_MAX_PAGES {
                return Err(internal_error!(
                    "too many state versions in one access index"
                ));
            }
            self.pages
                .push(Vec::with_capacity_in(VERSIONS_PAGE, self.allocator.clone()));
        }
        let page = &mut self.pages[page];
        // a truncated page is reused from its truncated length
        debug_assert_eq!(page.len(), self.len % VERSIONS_PAGE);
        page.push(version);
        self.len += 1;
        Ok(self.len as u32)
    }

    /// The version at `index` (which must be below the length)
    #[inline(always)]
    fn get(&self, index: u32) -> &T {
        let index = index as usize;
        &self.pages[index / VERSIONS_PAGE][index % VERSIONS_PAGE]
    }

    /// Drops the versions from `len` on
    fn truncate(&mut self, len: u32) {
        let len = len as usize;
        if len >= self.len {
            return;
        }
        let first_page = len / VERSIONS_PAGE;
        let last_page = (self.len - 1) / VERSIONS_PAGE;
        self.pages[first_page].truncate(len % VERSIONS_PAGE);
        for page in &mut self.pages[first_page + 1..=last_page] {
            page.clear();
        }
        self.len = len;
    }

    fn clear(&mut self) {
        self.truncate(0);
    }
}

/// One change the access list declares
#[derive(Clone, Copy, Debug)]
enum Change {
    Balance { account: u32, value: U256 },
    Nonce { account: u32, value: u64 },
    Code { account: u32, hash: Bytes32 },
    Slot { position: u32, value: Bytes32 },
}

/// A declared change with its block access index
#[derive(Clone, Copy, Debug)]
struct IndexedChange {
    index: u32,
    change: Change,
}

/// A frame snapshot: the lengths of the version arenas and the refund counter
pub struct BalModelSnapshot<R> {
    account_versions: u32,
    slot_versions: u32,
    refund: R,
}

pub struct BalStorageModel<
    A: Allocator + Clone,
    R: StateGasResources,
    P: StorageAccessPolicy<R, Bytes32>,
    const PROOF_ENV: bool,
> {
    encoding: UsizeAlignedByteBox<A>,
    /// keccak of the access list encoding, to check against the header
    pub access_list_hash: Bytes32,
    tables: Tables<A>,
    /// Per account: the end of the written run inside its slot range
    written_ends: Vec<u32, A>,
    pub initial: InitialValues<A>,
    accounts: Vec<AccountBase, A>,
    slots: Vec<SlotBase, A>,
    /// The versions of the running index, for frame rollbacks and the end-of-index
    /// check; dropped at the end of the index
    account_versions: Versions<AccountVersion, A>,
    slot_versions: Versions<SlotVersion, A>,
    /// The declared changes, sorted by access index
    counts: AccessListCounts,
    changes: Vec<IndexedChange, A>,
    /// First change of every index, with a final end
    change_starts: Vec<u32, A>,
    pub range: BlockAccessIndexRange,
    /// The access index being executed
    current_index: u32,
    allocator: A,
    refund_counter: R,
    preimages: BytecodeKeccakPreimagesStorage<R, A>,
    policy: P,
}

/// The fields of an account leaf item
fn parse_account_leaf(item: &[u8]) -> Result<(u64, U256, Bytes32), GlamsterdamError> {
    let error = GlamsterdamError::AccountEncoding;
    let mut outer = Rlp::new(item);
    let mut r = outer.list().map_err(|_| error)?;
    let nonce = r.u64().map_err(|_| error)?;
    let balance = r.u256().map_err(|_| error)?;
    let _storage_root = r.bytes_exact::<32>().map_err(|_| error)?;
    let code_hash = r.bytes_exact::<32>().map_err(|_| error)?;
    if !r.is_empty() || !outer.is_empty() {
        return Err(error);
    }
    Ok((nonce, balance, Bytes32::from_array(*code_hash)))
}

/// A minimal big-endian integer item as a 32-byte value
fn integer_item_value(item: &[u8]) -> Result<Bytes32, GlamsterdamError> {
    let mut r = Rlp::new(item);
    let digits = r.bytes().map_err(|_| GlamsterdamError::SlotEncoding)?;
    if !r.is_empty() || digits.len() > 32 || digits.first() == Some(&0) {
        return Err(GlamsterdamError::SlotEncoding);
    }
    let mut value = Bytes32::ZERO;
    value.as_u8_array_mut()[32 - digits.len()..].copy_from_slice(digits);
    Ok(value)
}

/// The error of an access outside the list: the address, the slot key (for a slot),
/// and the access index the model is at (`u32::MAX` when the address itself is unknown)
fn not_in_access_list_at(address: &B160, key: Option<&Bytes32>, index: u32) -> SystemError {
    use zk_ee::error_ctx;
    use zk_ee::system::errors::context::contextualized::Contextualized as _;
    let error: InternalError = internal_error!("access outside of the block access list");
    let address = *address;
    let key = key.copied();
    let error: InternalError = error.with_context(|| {
        error_ctx! { "address" => debug_format(address), "slot" => debug_format(key), "index" => debug_format(index) }
    });
    error.into()
}

impl<
        A: Allocator + Clone,
        R: StateGasResources,
        P: StorageAccessPolicy<R, Bytes32>,
        const PROOF_ENV: bool,
    > BalStorageModel<A, R, P, PROOF_ENV>
{
    /// Builds the model from the witnessed access list encoding, the initial
    /// tables and the range: the tables of the walk, the current state at the
    /// start of the range (the declared changes before it applied), and the
    /// declared changes by index.
    pub fn new(
        encoding: UsizeAlignedByteBox<A>,
        initial: InitialValues<A>,
        range: BlockAccessIndexRange,
        policy: P,
        allocator: A,
    ) -> Result<Self, GlamsterdamError> {
        let mut hasher = Keccak256::new();
        hasher.update(encoding.as_slice());
        let access_list_hash: Bytes32 = hasher.finalize_reset().into();

        let bytes = encoding.as_slice();
        // the pre-execution check validated the list in full (the hash links them)
        let counts = validate(bytes, Validation::Changes)?;
        let mut tables = Tables::with_counts(&counts, allocator.clone())?;
        collect_tables(bytes, &counts, &mut tables, &mut hasher)?;
        if initial.num_accounts() != counts.accounts || initial.num_slots() != counts.slots {
            return Err(GlamsterdamError::InitialValuesEncoding);
        }

        // the written run of every account
        let mut written_ends = Vec::with_capacity_in(counts.accounts, allocator.clone());
        for access in tables.accounts.iter() {
            let mut end = access.slots_start;
            while end < access.slots_end && tables.slots[end as usize].flags & SLOT_WRITTEN != 0 {
                end += 1;
            }
            written_ends.push(end);
        }

        // the state before the block
        let mut accounts = Vec::with_capacity_in(counts.accounts, allocator.clone());
        for item in initial.accounts() {
            let (nonce, balance, code_hash) = match item {
                Some(item) => parse_account_leaf(item)?,
                None => (0, U256::ZERO, EMPTY_STRING_KECCAK_HASH),
            };
            accounts.push(AccountBase {
                nonce,
                balance,
                code_hash: if code_hash.is_zero() {
                    EMPTY_STRING_KECCAK_HASH
                } else {
                    code_hash
                },
                head: 0,
                ever_accessed: false,
            });
        }
        let mut slots = Vec::with_capacity_in(counts.slots, allocator.clone());
        for item in initial.slots() {
            let value = match item {
                Some(item) => integer_item_value(item)?,
                None => Bytes32::ZERO,
            };
            slots.push(SlotBase {
                start: value,
                head: 0,
                is_new: item.is_none(),
                write_charged_before: false,
                ever_accessed: false,
            });
        }

        // the declared changes up to the end of the range, in access list
        // order and then sorted by index (a (field, index) pair is unique, so
        // the order within an index is immaterial)
        let mut changes = Vec::with_capacity_in(counts.changes, allocator.clone());
        {
            let mut reader = BlockAccessListReader::new(bytes)?;
            let mut address_index = 0u32;
            while let Some(mut account) = reader.next_account()? {
                let access = tables.accounts[address_index as usize];
                let mut position = access.slots_start;
                while let Some(mut slot) = account.next_slot_changes()? {
                    while let Some((index, item)) = slot.next_change()? {
                        if index <= range.last {
                            changes.push(IndexedChange {
                                index,
                                change: Change::Slot {
                                    position,
                                    value: integer_item_value(item)?,
                                },
                            });
                        }
                    }
                    position += 1;
                }
                while account.next_storage_read()?.is_some() {}
                while let Some((index, item)) = account.next_balance_change()? {
                    if index <= range.last {
                        let value = integer_item_value(item)?.into_u256_be();
                        changes.push(IndexedChange {
                            index,
                            change: Change::Balance {
                                account: address_index,
                                value,
                            },
                        });
                    }
                }
                while let Some((index, item)) = account.next_nonce_change()? {
                    if index <= range.last {
                        let mut r = Rlp::new(item);
                        let value = r.u64().map_err(|_| GlamsterdamError::AccessListEncoding)?;
                        changes.push(IndexedChange {
                            index,
                            change: Change::Nonce {
                                account: address_index,
                                value,
                            },
                        });
                    }
                }
                while let Some((index, code)) = account.next_code_change()? {
                    if index <= range.last {
                        hasher.update(code);
                        let hash: Bytes32 = hasher.finalize_reset().into();
                        changes.push(IndexedChange {
                            index,
                            change: Change::Code {
                                account: address_index,
                                hash,
                            },
                        });
                    }
                }
                address_index += 1;
            }
        }
        changes.sort_unstable_by_key(|change| change.index);
        // changes of index i are `changes[change_starts[i]..change_starts[i + 1]]`
        let mut change_starts = Vec::with_capacity_in(range.last as usize + 2, allocator.clone());
        let mut next = 0usize;
        for index in 0..=range.last {
            change_starts.push(next as u32);
            while next < changes.len() && changes[next].index == index {
                next += 1;
            }
        }
        change_starts.push(next as u32);
        debug_assert_eq!(next, changes.len());

        let mut new = Self {
            encoding,
            access_list_hash,
            tables,
            written_ends,
            initial,
            accounts,
            slots,
            account_versions: Versions::new_in(allocator.clone()),
            slot_versions: Versions::new_in(allocator.clone()),
            counts,
            changes,
            change_starts,
            range,
            current_index: range.first,
            allocator: allocator.clone(),
            refund_counter: R::empty(),
            preimages: BytecodeKeccakPreimagesStorage::new_from_parts(allocator),
            policy,
        };
        // the state at the start of the range
        for change in new.changes[..new.change_starts[range.first as usize] as usize].iter() {
            match change.change {
                Change::Balance { account, value } => {
                    new.accounts[account as usize].balance = value;
                }
                Change::Nonce { account, value } => {
                    new.accounts[account as usize].nonce = value;
                }
                Change::Code { account, hash } => {
                    new.accounts[account as usize].code_hash = hash;
                }
                Change::Slot { position, value } => {
                    new.slots[position as usize].start = value;
                }
            }
        }
        Ok(new)
    }

    /// The access list encoding, for the code changes' bytes
    pub fn encoding(&self) -> &[u8] {
        self.encoding.as_slice()
    }

    pub fn current_index(&self) -> u32 {
        self.current_index
    }

    /// The largest block access index the access list declares a change at;
    /// the caller checks it against the transaction count, as the post-block
    /// values are the changes at the largest indices
    pub fn max_access_index(&self) -> u32 {
        self.counts.max_access_index
    }

    /// Which records of the access list the range accessed, reverted frames
    /// included: the accounts by address index, then the slots by position;
    /// the aggregation of the ranges checks that every record is accessed
    /// somewhere in the block
    pub fn accessed_mask(&self) -> Result<IndexMask<A>, GlamsterdamError> {
        let mut mask = IndexMask::zeroed(
            self.accounts.len() + self.slots.len(),
            self.allocator.clone(),
        )?;
        let flags = self
            .accounts
            .iter()
            .map(|a| a.ever_accessed)
            .chain(self.slots.iter().map(|s| s.ever_accessed));
        for (index, flag) in flags.enumerate() {
            if flag {
                mask.set(index as u32);
            }
        }
        Ok(mask)
    }

    /// Starts an access index: the writes that follow are checked against
    /// the changes declared for it at `finish_tx`
    pub fn begin_index(&mut self, index: u32) -> Result<(), InternalError> {
        if index < self.range.first || index > self.range.last {
            return Err(internal_error!("access index outside of the range"));
        }
        self.current_index = index;
        self.account_versions.clear();
        self.slot_versions.clear();
        self.refund_counter = R::empty();
        Ok(())
    }

    // --- lookups -----------------------------------------------------------

    fn account_index(&self, address: &B160) -> Result<u32> {
        self.tables
            .addresses
            .get(&AddressKey(*address))
            .ok_or_else(|| not_in_access_list_at(address, None, self.current_index))
    }

    /// The index of an account the execution accesses. An address outside the access list
    /// was never accessed by the real execution: it charges the cold access first, so that
    /// an access the real execution could not afford fails the same way (out of gas,
    /// before the access list is consulted), while an affordable one is a missing entry.
    fn account_index_charging(
        &self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &B160,
    ) -> Result<u32> {
        match self.tables.addresses.get(&AddressKey(*address)) {
            Some(account) => Ok(account),
            None => {
                if ee_type == ExecutionEnvironmentType::EVM
                    && !evm_interpreter::utils::is_precompile(address)
                {
                    resources.charge_legacy_gas(COLD_ACCOUNT_ACCESS)?;
                }
                Err(not_in_access_list_at(address, None, self.current_index))
            }
        }
    }

    /// The position of a slot the execution accesses, charging the cold access when the
    /// slot is outside the access list (see `account_index_charging`)
    fn slot_position_charging(
        &self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        account: u32,
        key: &Bytes32,
    ) -> Result<u32> {
        match self.slot_position(account, key) {
            Ok(position) => Ok(position),
            Err(e) => {
                if ee_type == ExecutionEnvironmentType::EVM {
                    resources.charge_legacy_gas(COLD_STORAGE_ACCESS)?;
                }
                Err(e)
            }
        }
    }

    /// The position of a slot of an account in the slot table
    fn slot_position(&self, account: u32, key: &Bytes32) -> Result<u32> {
        let key_index = self.tables.slot_keys.get(&SlotKey(*key)).ok_or_else(|| {
            not_in_access_list_at(
                &self.tables.addresses.key(account).0,
                Some(key),
                self.current_index,
            )
        })?;
        let access = &self.tables.accounts[account as usize];
        let written_end = self.written_ends[account as usize];
        let slot_keys = &self.tables.slot_keys;
        let slots = &self.tables.slots;
        let key_bytes = key.as_u8_array_ref();
        for run in [
            access.slots_start..written_end,
            written_end..access.slots_end,
        ] {
            let (mut lo, mut hi) = (run.start as usize, run.end as usize);
            while lo < hi {
                let mid = (lo + hi) / 2;
                let mid_key = slot_keys.key(slots[mid].key_index).0;
                match mid_key.as_u8_array_ref().cmp(key_bytes) {
                    core::cmp::Ordering::Less => lo = mid + 1,
                    core::cmp::Ordering::Greater => hi = mid,
                    core::cmp::Ordering::Equal => return Ok(mid as u32),
                }
            }
            // a slot key index shared with another account is not this account's slot
            let _ = key_index;
        }
        Err(not_in_access_list_at(
            &self.tables.addresses.key(account).0,
            Some(key),
            self.current_index,
        ))
    }

    // --- warm/cold accounting ---------------------------------------------

    /// Charges an account access (EIP-8038: `WARM_ACCESS`, or `COLD_ACCOUNT_ACCESS` for
    /// the first access of the index) and warms the account. The beneficiary of a
    /// `SELFDESTRUCT` pays only the cold access, nothing when warm.
    fn access_account(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &B160,
        account: u32,
        is_selfdestruct: bool,
    ) -> Result<()> {
        let gas = match ee_type {
            ExecutionEnvironmentType::NoEE => 0,
            ExecutionEnvironmentType::EVM => {
                if is_selfdestruct {
                    0
                } else {
                    WARM_ACCESS
                }
            }
        };
        resources.charge_legacy_gas_and_native(gas, WARM_ACCOUNT_CACHE_ACCESS_NATIVE_COST)?;
        let base = &mut self.accounts[account as usize];
        base.ever_accessed = true;
        if base.head == 0 {
            match ee_type {
                ExecutionEnvironmentType::NoEE => {}
                ExecutionEnvironmentType::EVM => {
                    // the precompiles are warm from the start of every transaction
                    if !evm_interpreter::utils::is_precompile(address) {
                        let extra = if is_selfdestruct {
                            COLD_ACCOUNT_ACCESS
                        } else {
                            COLD_ACCOUNT_ACCESS - WARM_ACCESS
                        };
                        resources.charge_legacy_gas(extra)?;
                    }
                }
            }
            // warm from here on: the first version of the index, the base's values
            let view = AccountView {
                nonce: base.nonce,
                balance: base.balance,
                code_hash: base.code_hash,
                deployed: false,
                deconstruct: false,
            };
            self.push_account_version(account, view)?;
        }
        // An account marked for deconstruction (self-destructed in the transaction that
        // created it) stays accessible until the end of the index like any other: its code
        // still runs, its balance reads as it is; `finish_index` finalizes it.
        Ok(())
    }

    /// The current values of an account: its head version, or its base when it is
    /// untouched in the index
    #[inline(always)]
    fn account_view(&self, account: u32) -> AccountView {
        let base = &self.accounts[account as usize];
        if base.head == 0 {
            AccountView {
                nonce: base.nonce,
                balance: base.balance,
                code_hash: base.code_hash,
                deployed: false,
                deconstruct: false,
            }
        } else {
            let v = self.account_versions.get(base.head - 1);
            AccountView {
                nonce: v.nonce,
                balance: v.balance,
                code_hash: v.code_hash,
                deployed: v.deployed,
                deconstruct: v.deconstruct,
            }
        }
    }

    /// Pushes a version of the account with these values and makes it the head
    #[inline(always)]
    fn push_account_version(
        &mut self,
        account: u32,
        view: AccountView,
    ) -> Result<(), InternalError> {
        let previous = self.accounts[account as usize].head;
        let head = self.account_versions.push(AccountVersion {
            nonce: view.nonce,
            balance: view.balance,
            code_hash: view.code_hash,
            account,
            previous,
            deployed: view.deployed,
            deconstruct: view.deconstruct,
        })?;
        self.accounts[account as usize].head = head;
        Ok(())
    }

    /// Records a change of the account as a new version
    fn mutate_account(
        &mut self,
        account: u32,
        change: impl FnOnce(&mut AccountView),
    ) -> Result<(), InternalError> {
        let mut view = self.account_view(account);
        change(&mut view);
        self.push_account_version(account, view)
    }

    /// The current value of a slot: its head version, or its base
    #[inline(always)]
    fn slot_current(&self, position: u32) -> Bytes32 {
        let base = &self.slots[position as usize];
        if base.head == 0 {
            base.start
        } else {
            self.slot_versions.get(base.head - 1).value
        }
    }

    /// Whether a cold write extra was charged for the slot in this index
    #[inline(always)]
    fn slot_write_charged(&self, position: u32) -> bool {
        let base = &self.slots[position as usize];
        base.head != 0 && self.slot_versions.get(base.head - 1).write_charged
    }

    /// Pushes a version of the slot with this value and makes it the head
    #[inline(always)]
    fn push_slot_version(
        &mut self,
        position: u32,
        value: Bytes32,
        write_charged: bool,
    ) -> Result<(), InternalError> {
        let previous = self.slots[position as usize].head;
        let head = self.slot_versions.push(SlotVersion {
            value,
            position,
            previous,
            write_charged,
        })?;
        self.slots[position as usize].head = head;
        Ok(())
    }

    /// Charges a slot read and warms the slot; returns whether it was warm
    fn access_slot(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        position: u32,
    ) -> Result<bool> {
        self.policy.charge_warm_storage_read(ee_type, resources)?;
        let base = &mut self.slots[position as usize];
        base.ever_accessed = true;
        let is_warm = base.head != 0;
        if !is_warm {
            let is_new = base.is_new && !base.write_charged_before;
            let start = base.start;
            self.policy
                .charge_cold_storage_read_extra(ee_type, resources, is_new, false)?;
            // warm from here on: the first version of the index, the base's value
            self.push_slot_version(position, start, false)?;
        }
        Ok(is_warm)
    }

    /// Applies the EIP-8246 finalization to self-destructed accounts, then
    /// checks the writes of the index against the declared changes
    fn finish_index(&mut self) -> Result<(), InternalError> {
        // EIP-8246: nonce 0, no code, no storage, balance kept. The accounts marked are
        // found through their head versions (the loop pushes versions past `len`).
        let len = self.account_versions.len();
        for i in 0..len {
            let v = *self.account_versions.get(i);
            if !v.deconstruct || self.accounts[v.account as usize].head != i + 1 {
                continue;
            }
            self.mutate_account(v.account, |state| {
                state.nonce = 0;
                state.code_hash = EMPTY_STRING_KECCAK_HASH;
                state.deconstruct = false;
            })?;
            let access = self.tables.accounts[v.account as usize];
            for position in access.slots_start..access.slots_end {
                self.write_slot_value(position, Bytes32::ZERO)?;
            }
        }

        let index = self.current_index as usize;
        let declared = &self.changes
            [self.change_starts[index] as usize..self.change_starts[index + 1] as usize];
        // every declared change happened, with the declared final value
        for change in declared {
            let ok = match change.change {
                Change::Balance { account, value } => {
                    let a = &self.accounts[account as usize];
                    let current = self.account_view(account);
                    a.head != 0 && current.balance == value && current.balance != a.balance
                }
                Change::Nonce { account, value } => {
                    let a = &self.accounts[account as usize];
                    let current = self.account_view(account);
                    a.head != 0 && current.nonce == value && current.nonce != a.nonce
                }
                Change::Code { account, hash } => {
                    let a = &self.accounts[account as usize];
                    let current = self.account_view(account);
                    a.head != 0 && current.code_hash == hash && current.code_hash != a.code_hash
                }
                Change::Slot { position, value } => {
                    let s = &self.slots[position as usize];
                    let current = self.slot_current(position);
                    s.head != 0 && current == value && current != s.start
                }
            };
            if !ok {
                return Err(internal_error!(
                    "a change declared by the block access list did not happen"
                ));
            }
        }
        // and nothing else did: the actual changes are as many as the declared ones. The
        // entries touched in the index are those with a head version; a rolled-back
        // version is gone from the arena, so every one left belongs to the index.
        let mut actual = 0usize;
        for i in 0..self.account_versions.len() {
            let v = self.account_versions.get(i);
            let a = &self.accounts[v.account as usize];
            if a.head != i + 1 {
                continue;
            }
            actual += (v.balance != a.balance) as usize
                + (v.nonce != a.nonce) as usize
                + (v.code_hash != a.code_hash) as usize;
        }
        for i in 0..self.slot_versions.len() {
            let v = self.slot_versions.get(i);
            let s = &self.slots[v.position as usize];
            actual += (s.head == i + 1 && v.value != s.start) as usize;
        }
        if actual != declared.len() {
            return Err(self.undeclared_change_error(index as u32, declared.len(), actual));
        }
        // the index ends: the head versions become the bases of the next one, and
        // nothing of the index outlives it
        for i in 0..self.account_versions.len() {
            let v = *self.account_versions.get(i);
            let a = &mut self.accounts[v.account as usize];
            if a.head == i + 1 {
                a.nonce = v.nonce;
                a.balance = v.balance;
                a.code_hash = v.code_hash;
                a.head = 0;
            }
        }
        for i in 0..self.slot_versions.len() {
            let v = *self.slot_versions.get(i);
            let s = &mut self.slots[v.position as usize];
            if s.head == i + 1 {
                s.start = v.value;
                s.write_charged_before |= v.write_charged;
                s.head = 0;
            }
        }
        self.account_versions.clear();
        self.slot_versions.clear();
        Ok(())
    }

    /// The error of a transaction whose changes are not the declared ones, with every
    /// change that is not declared (or declared and missing) as context for the host:
    /// address, field, value at the start of the index, current value, slot key
    #[cold]
    fn undeclared_change_error(&self, index: u32, declared: usize, actual: usize) -> InternalError {
        use zk_ee::error_ctx;
        use zk_ee::system::errors::context::contextualized::Contextualized as _;
        let changes = &self.changes[self.change_starts[index as usize] as usize
            ..self.change_starts[index as usize + 1] as usize];
        let is_declared =
            |probe: &dyn Fn(&Change) -> bool| changes.iter().any(|c| probe(&c.change));
        let mut offending: Vec<(B160, &'static str, Bytes32, Bytes32, Bytes32), A> =
            Vec::new_in(self.allocator.clone());
        for i in 0..self.account_versions.len() {
            let v = self.account_versions.get(i);
            let account = v.account;
            let a = &self.accounts[account as usize];
            if a.head != i + 1 {
                continue;
            }
            let address = self.tables.addresses.key(account).0;
            let fields: [(&'static str, bool, bool, Bytes32, Bytes32); 3] = [
                (
                    "balance",
                    v.balance != a.balance,
                    is_declared(
                        &|c| matches!(c, Change::Balance { account: x, .. } if *x == account),
                    ),
                    Bytes32::from_array(a.balance.to_be_bytes()),
                    Bytes32::from_array(v.balance.to_be_bytes()),
                ),
                (
                    "nonce",
                    v.nonce != a.nonce,
                    is_declared(
                        &|c| matches!(c, Change::Nonce { account: x, .. } if *x == account),
                    ),
                    Bytes32::from_array(U256::from(a.nonce).to_be_bytes()),
                    Bytes32::from_array(U256::from(v.nonce).to_be_bytes()),
                ),
                (
                    "code",
                    v.code_hash != a.code_hash,
                    is_declared(&|c| matches!(c, Change::Code { account: x, .. } if *x == account)),
                    a.code_hash,
                    v.code_hash,
                ),
            ];
            for (name, changed, declared, start, current) in fields {
                if changed != declared {
                    offending.push((address, name, start, current, Bytes32::ZERO));
                }
            }
        }
        for i in 0..self.slot_versions.len() {
            let v = self.slot_versions.get(i);
            let position = v.position;
            let s = &self.slots[position as usize];
            if s.head != i + 1 {
                continue;
            }
            let access = self.tables.slots[position as usize];
            let address = self.tables.addresses.key(access.address_index).0;
            let key = self.tables.slot_keys.key(access.key_index).0;
            let declared =
                is_declared(&|c| matches!(c, Change::Slot { position: p, .. } if *p == position));
            if (v.value != s.start) != declared {
                offending.push((address, "slot", s.start, v.value, key));
            }
        }
        let error: InternalError =
            internal_error!("the transaction changed state the block access list does not declare");
        error.with_context(|| {
            error_ctx! {
                "index" => debug_format(index),
                "declared" => debug_format(declared),
                "actual" => debug_format(actual),
                "offending" => debug_format(offending),
            }
        })
    }

    /// Writes a slot value in the current index: a new version, which keeps the write
    /// charge flag of the one before
    fn write_slot_value(&mut self, position: u32, value: Bytes32) -> Result<(), InternalError> {
        let write_charged = self.slot_write_charged(position);
        self.push_slot_version(position, value, write_charged)
    }
}

impl<
        A: Allocator + Clone,
        R: StateGasResources,
        P: StorageAccessPolicy<R, Bytes32>,
        const PROOF_ENV: bool,
    > SnapshottableIo for BalStorageModel<A, R, P, PROOF_ENV>
{
    type StateSnapshot = BalModelSnapshot<R>;

    fn begin_new_tx(&mut self) {
        // the index is set by `begin_index`; the loop advances it per transaction
    }

    fn finish_tx(&mut self) -> Result<(), InternalError> {
        self.finish_index()
    }

    fn start_frame(&mut self) -> BalModelSnapshot<R> {
        BalModelSnapshot {
            account_versions: self.account_versions.len(),
            slot_versions: self.slot_versions.len(),
            refund: self.refund_counter.clone(),
        }
    }

    fn finish_frame(
        &mut self,
        rollback_handle: Option<&BalModelSnapshot<R>>,
    ) -> Result<(), InternalError> {
        let Some(snapshot) = rollback_handle else {
            return Ok(());
        };
        // The versions the frame pushed are walked forward: the first one of an entry
        // in the frame points at the head before the frame, which the entry goes back
        // to; the later ones of the same entry are skipped, as its head already points
        // before them. Then the arena is cut back to the frame's start.
        for i in snapshot.account_versions..self.account_versions.len() {
            let v = self.account_versions.get(i);
            let base = &mut self.accounts[v.account as usize];
            if base.head > i {
                base.head = v.previous;
            }
        }
        self.account_versions.truncate(snapshot.account_versions);
        for i in snapshot.slot_versions..self.slot_versions.len() {
            let v = self.slot_versions.get(i);
            let base = &mut self.slots[v.position as usize];
            if base.head > i {
                base.head = v.previous;
            }
        }
        self.slot_versions.truncate(snapshot.slot_versions);
        self.refund_counter = snapshot.refund.clone();
        Ok(())
    }
}

impl<
        A: Allocator + Clone,
        R: StateGasResources,
        P: StorageAccessPolicy<R, Bytes32>,
        const PROOF_ENV: bool,
    > StorageModel for BalStorageModel<A, R, P, PROOF_ENV>
{
    type IOTypes = EthereumIOTypesConfig;
    type Resources = R;
    type StorageCommitment = Bytes32;
    type Allocator = A;
    /// Constructed by `new`, not through the trait
    type InitData = ();

    fn construct(_init_data: (), _allocator: A) -> Self {
        unreachable!("the model is built from the witness with `BalStorageModel::new`")
    }

    fn pubdata_used_by_tx(&self) -> u32 {
        0
    }

    fn storage_read(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &B160,
        key: &Bytes32,
        _oracle: &mut impl IOOracle,
    ) -> Result<Bytes32> {
        let account = self.account_index_charging(ee_type, resources, address)?;
        let position = self.slot_position_charging(ee_type, resources, account, key)?;
        self.access_slot(ee_type, resources, position)?;
        Ok(self.slot_current(position))
    }

    /// Warms a slot (a transaction access list entry). A warm-up is not an access: a
    /// slot outside the block access list was never reached by the execution (the
    /// list does not carry access list entries nothing accessed), so there is nothing
    /// to warm
    fn storage_touch(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &B160,
        key: &Bytes32,
        _oracle: &mut impl IOOracle,
    ) -> Result<()> {
        self.policy.charge_warm_storage_read(ee_type, resources)?;
        let Some(account) = self.tables.addresses.get(&AddressKey(*address)) else {
            return Ok(());
        };
        let Ok(position) = self.slot_position(account, key) else {
            return Ok(());
        };
        let base = &mut self.slots[position as usize];
        base.ever_accessed = true;
        if base.head == 0 {
            let start = base.start;
            self.push_slot_version(position, start, false)?;
        }
        Ok(())
    }

    fn storage_write(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &B160,
        key: &Bytes32,
        new_value: &Bytes32,
        _oracle: &mut impl IOOracle,
    ) -> Result<Bytes32> {
        let account = self.account_index_charging(ee_type, resources, address)?;
        let position = self.slot_position_charging(ee_type, resources, account, key)?;
        let is_warm_access = self.access_slot(ee_type, resources, position)?;
        let base = self.slots[position as usize];
        let at_tx_start = base.start;
        let current = self.slot_current(position);
        let is_cold_write_charged = self.slot_write_charged(position);
        let is_new_slot = base.is_new && !base.write_charged_before && !is_cold_write_charged;
        self.policy.charge_storage_write_extra(
            ee_type,
            &at_tx_start,
            &current,
            new_value,
            resources,
            is_warm_access,
            is_cold_write_charged,
            is_new_slot,
        )?;
        let mut refund = self.refund_counter.clone();
        self.policy.refund_for_storage_write(
            ee_type,
            &at_tx_start,
            &current,
            new_value,
            resources,
            &mut refund,
        )?;
        self.refund_counter = refund;
        // the version of the write carries the write charge flag
        let write_charged = is_cold_write_charged || *new_value != current;
        self.push_slot_version(position, *new_value, write_charged)?;
        Ok(current)
    }

    fn read_account_properties<
        EEVersion: Maybe<u8>,
        ObservableBytecodeHash: Maybe<Bytes32>,
        ObservableBytecodeLen: Maybe<u32>,
        Nonce: Maybe<u64>,
        BytecodeHash: Maybe<Bytes32>,
        BytecodeLen: Maybe<u32>,
        ArtifactsLen: Maybe<u32>,
        NominalTokenBalance: Maybe<U256>,
        Bytecode: Maybe<&'static [u8]>,
        CodeVersion: Maybe<u8>,
        IsDelegated: Maybe<bool>,
    >(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &B160,
        _request: AccountDataRequest<
            AccountData<
                EEVersion,
                ObservableBytecodeHash,
                ObservableBytecodeLen,
                Nonce,
                BytecodeHash,
                BytecodeLen,
                ArtifactsLen,
                NominalTokenBalance,
                Bytecode,
                CodeVersion,
                IsDelegated,
            >,
        >,
        oracle: &mut impl IOOracle,
    ) -> Result<
        AccountData<
            EEVersion,
            ObservableBytecodeHash,
            ObservableBytecodeLen,
            Nonce,
            BytecodeHash,
            BytecodeLen,
            ArtifactsLen,
            NominalTokenBalance,
            Bytecode,
            CodeVersion,
            IsDelegated,
        >,
    > {
        let account = self.account_index_charging(ee_type, resources, address)?;
        self.access_account(ee_type, resources, address, account, false)?;
        let state = self.account_view(account);

        let needs_preimage = ObservableBytecodeLen::IS_MATERIAL
            || BytecodeLen::IS_MATERIAL
            || ArtifactsLen::IS_MATERIAL
            || Bytecode::IS_MATERIAL
            || IsDelegated::IS_MATERIAL;
        let needs_artifacts = ArtifactsLen::IS_MATERIAL;
        let (bytecode, code_length, artifacts_len): (&'static [u8], u32, u32) =
            if needs_preimage && state.code_hash != EMPTY_STRING_KECCAK_HASH {
                let executable = self.preimages.get_executable_bytecode::<PROOF_ENV>(
                    ee_type,
                    &state.code_hash,
                    resources,
                    oracle,
                    needs_artifacts,
                )?;
                (
                    executable.bytecode,
                    executable.code_len,
                    executable.artifacts_len,
                )
            } else {
                (&[], 0, 0)
            };
        let code_version = if artifacts_len > 0 {
            evm_interpreter::ARTIFACTS_FROM_CODE_CACHE_CODE_VERSION_BYTE
        } else {
            evm_interpreter::DEFAULT_CODE_VERSION_BYTE
        };
        let is_delegated = code_length == 3 + 20 && bytecode[..3] == EIP7702_DELEGATION_MARKER;
        // the hash of an empty account is zero (EXTCODEHASH), of an existing one its code hash
        let observable_hash = if state.code_hash == EMPTY_STRING_KECCAK_HASH
            && state.nonce == 0
            && state.balance.is_zero()
        {
            Bytes32::ZERO
        } else {
            state.code_hash
        };

        Ok(AccountData {
            ee_version: Maybe::construct(|| ExecutionEnvironmentType::EVM as u8),
            observable_bytecode_hash: Maybe::construct(|| observable_hash),
            observable_bytecode_len: Maybe::construct(|| code_length),
            nonce: Maybe::construct(|| state.nonce),
            bytecode_hash: Maybe::construct(|| state.code_hash),
            unpadded_code_len: Maybe::construct(|| code_length),
            artifacts_len: Maybe::construct(|| artifacts_len),
            nominal_token_balance: Maybe::construct(|| state.balance),
            bytecode: Maybe::construct(|| bytecode),
            code_version: Maybe::construct(|| code_version),
            is_delegated: Maybe::construct(|| is_delegated),
        })
    }

    /// Warms an account (a transaction access list entry, the coinbase, the delegate of
    /// an EIP-7702 recipient). A warm-up records no access: an address outside the
    /// block access list was never reached by the real execution past this point, so
    /// there is nothing to warm, but the access is still paid for, as it is what may
    /// have run the transaction out of gas before it reached the address (a recipient's
    /// delegate the gas does not cover). A precompile is an account like any other here
    /// (it can be called and receive value); it is only never cold, see `access_account`.
    fn touch_account(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &B160,
        _oracle: &mut impl IOOracle,
    ) -> Result<()> {
        match self.tables.addresses.get(&AddressKey(*address)) {
            Some(account) => self.access_account(ee_type, resources, address, account, false),
            None => {
                if ee_type == ExecutionEnvironmentType::EVM
                    && !evm_interpreter::utils::is_precompile(address)
                {
                    resources.charge_legacy_gas(COLD_ACCOUNT_ACCESS)?;
                }
                Ok(())
            }
        }
    }

    fn increment_nonce(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &B160,
        increment_by: u64,
        _oracle: &mut impl IOOracle,
    ) -> Result<u64, NonceSubsystemError> {
        let account = self.account_index_charging(ee_type, resources, address)?;
        self.access_account(ee_type, resources, address, account, false)?;
        resources.charge(&R::from_native(R::Native::from_computational(
            WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST,
        )))?;
        let nonce = self.account_view(account).nonce;
        let Some(new_nonce) = nonce.checked_add(increment_by) else {
            return Err(interface_error!(NonceError::NonceOverflow));
        };
        self.mutate_account(account, |state| state.nonce = new_nonce)?;
        Ok(nonce)
    }

    fn update_nominal_token_value(
        &mut self,
        from_ee: ExecutionEnvironmentType,
        resources: &mut R,
        address: &B160,
        update_fn: impl FnOnce(&U256) -> Result<U256, BalanceSubsystemError>,
        _oracle: &mut impl IOOracle,
        _fee_payment_in_simulation: bool,
    ) -> Result<U256, BalanceSubsystemError> {
        let account = self.account_index_charging(from_ee, resources, address)?;
        self.access_account(from_ee, resources, address, account, false)?;
        resources.charge(&R::from_native(R::Native::from_computational(
            WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST,
        )))?;
        let current = self.account_view(account).balance;
        let new = update_fn(&current)?;
        self.mutate_account(account, |state| state.balance = new)?;
        Ok(current)
    }

    fn get_selfbalance(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &B160,
    ) -> Result<U256> {
        if ee_type == ExecutionEnvironmentType::EVM {
            resources.charge_legacy_gas(KNOWN_TO_BE_WARM_PROPERTIES_ACCESS_COST_GAS)?;
        }
        let account = self.account_index(address)?;
        Ok(self.account_view(account).balance)
    }

    fn transfer_nominal_token_value(
        &mut self,
        from_ee: ExecutionEnvironmentType,
        resources: &mut R,
        from: &B160,
        to: &B160,
        amount: &U256,
        oracle: &mut impl IOOracle,
    ) -> Result<(), BalanceSubsystemError> {
        self.update_nominal_token_value(
            from_ee,
            resources,
            from,
            |old| {
                old.checked_sub(*amount)
                    .ok_or(interface_error!(BalanceError::InsufficientBalance))
            },
            oracle,
            false,
        )?;
        self.update_nominal_token_value(
            from_ee,
            resources,
            to,
            |old| {
                old.checked_add(*amount)
                    .ok_or(interface_error!(BalanceError::Overflow))
            },
            oracle,
            false,
        )?;
        Ok(())
    }

    fn deploy_code(
        &mut self,
        from_ee: ExecutionEnvironmentType,
        resources: &mut R,
        at_address: &B160,
        bytecode: &[u8],
        _oracle: &mut impl IOOracle,
    ) -> Result<(&'static [u8], Bytes32, u32)> {
        if from_ee != ExecutionEnvironmentType::EVM {
            return Err(internal_error!("Deployment cannot happen in NoEE").into());
        }
        // the deposit (hashing gas and storage state gas, EIP-8037) is charged by the
        // interpreter; here only the native work
        let account = self.account_index(at_address)?;
        resources.with_infinite_ergs(|inf_resources| {
            self.access_account(from_ee, inf_resources, at_address, account, false)
        })?;
        let native_cost = keccak256_native_cost::<R>(bytecode.len());
        resources.charge(&R::from_native(native_cost))?;
        resources.charge(&R::from_native(R::Native::from_computational(
            blake2s_native_cost(bytecode.len()),
        )))?;
        let hash = Bytes32::from_array(*Keccak256::digest(bytecode));
        let stored = self.preimages.record_preimage::<PROOF_ENV>(
            from_ee,
            &PreimageRequestForUnknownLength {
                hash,
                preimage_type: PreimageType::Bytecode,
            },
            resources,
            &[bytecode],
        )?;
        resources.charge(&R::from_native(R::Native::from_computational(
            WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST,
        )))?;
        self.mutate_account(account, |state| {
            state.code_hash = hash;
            state.deployed = true;
        })?;
        Ok((stored, hash, stored.len() as u32))
    }

    fn set_bytecode_details(
        &mut self,
        _resources: &mut R,
        _at_address: &B160,
        _ee: ExecutionEnvironmentType,
        _bytecode_hash: Bytes32,
        _bytecode_len: u32,
        _artifacts_len: u32,
        _observable_bytecode_hash: Bytes32,
        _observable_bytecode_len: u32,
        _oracle: &mut impl IOOracle,
    ) -> Result<()> {
        Err(internal_error!("not valid for this storage model").into())
    }

    fn set_delegation(
        &mut self,
        resources: &mut R,
        at_address: &B160,
        delegate: &B160,
        _oracle: &mut impl IOOracle,
    ) -> Result<()> {
        let account = self.account_index(at_address)?;
        resources.with_infinite_ergs(|inf_resources| {
            self.access_account(
                ExecutionEnvironmentType::EVM,
                inf_resources,
                at_address,
                account,
                false,
            )
        })?;
        let hash = if delegate == &B160::ZERO {
            EMPTY_STRING_KECCAK_HASH
        } else {
            let mut code = [0u8; 23];
            code[0..3].copy_from_slice(&EIP7702_DELEGATION_MARKER);
            code[3..].copy_from_slice(&delegate.to_be_bytes::<{ B160::BYTES }>());
            resources.charge(&R::from_native(R::Native::from_computational(
                blake2s_native_cost(code.len()),
            )))?;
            let hash = Bytes32::from_array(*Keccak256::digest(&code));
            self.preimages.record_preimage::<PROOF_ENV>(
                ExecutionEnvironmentType::NoEE,
                &PreimageRequestForUnknownLength {
                    hash,
                    preimage_type: PreimageType::Bytecode,
                },
                resources,
                &[&code],
            )?;
            hash
        };
        resources.charge(&R::from_native(R::Native::from_computational(
            WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST,
        )))?;
        self.mutate_account(account, |state| state.code_hash = hash)?;
        Ok(())
    }

    /// `SELFDESTRUCT` after its base cost: the beneficiary's access (cold only), its
    /// write and creation when the sweep revives it (EIP-8038, EIP-8037), the sweep
    /// itself, and the destruction mark of a contract created in this transaction
    /// (EIP-6780); the finalization keeps the balance (EIP-8246, see `finish_index`).
    /// A self-destruct to itself moves nothing (EIP-8246).
    fn mark_for_deconstruction(
        &mut self,
        from_ee: ExecutionEnvironmentType,
        resources: &mut R,
        at_address: &B160,
        nominal_token_beneficiary: &B160,
        oracle: &mut impl IOOracle,
    ) -> Result<U256, DeconstructionSubsystemError> {
        let account = self.account_index(at_address)?;
        // the originator is warm (it is executing)
        resources.with_infinite_ergs(|inf_resources| {
            self.access_account(from_ee, inf_resources, at_address, account, false)
        })?;
        resources.charge(&R::from_native(R::Native::from_computational(
            WARM_ACCOUNT_CACHE_WRITE_EXTRA_NATIVE_COST,
        )))?;
        let state = self.account_view(account);
        let same_address = at_address == nominal_token_beneficiary;
        let transfer_amount = state.balance;

        let beneficiary =
            self.account_index_charging(from_ee, resources, nominal_token_beneficiary)?;
        self.access_account(
            from_ee,
            resources,
            nominal_token_beneficiary,
            beneficiary,
            true,
        )?;
        if from_ee == ExecutionEnvironmentType::EVM {
            let b = self.account_view(beneficiary);
            let beneficiary_alive =
                b.nonce != 0 || b.code_hash != EMPTY_STRING_KECCAK_HASH || !b.balance.is_zero();
            if !beneficiary_alive && !transfer_amount.is_zero() {
                resources.charge_legacy_gas(ACCOUNT_WRITE)?;
                resources.charge_state_gas(NEW_ACCOUNT)?;
            }
        }

        // EIP-6780: only a contract created in this transaction is destroyed (a self-destruct
        // with no code yet is in its constructor)
        let should_be_deconstructed = state.deployed || state.code_hash == EMPTY_STRING_KECCAK_HASH;
        if should_be_deconstructed {
            self.mutate_account(account, |state| state.deconstruct = true)?;
        }
        if !same_address {
            resources
                .with_infinite_ergs(|inf_resources| {
                    self.transfer_nominal_token_value(
                        from_ee,
                        inf_resources,
                        at_address,
                        nominal_token_beneficiary,
                        &transfer_amount,
                        oracle,
                    )
                })
                .map_err(wrap_error!())?;
        }
        Ok(transfer_amount)
    }

    fn get_refund_counter(&'_ self) -> &'_ R {
        &self.refund_counter
    }

    fn add_to_refund_counter(&mut self, refund: R) -> Result<()> {
        self.refund_counter.add_ergs(refund.ergs());
        Ok(())
    }

    fn persist_caches(
        &mut self,
        _oracle: &mut impl IOOracle,
        _result_keeper: &mut impl IOResultKeeper<EthereumIOTypesConfig>,
    ) {
    }

    fn report_new_preimages(
        &mut self,
        result_keeper: &mut impl IOResultKeeper<EthereumIOTypesConfig>,
    ) {
        result_keeper.new_preimages(
            self.preimages
                .storage
                .iter()
                .map(|(k, v)| (k, v.code(), PreimageType::Bytecode)),
        );
    }

    type StorageKey<'a>
        = WarmStorageKey
    where
        Self: 'a;
    type StorageDiff<'a>
        = StorageDiff<Self::IOTypes>
    where
        Self: 'a;

    fn get_storage_diff<'a>(
        &'a self,
        key: WarmStorageKey,
    ) -> Option<StorageDiff<EthereumIOTypesConfig>> {
        let account = self.account_index(&key.address).ok()?;
        let position = self.slot_position(account, &key.key).ok()?;
        let state = &self.slots[position as usize];
        Some(StorageDiff {
            initial_value: state.start,
            current_value: self.slot_current(position),
            is_new_storage_slot: state.is_new,
            initial_value_used: true,
        })
    }

    fn storage_diffs_iterator<'a>(
        &'a self,
    ) -> impl ExactSizeIterator<Item = (WarmStorageKey, StorageDiff<EthereumIOTypesConfig>)> + Clone
    {
        let tables = &self.tables;
        let addresses = tables.addresses.keys();
        let slot_keys = tables.slot_keys.keys();
        self.slots.iter().enumerate().map(move |(position, state)| {
            let access = tables.slots[position];
            (
                WarmStorageKey {
                    address: addresses[access.address_index as usize].0,
                    key: slot_keys[access.key_index as usize].0,
                },
                StorageDiff {
                    initial_value: state.start,
                    current_value: self.slot_current(position as u32),
                    is_new_storage_slot: state.is_new,
                    initial_value_used: true,
                },
            )
        })
    }

    fn update_commitment(
        &mut self,
        _state_commitment: Option<&mut Bytes32>,
        _oracle: &mut impl IOOracle,
        _logger: &mut impl zk_ee::system::logger::Logger,
        _result_keeper: &mut impl IOResultKeeper<EthereumIOTypesConfig>,
    ) {
        // the pre-execution check commits the state; a range only checks its transactions
    }
}
