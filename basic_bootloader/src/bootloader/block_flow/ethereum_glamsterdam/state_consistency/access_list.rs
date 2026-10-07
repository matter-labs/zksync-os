//! Lazy reader of the RLP encoding of an EIP-7928 block access list.
//!
//! Nothing is materialized: every reader is a cursor over the payload of one
//! RLP list, and the caller pulls the entries in order. The readers check the
//! encoding only; the ordering and uniqueness rules are checked by the caller
//! as it consumes the entries (see `collect`).
//!
//! ```text
//! BlockAccessList = [AccountChanges, ...]
//! AccountChanges  = [address, [SlotChanges, ...], [slot, ...], [BalanceChange, ...],
//!                    [NonceChange, ...], [CodeChange, ...]]
//! SlotChanges     = [slot, [[block_access_index, new_value], ...]]
//! BalanceChange   = [block_access_index, post_balance]
//! NonceChange     = [block_access_index, new_nonce]
//! CodeChange      = [block_access_index, new_code]
//! ```

use super::super::GlamsterdamError;
use crate::bootloader::errors::InvalidTransaction;
use crate::bootloader::transaction::rlp_encoded::rlp::minimal_rlp_parser::Rlp;
use ruint::aliases::B160;
use zk_ee::utils::Bytes32;

impl From<InvalidTransaction> for GlamsterdamError {
    fn from(_: InvalidTransaction) -> Self {
        GlamsterdamError::AccessListEncoding
    }
}

type Result<T> = core::result::Result<T, GlamsterdamError>;

/// Decodes a minimal big-endian integer of at most 32 bytes into its
/// big-endian 32-byte form
fn bytes32(r: &mut Rlp<'_>) -> Result<Bytes32> {
    let digits = r.bytes()?;
    if digits.len() > 32 || digits.first() == Some(&0) {
        return Err(GlamsterdamError::AccessListEncoding);
    }
    let mut value = Bytes32::ZERO;
    value.as_u8_array_mut()[32 - digits.len()..].copy_from_slice(digits);
    Ok(value)
}

/// The RLP item (header and digits) of a minimal big-endian integer of at
/// most `max_len` bytes, left in place
fn integer_item<'a>(r: &mut Rlp<'a>, max_len: usize) -> Result<&'a [u8]> {
    let mark = r.mark();
    let digits = r.bytes()?;
    if digits.len() > max_len || digits.first() == Some(&0) {
        return Err(GlamsterdamError::AccessListEncoding);
    }
    Ok(r.consumed_since(mark))
}

/// Decodes a block access index
fn access_index(r: &mut Rlp<'_>) -> Result<u32> {
    u32::try_from(r.u64()?).map_err(|_| GlamsterdamError::AccessListValue)
}

/// Enters a list that must be the only remaining item of `r`
fn last_list<'a>(r: &mut Rlp<'a>) -> Result<Rlp<'a>> {
    let list = r.list()?;
    if !r.is_empty() {
        return Err(GlamsterdamError::AccessListEncoding);
    }
    Ok(list)
}

/// Reader of a whole block access list (not a per-transaction EIP-2930 one)
#[derive(Clone, Copy)]
pub struct BlockAccessListReader<'a> {
    accounts: Rlp<'a>,
}

impl<'a> BlockAccessListReader<'a> {
    pub fn new(encoding: &'a [u8]) -> Result<Self> {
        let mut r = Rlp::new(encoding);
        let accounts = last_list(&mut r)?;
        Ok(Self { accounts })
    }

    pub fn is_empty(&self) -> bool {
        self.accounts.is_empty()
    }

    /// The next account, with its change lists still unread
    pub fn next_account(&mut self) -> Result<Option<AccountChangesReader<'a>>> {
        if self.accounts.is_empty() {
            return Ok(None);
        }
        let mut r = self.accounts.list()?;
        let address = r.b160()?;
        let storage_changes = r.list()?;
        let storage_reads = r.list()?;
        let balance_changes = r.list()?;
        let nonce_changes = r.list()?;
        let code_changes = last_list(&mut r)?;
        Ok(Some(AccountChangesReader {
            address,
            storage_changes,
            storage_reads,
            balance_changes,
            nonce_changes,
            code_changes,
        }))
    }
}

