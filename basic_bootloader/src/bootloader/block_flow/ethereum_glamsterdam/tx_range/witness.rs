//! What a transaction range witnesses before its transactions run: the
//! transaction range (transaction numbers, half-open), the number of
//! transactions of the block (unverified here: the aggregation requires it to
//! be the one the pre-execution check proved), and the initial value tables of
//! the pre-execution check (`state_consistency`), whose hashes link the two.
//! The block access index range the range covers follows from the first two
//! ([`BlockAccessIndexRange::of`]).

use super::super::index_mask::IndexMask;
use super::super::GlamsterdamError;
use super::index_trie;
use super::oracle_queries::*;
use crate::bootloader::transaction_flow::logs_bloom::LogsBloom;
use basic_system::system_implementation::ethereum_storage_model::{
    BoxInterner, MptPreimagesOracle,
};
use core::alloc::Allocator;
use crypto::sha3::Keccak256;
use crypto::MiniDigest;
use zk_ee::oracle::memory_io::{
    get_bytes_with_length_query, ContinuousDeserializable, ContinuousSerializable, OracleQuery,
};
use zk_ee::oracle::IOOracle;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::utils::{Bytes32, UsizeAlignedByteBox};

type Result<T> = core::result::Result<T, GlamsterdamError>;

/// A half-open range of transaction numbers `start..end`: what a range program
/// validates. A range starting at `0` also runs the pre-transaction system
/// phase, a range ending at the number of transactions of the block also runs
/// the post-transaction phase (the withdrawals and the request system calls):
/// the phases go with the first and the last transaction, so that no range can
/// cover the last transaction without the post-transaction phase.
/// `#[repr(C)]`: the oracle writes it memcpy-like.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TransactionRange {
    pub start: u32,
    pub end: u32,
}

const _: () = {
    assert!(core::mem::size_of::<TransactionRange>() == 8);
    assert!(core::mem::align_of::<TransactionRange>() == 4);
};

// SAFETY: two `u32`, no padding
unsafe impl ContinuousSerializable for TransactionRange {}

// SAFETY: any bytes form a valid value; the range is checked against the
// transaction count by `BlockAccessIndexRange::of`
unsafe impl ContinuousDeserializable for TransactionRange {
    #[inline(always)]
    unsafe fn validate<'a>(this: *mut Self) -> core::result::Result<&'a mut Self, InternalError> {
        // SAFETY: guaranteed by the caller
        Ok(unsafe { &mut *this })
    }
}

impl TransactionRange {
    /// Whether the pre-transaction system phase is part of the range
    pub fn includes_pre_transactions(&self) -> bool {
        self.start == 0
    }

    /// Whether the post-transaction phase is part of the range, given the
    /// number of transactions
    pub fn includes_post_transactions(&self, transaction_count: u32) -> bool {
        self.end == transaction_count
    }

    /// The transaction numbers of the range
    pub fn transactions(&self) -> core::ops::Range<u32> {
        self.start..self.end.max(self.start)
    }

    /// The number of transactions of the range
    pub fn len(&self) -> u32 {
        self.end.saturating_sub(self.start)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Checks the witnessed range against the transaction count: it must be
    /// ordered, within the block, and hold a transaction, except that a
    /// block without transactions is covered by the one range `0..0`, which
    /// runs both system phases. An empty range inside a block is rejected:
    /// it would either make a phase run twice (`0..0` or `n..n` next to the
    /// range that runs it) or validate nothing.
    pub fn check(&self, transaction_count: u32) -> Result<()> {
        if self.start > self.end || self.end > transaction_count {
            return Err(GlamsterdamError::TransactionRange);
        }
        if self.is_empty() && !(self.start == 0 && transaction_count == 0) {
            return Err(GlamsterdamError::TransactionRange);
        }
        Ok(())
    }
}

/// An inclusive range of block access indices: `0` is the pre-transaction
/// system phase, `1..=n` the transactions, `n + 1` the post-transaction phase.
/// Derived from a [`TransactionRange`] and the transaction count, never
/// witnessed on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockAccessIndexRange {
    pub first: u32,
    pub last: u32,
}

