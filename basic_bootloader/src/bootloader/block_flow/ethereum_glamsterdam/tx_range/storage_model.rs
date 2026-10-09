//! The storage model of a transaction range: every account and slot the block
//! touches is known up front from the block access list, with its pre-block
//! leaf from the pre-execution tables, so there is no cache to fill from the
//! oracle. The model keeps one current value per account field and per slot,
//! journals the writes of the running access index for frame rollbacks, and
//! at the end of every index checks that the writes are exactly the changes
//! the access list declares for it (a symmetric difference of zero), after
//! which the journal is dropped: nothing past the current values is kept.
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
use zk_ee::common_structs::skip_list_quasi_vec::ListVec;
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

/// Current state of an account, by address index
#[derive(Clone, Copy, Debug)]
struct AccountState {
    nonce: u64,
    balance: U256,
    /// Never zero: the hash of the empty string for an account without code
    code_hash: Bytes32,
    /// Stamp of the access index the account is warm in (0: cold)
    warm: u32,
    /// Stamp of the access index the account was first changed in (0: never)
    changed: u32,
    /// Values at the start of the index of `changed`
    start_nonce: u64,
    start_balance: U256,
    start_code_hash: Bytes32,
    /// Stamp of the access index code was deployed in (0: never)
    deployed: u32,
    /// Self-destructed in the current index
    deconstruct: bool,
    /// Accessed at some point of the range, in a reverted frame or not (never
    /// journaled): the aggregation checks that the access list is exact
    ever_accessed: bool,
}