/// Reader of one `AccountChanges` entry. The lists are independent cursors, so
/// they can be consumed in any order.
#[derive(Clone, Copy)]
pub struct AccountChangesReader<'a> {
    pub address: B160,
    storage_changes: Rlp<'a>,
    storage_reads: Rlp<'a>,
    balance_changes: Rlp<'a>,
    nonce_changes: Rlp<'a>,
    code_changes: Rlp<'a>,
}

impl<'a> AccountChangesReader<'a> {
    pub fn next_slot_changes(&mut self) -> Result<Option<SlotChangesReader<'a>>> {
        if self.storage_changes.is_empty() {
            return Ok(None);
        }
        let mut r = self.storage_changes.list()?;
        let slot = bytes32(&mut r)?;
        let changes = last_list(&mut r)?;
        Ok(Some(SlotChangesReader { slot, changes }))
    }

    pub fn next_storage_read(&mut self) -> Result<Option<Bytes32>> {
        if self.storage_reads.is_empty() {
            return Ok(None);
        }
        Ok(Some(bytes32(&mut self.storage_reads)?))
    }

    /// `(block_access_index, post_balance)`, the balance as its RLP item
    /// (a minimal big-endian integer), as the account leaf stores it
    pub fn next_balance_change(&mut self) -> Result<Option<(u32, &'a [u8])>> {
        if self.balance_changes.is_empty() {
            return Ok(None);
        }
        let mut r = self.balance_changes.list()?;
        let index = access_index(&mut r)?;
        let balance = integer_item(&mut r, 32)?;
        if !r.is_empty() {
            return Err(GlamsterdamError::AccessListEncoding);
        }
        Ok(Some((index, balance)))
    }

    /// `(block_access_index, new_nonce)`, the nonce as its RLP item (a
    /// minimal big-endian integer), as the account leaf stores it
    pub fn next_nonce_change(&mut self) -> Result<Option<(u32, &'a [u8])>> {
        if self.nonce_changes.is_empty() {
            return Ok(None);
        }
        let mut r = self.nonce_changes.list()?;
        let index = access_index(&mut r)?;
        let nonce = integer_item(&mut r, 8)?;
        if !r.is_empty() {
            return Err(GlamsterdamError::AccessListEncoding);
        }
        Ok(Some((index, nonce)))
    }

    /// `(block_access_index, new_code)`
    pub fn next_code_change(&mut self) -> Result<Option<(u32, &'a [u8])>> {
        if self.code_changes.is_empty() {
            return Ok(None);
        }
        let mut r = self.code_changes.list()?;
        let index = access_index(&mut r)?;
        let code = r.bytes()?;
        if !r.is_empty() {
            return Err(GlamsterdamError::AccessListEncoding);
        }
        Ok(Some((index, code)))
    }
}

/// Reader of one `SlotChanges` entry
#[derive(Clone, Copy)]
pub struct SlotChangesReader<'a> {
    /// The slot key, big-endian
    pub slot: Bytes32,
    changes: Rlp<'a>,
}

impl<'a> SlotChangesReader<'a> {
    /// `(block_access_index, new_value)`, the value as its RLP item (a
    /// minimal big-endian integer), as the storage leaf stores it
    pub fn next_change(&mut self) -> Result<Option<(u32, &'a [u8])>> {
        if self.changes.is_empty() {
            return Ok(None);
        }
        let mut r = self.changes.list()?;
        let index = access_index(&mut r)?;
        let value = integer_item(&mut r, 32)?;
        if !r.is_empty() {
            return Err(GlamsterdamError::AccessListEncoding);
        }
        Ok(Some((index, value)))
    }
}

/// A source of block access list entries, consumed in order: the accounts, and
/// for every account its slot changes, storage reads, balance, nonce and code
/// changes, so the walk does not depend on the encoding. The RLP readers
/// above are the source of the RLP witness.
pub trait AccessListSource<'a> {
    type Account<'s>: AccountSource<'a>
    where
        Self: 's;

    fn next_account(&mut self) -> Result<Option<Self::Account<'_>>>;
}