impl BlockAccessIndexRange {
    /// The access indices a transaction range covers: its transactions, the
    /// pre-transaction phase when it starts at the first transaction, the
    /// post-transaction phase when it ends at the last. The range must be
    /// within the block and hold a transaction, except that a block without
    /// transactions is covered by the one range `0..0`, which runs both
    /// phases: an empty range inside a block would make a phase run twice.
    pub fn of(range: TransactionRange, transaction_count: u32) -> Result<Self> {
        range.check(transaction_count)?;
        let first = if range.includes_pre_transactions() {
            0
        } else {
            range.start + 1
        };
        let last = if range.includes_post_transactions(transaction_count) {
            transaction_count + 1
        } else {
            range.end
        };
        Ok(Self { first, last })
    }
}

/// The block's running accounting after some of its transactions, as the receipts and
/// the header count it: the receipts' cumulative gas (after refunds and the floor), the
/// block's execution and state gas (EIP-8037 `block_execution_gas_used`,
/// `block_state_gas_used`), the blob gas and the logs bloom of the transactions so
/// far. A range starts from the accounting after the transactions before it, witnessed
/// and unverified (nothing before the first transaction), ends with its own and
/// publishes both (the bloom hashed): the aggregation links a range's start to the
/// previous range's end. No range opens the receipt of the transaction before it, which
/// is as long as that transaction's logs, to learn where it starts.
/// `#[repr(C)]`: the oracle writes it memcpy-like.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BlockProgress {
    pub cumulative_gas_used: u64,
    pub execution_gas_used: u64,
    pub state_gas_used: u64,
    pub blob_gas_used: u64,
    pub logs_bloom: LogsBloom,
}

const _: () = {
    assert!(core::mem::size_of::<BlockProgress>() == 4 * 8 + 256);
    assert!(core::mem::offset_of!(BlockProgress, logs_bloom) == 32);
};

impl BlockProgress {
    /// The block's gas used as the header counts it (EIP-8037): the larger of
    /// the execution and the state gas
    pub fn block_gas_used(&self) -> u64 {
        self.execution_gas_used.max(self.state_gas_used)
    }
}

// SAFETY: four `u64` then 256 bytes, no padding
unsafe impl ContinuousSerializable for BlockProgress {}

// SAFETY: any bytes form a valid value (the bloom is a plain byte array)
unsafe impl ContinuousDeserializable for BlockProgress {
    #[inline(always)]
    unsafe fn validate<'a>(this: *mut Self) -> core::result::Result<&'a mut Self, InternalError> {
        // SAFETY: guaranteed by the caller
        Ok(unsafe { &mut *this })
    }
}

pub struct BlockProgressQuery;

impl OracleQuery for BlockProgressQuery {
    const QUERY_ID: u32 = GLAMSTERDAM_BLOCK_PROGRESS_QUERY_ID;
    type Input = ();
    type Output = BlockProgress;
}

pub struct TransactionRangeQuery;

impl OracleQuery for TransactionRangeQuery {
    const QUERY_ID: u32 = GLAMSTERDAM_TRANSACTION_RANGE_QUERY_ID;
    type Input = ();
    type Output = TransactionRange;
}

pub struct TransactionCountQuery;

impl OracleQuery for TransactionCountQuery {
    const QUERY_ID: u32 = GLAMSTERDAM_TRANSACTION_COUNT_QUERY_ID;
    type Input = ();
    type Output = u32;
}

/// Witnesses the number of transactions of the block, unverified: what a
/// range runs with, bound to the pre-execution check's by the aggregation
pub fn witness_transaction_count<O: IOOracle>(oracle: &mut O) -> Result<u32> {
    TransactionCountQuery::get(oracle, ()).map_err(|_| GlamsterdamError::Oracle)
}

/// Capacity of the interner of the count proof (see `witness_proven_transaction_count`)
const COUNT_PROOF_INTERNER_CAPACITY: usize = 1 << 22;