/// Current state of a slot, by slot position
#[derive(Clone, Copy, Debug)]
struct SlotState {
    current: Bytes32,
    /// Value at the start of the index of `written`
    start: Bytes32,
    /// Had no leaf before the block (reads of it are "new slot" reads)
    is_new: bool,
    warm: u32,
    written: u32,
    /// Stamp of the index a cold write extra was charged in (0: never)
    write_charged: u32,
    /// Accessed at some point of the range (never journaled)
    ever_accessed: bool,
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

/// What a frame rollback undoes
#[derive(Clone, Copy, Debug)]
enum JournalEntry {
    AccountWarm { account: u32, old: u32 },
    Account { account: u32, old: AccountState },
    SlotWarm { position: u32, old: u32 },
    Slot { position: u32, old: SlotState },
}

/// A frame snapshot: the journal length and the refund counter
pub struct BalModelSnapshot<R> {
    journal_len: usize,
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
    accounts: Vec<AccountState, A>,
    slots: Vec<SlotState, A>,
    /// The declared changes, sorted by access index
    counts: AccessListCounts,
    changes: Vec<IndexedChange, A>,
    /// First change of every index, with a final end
    change_starts: Vec<u32, A>,
    pub range: BlockAccessIndexRange,
    /// The access index being executed
    current_index: u32,
    /// Stamp of the current index (never 0)
    stamp: u32,
    /// The writes of the running index, for frame rollbacks: a chunked list, as the
    /// proving-target allocator cannot grow a buffer
    journal: ListVec<JournalEntry, 32, A>,
    allocator: A,
    /// Accounts and slots changed in the current index
    touched_accounts: ListVec<u32, 32, A>,
    touched_slots: ListVec<u32, 32, A>,
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

fn not_in_access_list(address: &B160) -> SystemError {
    not_in_access_list_slot(address, None)
}

fn not_in_access_list_slot(address: &B160, key: Option<&Bytes32>) -> SystemError {
    not_in_access_list_at(address, key, u32::MAX)
}

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
            accounts.push(AccountState {
                nonce,
                balance,
                code_hash: if code_hash.is_zero() {
                    EMPTY_STRING_KECCAK_HASH
                } else {
                    code_hash
                },
                warm: 0,
                changed: 0,
                start_nonce: nonce,
                start_balance: balance,
                start_code_hash: code_hash,
                deployed: 0,
                deconstruct: false,
                ever_accessed: false,
            });
        }
        let mut slots = Vec::with_capacity_in(counts.slots, allocator.clone());
        for item in initial.slots() {
            let value = match item {
                Some(item) => integer_item_value(item)?,
                None => Bytes32::ZERO,
            };
            slots.push(SlotState {
                current: value,
                start: value,
                is_new: item.is_none(),
                warm: 0,
                written: 0,
                write_charged: 0,
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
            counts,
            changes,
            change_starts,
            range,
            current_index: range.first,
            stamp: 1,
            journal: ListVec::new_in(allocator.clone()),
            touched_accounts: ListVec::new_in(allocator.clone()),
            touched_slots: ListVec::new_in(allocator.clone()),
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
                    new.slots[position as usize].current = value;
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
        self.stamp += 1;
        self.journal.clear();
        self.touched_accounts.clear();
        self.touched_slots.clear();
        self.refund_counter = R::empty();
        Ok(())
    }

    // --- lookups -----------------------------------------------------------

    fn account_index(&self, address: &B160) -> Result<u32> {
        self.tables
            .addresses
            .get(&AddressKey(*address))
            .ok_or_else(|| not_in_access_list(address))
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
                Err(not_in_access_list(address))
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
            not_in_access_list_slot(&self.tables.addresses.key(account).0, Some(key))
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
        self.accounts[account as usize].ever_accessed = true;
        let state = &self.accounts[account as usize];
        if state.warm != self.stamp {
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
            self.journal.push(JournalEntry::AccountWarm {
                account,
                old: state.warm,
            });
            self.accounts[account as usize].warm = self.stamp;
        }
        // An account marked for deconstruction (self-destructed in the transaction that
        // created it) stays accessible until the end of the index like any other: its code
        // still runs, its balance reads as it is; `finish_index` finalizes it.
        Ok(())
    }

    /// Journals the account before a change and records it as changed
    fn mutate_account(&mut self, account: u32, change: impl FnOnce(&mut AccountState)) {
        let old = self.accounts[account as usize];
        self.journal.push(JournalEntry::Account { account, old });
        let state = &mut self.accounts[account as usize];
        if state.changed != self.stamp {
            state.changed = self.stamp;
            state.start_nonce = old.nonce;
            state.start_balance = old.balance;
            state.start_code_hash = old.code_hash;
            self.touched_accounts.push(account);
        }
        change(state);
    }

    /// Charges a slot read and warms the slot; returns whether it was warm
    fn access_slot(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        position: u32,
    ) -> Result<bool> {
        self.policy.charge_warm_storage_read(ee_type, resources)?;
        self.slots[position as usize].ever_accessed = true;
        let state = self.slots[position as usize];
        let is_warm = state.warm == self.stamp;
        if !is_warm {
            self.policy.charge_cold_storage_read_extra(
                ee_type,
                resources,
                state.is_new && state.write_charged == 0,
                false,
            )?;
            self.journal.push(JournalEntry::SlotWarm {
                position,
                old: state.warm,
            });
            self.slots[position as usize].warm = self.stamp;
        }
        Ok(is_warm)
    }

    /// Value of a slot at the start of the current index
    fn slot_value_at_index_start(&self, position: u32) -> Bytes32 {
        let state = &self.slots[position as usize];
        if state.written == self.stamp {
            state.start
        } else {
            state.current
        }
    }

    /// Applies the EIP-8246 finalization to self-destructed accounts, then
    /// checks the writes of the index against the declared changes
    fn finish_index(&mut self) -> Result<(), InternalError> {
        // EIP-8246: nonce 0, no code, no storage, balance kept
        for account in 0..self.accounts.len() as u32 {
            if !self.accounts[account as usize].deconstruct {
                continue;
            }
            self.mutate_account(account, |state| {
                state.nonce = 0;
                state.code_hash = EMPTY_STRING_KECCAK_HASH;
                state.deconstruct = false;
            });
            let access = self.tables.accounts[account as usize];
            for position in access.slots_start..access.slots_end {
                self.write_slot_value(position, Bytes32::ZERO);
            }
        }

        let index = self.current_index as usize;
        let declared = &self.changes
            [self.change_starts[index] as usize..self.change_starts[index + 1] as usize];
        let stamp = self.stamp;
        // every declared change happened, with the declared final value
        for change in declared {
            let ok = match change.change {
                Change::Balance { account, value } => {
                    let a = &self.accounts[account as usize];
                    a.changed == stamp && a.balance == value && a.balance != a.start_balance
                }
                Change::Nonce { account, value } => {
                    let a = &self.accounts[account as usize];
                    a.changed == stamp && a.nonce == value && a.nonce != a.start_nonce
                }
                Change::Code { account, hash } => {
                    let a = &self.accounts[account as usize];
                    a.changed == stamp && a.code_hash == hash && a.code_hash != a.start_code_hash
                }
                Change::Slot { position, value } => {
                    let s = &self.slots[position as usize];
                    s.written == stamp && s.current == value && s.current != s.start
                }
            };
            if !ok {
                return Err(internal_error!(
                    "a change declared by the block access list did not happen"
                ));
            }
        }
        // and nothing else did: the actual changes are as many as the declared ones. A
        // rollback restores an entry's stamp and start value from the index it was last
        // changed in, so only the entries still stamped with this index count.
        let mut actual = 0usize;
        for account in self.touched_accounts.iter() {
            let a = &self.accounts[*account as usize];
            if a.changed != stamp {
                continue;
            }
            actual += (a.balance != a.start_balance) as usize
                + (a.nonce != a.start_nonce) as usize
                + (a.code_hash != a.start_code_hash) as usize;
        }
        for position in self.touched_slots.iter() {
            let s = &self.slots[*position as usize];
            actual += (s.written == stamp && s.current != s.start) as usize;
        }
        if actual != declared.len() {
            return Err(self.undeclared_change_error(index as u32, declared.len(), actual));
        }
        // nothing of the index outlives it: the journal and the touched lists are
        // released here, so they hold nothing between transactions
        self.journal.clear();
        self.touched_accounts.clear();
        self.touched_slots.clear();
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
        for account in self.touched_accounts.iter() {
            let a = &self.accounts[*account as usize];
            if a.changed != self.stamp {
                continue;
            }
            let address = self.tables.addresses.key(*account).0;
            let fields: [(&'static str, bool, bool, Bytes32, Bytes32); 3] = [
                (
                    "balance",
                    a.balance != a.start_balance,
                    is_declared(
                        &|c| matches!(c, Change::Balance { account: x, .. } if *x == *account),
                    ),
                    Bytes32::from_array(a.start_balance.to_be_bytes()),
                    Bytes32::from_array(a.balance.to_be_bytes()),
                ),
                (
                    "nonce",
                    a.nonce != a.start_nonce,
                    is_declared(
                        &|c| matches!(c, Change::Nonce { account: x, .. } if *x == *account),
                    ),
                    Bytes32::from_array(U256::from(a.start_nonce).to_be_bytes()),
                    Bytes32::from_array(U256::from(a.nonce).to_be_bytes()),
                ),
                (
                    "code",
                    a.code_hash != a.start_code_hash,
                    is_declared(
                        &|c| matches!(c, Change::Code { account: x, .. } if *x == *account),
                    ),
                    a.start_code_hash,
                    a.code_hash,
                ),
            ];
            for (name, changed, declared, start, current) in fields {
                if changed != declared {
                    offending.push((address, name, start, current, Bytes32::ZERO));
                }
            }
        }
        for position in self.touched_slots.iter() {
            let s = &self.slots[*position as usize];
            let access = self.tables.slots[*position as usize];
            let address = self.tables.addresses.key(access.address_index).0;
            let key = self.tables.slot_keys.key(access.key_index).0;
            let declared =
                is_declared(&|c| matches!(c, Change::Slot { position: p, .. } if *p == *position));
            if (s.written == self.stamp && s.current != s.start) != declared {
                offending.push((address, "slot", s.start, s.current, key));
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

    /// Writes a slot value in the current index (journaled)
    fn write_slot_value(&mut self, position: u32, value: Bytes32) {
        let old = self.slots[position as usize];
        self.journal.push(JournalEntry::Slot { position, old });
        let state = &mut self.slots[position as usize];
        if state.written != self.stamp {
            state.written = self.stamp;
            state.start = old.current;
            self.touched_slots.push(position);
        }
        state.current = value;
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
            journal_len: self.journal.len(),
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
        // An entry first changed in the frame goes back to unchanged in this index
        // (its stamp is restored), so it leaves the touched list too: a later change
        // in the index records it again, and the list would count it twice. The
        // first changes of the frame are the tail of the list, in journal order.
        while self.journal.len() > snapshot.journal_len {
            let entry = self.journal.pop().expect("journal entry");
            match entry {
                JournalEntry::AccountWarm { account, old } => {
                    self.accounts[account as usize].warm = old;
                }
                JournalEntry::Account { account, old } => {
                    if old.changed != self.stamp {
                        let last = self.touched_accounts.pop();
                        debug_assert_eq!(last, Some(account));
                    }
                    self.accounts[account as usize] = old;
                }
                JournalEntry::SlotWarm { position, old } => {
                    self.slots[position as usize].warm = old;
                }
                JournalEntry::Slot { position, old } => {
                    if old.written != self.stamp {
                        let last = self.touched_slots.pop();
                        debug_assert_eq!(last, Some(position));
                    }
                    self.slots[position as usize] = old;
                }
            }
        }
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
        Ok(self.slots[position as usize].current)
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
        self.slots[position as usize].ever_accessed = true;
        let state = self.slots[position as usize];
        if state.warm != self.stamp {
            self.journal.push(JournalEntry::SlotWarm {
                position,
                old: state.warm,
            });
            self.slots[position as usize].warm = self.stamp;
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
        let state = self.slots[position as usize];
        let at_tx_start = self.slot_value_at_index_start(position);
        let current = state.current;
        let is_cold_write_charged = state.write_charged == self.stamp;
        let is_new_slot = state.is_new && state.write_charged == 0;
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
        self.write_slot_value(position, *new_value);
        if !is_cold_write_charged && *new_value != current {
            self.slots[position as usize].write_charged = self.stamp;
        }
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
        let state = self.accounts[account as usize];

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

    /// Warms an account (a transaction access list entry, the coinbase). A warm-up is
    /// not an access: an address outside the block access list was never reached by
    /// the execution, so there is nothing to warm (see `storage_touch`). A precompile is
    /// an account like any other here (it can be called and receive value); it is only
    /// never cold, see `access_account`.
    fn touch_account(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        resources: &mut R,
        address: &B160,
        _oracle: &mut impl IOOracle,
    ) -> Result<()> {
        let Some(account) = self.tables.addresses.get(&AddressKey(*address)) else {
            return Ok(());
        };
        self.access_account(ee_type, resources, address, account, false)
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
        let nonce = self.accounts[account as usize].nonce;
        let Some(new_nonce) = nonce.checked_add(increment_by) else {
            return Err(interface_error!(NonceError::NonceOverflow));
        };
        self.mutate_account(account, |state| state.nonce = new_nonce);
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
        let current = self.accounts[account as usize].balance;
        let new = update_fn(&current)?;
        self.mutate_account(account, |state| state.balance = new);
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
        Ok(self.accounts[account as usize].balance)
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
        let stamp = self.stamp;
        self.mutate_account(account, |state| {
            state.code_hash = hash;
            state.deployed = stamp;
        });
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
        self.mutate_account(account, |state| state.code_hash = hash);
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
        let state = self.accounts[account as usize];
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
            let b = &self.accounts[beneficiary as usize];
            let beneficiary_alive =
                b.nonce != 0 || b.code_hash != EMPTY_STRING_KECCAK_HASH || !b.balance.is_zero();
            if !beneficiary_alive && !transfer_amount.is_zero() {
                resources.charge_legacy_gas(ACCOUNT_WRITE)?;
                resources.charge_state_gas(NEW_ACCOUNT)?;
            }
        }

        // EIP-6780: only a contract created in this transaction is destroyed (a self-destruct
        // with no code yet is in its constructor)
        let should_be_deconstructed =
            state.deployed == self.stamp || state.code_hash == EMPTY_STRING_KECCAK_HASH;
        if should_be_deconstructed {
            self.mutate_account(account, |state| state.deconstruct = true);
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
            current_value: state.current,
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
                    current_value: state.current,
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