pub trait AccountSource<'a> {
    type Slot<'s>: SlotSource<'a>
    where
        Self: 's;

    fn address(&self) -> B160;
    fn next_slot_changes(&mut self) -> Result<Option<Self::Slot<'_>>>;
    fn next_storage_read(&mut self) -> Result<Option<Bytes32>>;
    /// `(block_access_index, post_balance)`, the balance as its RLP item in the witness
    fn next_balance_change(&mut self) -> Result<Option<(u32, &'a [u8])>>;
    /// `(block_access_index, new_nonce)`, the nonce as its RLP item in the witness
    fn next_nonce_change(&mut self) -> Result<Option<(u32, &'a [u8])>>;
    /// `(block_access_index, new_code)`, the code living in the witness buffer
    fn next_code_change(&mut self) -> Result<Option<(u32, &'a [u8])>>;
}

pub trait SlotSource<'a> {
    /// The slot key, big-endian
    fn slot(&self) -> Bytes32;
    /// `(block_access_index, new_value)`, the value as its RLP item in the witness
    fn next_change(&mut self) -> Result<Option<(u32, &'a [u8])>>;
}

impl<'a> AccessListSource<'a> for BlockAccessListReader<'a> {
    type Account<'s>
        = AccountChangesReader<'a>
    where
        Self: 's;

    fn next_account(&mut self) -> Result<Option<Self::Account<'_>>> {
        BlockAccessListReader::next_account(self)
    }
}

impl<'a> AccountSource<'a> for AccountChangesReader<'a> {
    type Slot<'s>
        = SlotChangesReader<'a>
    where
        Self: 's;

    fn address(&self) -> B160 {
        self.address
    }
    fn next_slot_changes(&mut self) -> Result<Option<Self::Slot<'_>>> {
        AccountChangesReader::next_slot_changes(self)
    }
    fn next_storage_read(&mut self) -> Result<Option<Bytes32>> {
        AccountChangesReader::next_storage_read(self)
    }
    fn next_balance_change(&mut self) -> Result<Option<(u32, &'a [u8])>> {
        AccountChangesReader::next_balance_change(self)
    }
    fn next_nonce_change(&mut self) -> Result<Option<(u32, &'a [u8])>> {
        AccountChangesReader::next_nonce_change(self)
    }
    fn next_code_change(&mut self) -> Result<Option<(u32, &'a [u8])>> {
        AccountChangesReader::next_code_change(self)
    }
}

impl<'a> SlotSource<'a> for SlotChangesReader<'a> {
    fn slot(&self) -> Bytes32 {
        self.slot
    }
    fn next_change(&mut self) -> Result<Option<(u32, &'a [u8])>> {
        SlotChangesReader::next_change(self)
    }
}

/// Sizes of an access list, for allocating the tables up front
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AccessListCounts {
    pub accounts: usize,
    /// Written slots (`SlotChanges` entries) plus read slots, over all accounts
    pub slots: usize,
    /// The largest number of slots of one account
    pub max_slots_of_account: usize,
}

/// Walks a source once to count its entries. Only the encoding is checked here.
pub fn count<'a>(source: &mut impl AccessListSource<'a>) -> Result<AccessListCounts> {
    let mut counts = AccessListCounts::default();
    while let Some(mut account) = source.next_account()? {
        counts.accounts += 1;
        let mut slots = 0usize;
        while let Some(mut slot) = account.next_slot_changes()? {
            slots += 1;
            while slot.next_change()?.is_some() {}
        }
        while account.next_storage_read()?.is_some() {
            slots += 1;
        }
        while account.next_balance_change()?.is_some() {}
        while account.next_nonce_change()?.is_some() {}
        while account.next_code_change()?.is_some() {}
        counts.slots += slots;
        counts.max_slots_of_account = counts.max_slots_of_account.max(slots);
    }
    Ok(counts)
}