/// Witnesses the number of transactions of the block and proves it against
/// the transactions and the receipts tries, as the pre-execution check does
/// (see `index_trie::verify_count` for what that proves and what it leaves
/// to the ranges)
pub fn witness_proven_transaction_count<O: IOOracle, A: Allocator + Clone>(
    oracle: &mut O,
    transactions_root: &Bytes32,
    receipts_root: &Bytes32,
    allocator: A,
) -> Result<u32> {
    let count = witness_transaction_count(oracle)?;
    // one path per trie (at most 14 nodes of at most 17 * 33 bytes each) and the leaf at
    // its end, which is interned too: a whole transaction (at most 128 KiB) or receipt
    // (its logs can reach 2 MiB under the transaction gas cap)
    let mut interner = BoxInterner::with_capacity_in(COUNT_PROOF_INTERNER_CAPACITY, allocator);
    let mut preimages = MptPreimagesOracle(oracle);
    let mut hasher = Keccak256::new();
    index_trie::verify_count(
        transactions_root,
        count,
        &mut preimages,
        &mut interner,
        &mut hasher,
    )?;
    index_trie::verify_count(
        receipts_root,
        count,
        &mut preimages,
        &mut interner,
        &mut hasher,
    )?;
    Ok(count)
}

/// Witnesses the transaction range of the program
pub fn witness_transaction_range<O: IOOracle>(oracle: &mut O) -> Result<TransactionRange> {
    TransactionRangeQuery::get(oracle, ()).map_err(|_| GlamsterdamError::Oracle)
}

/// The initial value tables of the pre-execution check, as witnessed: the
/// streams in index order, so an entry follows the previous one (see
/// `state_consistency`)
pub struct InitialValues<A: Allocator> {
    /// Account leaf stream: entries `length byte || RLP item`, by address index
    pub accounts: UsizeAlignedByteBox<A>,
    /// The accounts without a leaf before the block, by address index
    pub fresh_accounts: IndexMask<A>,
    /// Slot leaf stream, by slot position
    pub slots: UsizeAlignedByteBox<A>,
    /// keccak of each table as witnessed (`accounts`, `fresh_accounts`,
    /// `slots`): what links these tables to the pre-execution check
    pub hashes: [Bytes32; 3],
    num_accounts: u32,
    num_slots: u32,
}

fn witness_buffer<O: IOOracle, A: Allocator>(
    oracle: &mut O,
    len_query: u32,
    data_query: u32,
    allocator: A,
) -> Result<UsizeAlignedByteBox<A>> {
    get_bytes_with_length_query::<_, (), _>(oracle, len_query, data_query, (), allocator)
        .map_err(|_| GlamsterdamError::Oracle)?
        .ok_or(GlamsterdamError::InitialValuesEncoding)
}

/// The entries of a stream, `None` for an entry of length zero
pub struct Entries<'a> {
    stream: &'a [u8],
}

impl<'a> Iterator for Entries<'a> {
    type Item = Option<&'a [u8]>;

    fn next(&mut self) -> Option<Self::Item> {
        let (len, rest) = self.stream.split_first()?;
        let len = *len as usize;
        // a validated stream never ends inside an entry; a malformed one just ends
        let (item, rest) = rest.split_at_checked(len)?;
        self.stream = rest;
        Some((len != 0).then_some(item))
    }
}

/// The number of entries of a stream, which must end exactly after its last one
fn count_entries(stream: &[u8]) -> Result<u32> {
    let mut entries = Entries { stream };
    let mut count = 0usize;
    while entries.next().is_some() {
        count += 1;
    }
    if !entries.stream.is_empty() || count >= u32::MAX as usize {
        return Err(GlamsterdamError::InitialValuesEncoding);
    }
    Ok(count as u32)
}

impl<A: Allocator + Clone> InitialValues<A> {
    pub fn witness<O: IOOracle>(oracle: &mut O, allocator: A) -> Result<Self> {
        let pairs = [
            (
                GLAMSTERDAM_INITIAL_ACCOUNTS_LEN_QUERY_ID,
                GLAMSTERDAM_INITIAL_ACCOUNTS_DATA_QUERY_ID,
            ),
            (
                GLAMSTERDAM_FRESH_ACCOUNTS_LEN_QUERY_ID,
                GLAMSTERDAM_FRESH_ACCOUNTS_DATA_QUERY_ID,
            ),
            (
                GLAMSTERDAM_INITIAL_SLOTS_LEN_QUERY_ID,
                GLAMSTERDAM_INITIAL_SLOTS_DATA_QUERY_ID,
            ),
        ];
        let mut hasher = Keccak256::new();
        let mut buffers =
            pairs.map(|(len, data)| witness_buffer(oracle, len, data, allocator.clone()));
        let mut hashes = [Bytes32::ZERO; 3];
        for (hash, buffer) in hashes.iter_mut().zip(buffers.iter_mut()) {
            let buffer = buffer.as_ref().map_err(|e| *e)?;
            hasher.update(buffer.as_slice());
            *hash = hasher.finalize_reset().into();
        }
        let [accounts, fresh_accounts, slots] = buffers;
        let accounts = accounts?;
        let slots = slots?;
        let num_accounts = count_entries(accounts.as_slice())?;
        let num_slots = count_entries(slots.as_slice())?;
        let fresh_accounts =
            IndexMask::from_bytes(num_accounts as usize, fresh_accounts?.as_slice(), allocator)?;
        Ok(Self {
            accounts,
            fresh_accounts,
            slots,
            hashes,
            num_accounts,
            num_slots,
        })
    }

    pub fn num_accounts(&self) -> usize {
        self.num_accounts as usize
    }

    pub fn num_slots(&self) -> usize {
        self.num_slots as usize
    }

    /// The RLP items of the pre-block leaves of the accounts, by address
    /// index, `None` without a leaf
    pub fn accounts(&self) -> Entries<'_> {
        Entries {
            stream: self.accounts.as_slice(),
        }
    }

    /// The RLP items of the pre-block leaves of the slots, by position,
    /// `None` without a leaf
    pub fn slots(&self) -> Entries<'_> {
        Entries {
            stream: self.slots.as_slice(),
        }
    }

    /// Whether the account had no leaf before the block
    pub fn is_fresh(&self, address_index: u32) -> bool {
        self.fresh_accounts.get(address_index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transaction_ranges_are_checked_against_the_count() {
        let range = |start, end| TransactionRange { start, end };
        let indices = |first, last| BlockAccessIndexRange { first, last };
        // the whole block, a first, a middle and a last range
        assert_eq!(
            BlockAccessIndexRange::of(range(0, 10), 10),
            Ok(indices(0, 11))
        );
        assert_eq!(
            BlockAccessIndexRange::of(range(0, 4), 10),
            Ok(indices(0, 4))
        );
        assert_eq!(
            BlockAccessIndexRange::of(range(4, 7), 10),
            Ok(indices(5, 7))
        );
        assert_eq!(
            BlockAccessIndexRange::of(range(7, 10), 10),
            Ok(indices(8, 11))
        );
        assert_eq!(range(0, 1).transactions(), 0..1);
        assert_eq!(range(7, 10).transactions(), 7..10);
        // the empty block is the one range 0..0, running both phases
        assert_eq!(BlockAccessIndexRange::of(range(0, 0), 0), Ok(indices(0, 1)));
        // empty ranges inside a block: a phase alone, or nothing at all
        for empty in [range(0, 0), range(4, 4), range(10, 10)] {
            assert_eq!(
                empty.check(10),
                Err(GlamsterdamError::TransactionRange),
                "{empty:?}"
            );
            assert!(BlockAccessIndexRange::of(empty, 10).is_err());
        }
        assert!(range(1, 1).check(0).is_err());
        // outside the block, or reversed
        assert!(range(0, 11).check(10).is_err());
        assert!(range(10, 11).check(10).is_err());
        assert!(range(5, 4).check(10).is_err());
        assert!(range(1, 0).check(0).is_err());
        assert_eq!(range(5, 4).len(), 0);
    }
}
