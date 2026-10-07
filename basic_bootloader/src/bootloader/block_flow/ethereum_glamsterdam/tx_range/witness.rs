//! What a transaction range witnesses before its transactions run: the block
//! access index range, the number of transactions of the block (proven
//! against the transactions trie), and the initial value tables of the
//! pre-execution check (`state_consistency`), whose hashes link the two.

use super::super::index_mask::IndexMask;
use super::super::GlamsterdamError;
use super::index_trie;
use super::oracle_queries::*;
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

/// An inclusive range of block access indices: `0` is the pre-transaction
/// system phase, `1..=n` the transactions, `n + 1` the post-transaction phase.
/// `#[repr(C)]`: the oracle writes it memcpy-like.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockAccessIndexRange {
    pub first: u32,
    pub last: u32,
}

const _: () = {
    assert!(core::mem::size_of::<BlockAccessIndexRange>() == 8);
    assert!(core::mem::align_of::<BlockAccessIndexRange>() == 4);
};

// SAFETY: two `u32`, no padding
unsafe impl ContinuousSerializable for BlockAccessIndexRange {}

// SAFETY: any bytes form a valid value; the range itself is checked by `witness_range`
unsafe impl ContinuousDeserializable for BlockAccessIndexRange {
    #[inline(always)]
    unsafe fn validate<'a>(this: *mut Self) -> core::result::Result<&'a mut Self, InternalError> {
        // SAFETY: guaranteed by the caller
        Ok(unsafe { &mut *this })
    }
}

impl BlockAccessIndexRange {
    /// Whether the pre-transaction system phase is part of the range
    pub fn includes_pre_transactions(&self) -> bool {
        self.first == 0
    }

    /// Whether the post-transaction phase is part of the range, given the
    /// number of transactions
    pub fn includes_post_transactions(&self, transaction_count: u32) -> bool {
        self.last == transaction_count + 1
    }

    /// The transaction indices (zero-based) of the range
    pub fn transactions(&self, transaction_count: u32) -> core::ops::Range<u32> {
        let first = self.first.max(1) - 1;
        let end = self.last.min(transaction_count);
        first..end.max(first)
    }
}

/// The block's gas accounting at the start of a range: what the ranges before it used
/// (EIP-8037 `block_execution_gas_used`, `block_state_gas_used`, the blob gas).
/// `#[repr(C)]`: the oracle writes it memcpy-like.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RangeStartGas {
    pub execution_gas_used: u64,
    pub state_gas_used: u64,
    pub blob_gas_used: u64,
}

const _: () = {
    assert!(core::mem::size_of::<RangeStartGas>() == 24);
};

impl RangeStartGas {
    /// The block's gas used as the header counts it (EIP-8037): the larger of
    /// the execution and the state gas
    pub fn block_gas_used(&self) -> u64 {
        self.execution_gas_used.max(self.state_gas_used)
    }
}

// SAFETY: three `u64`, no padding
unsafe impl ContinuousSerializable for RangeStartGas {}

// SAFETY: any bytes form a valid value
unsafe impl ContinuousDeserializable for RangeStartGas {
    #[inline(always)]
    unsafe fn validate<'a>(this: *mut Self) -> core::result::Result<&'a mut Self, InternalError> {
        // SAFETY: guaranteed by the caller
        Ok(unsafe { &mut *this })
    }
}

pub struct RangeStartGasQuery;

impl OracleQuery for RangeStartGasQuery {
    const QUERY_ID: u32 = GLAMSTERDAM_RANGE_START_GAS_QUERY_ID;
    type Input = ();
    type Output = RangeStartGas;
}

pub struct AccessIndexRangeQuery;

impl OracleQuery for AccessIndexRangeQuery {
    const QUERY_ID: u32 = GLAMSTERDAM_ACCESS_INDEX_RANGE_QUERY_ID;
    type Input = ();
    type Output = BlockAccessIndexRange;
}

pub struct TransactionCountQuery;

impl OracleQuery for TransactionCountQuery {
    const QUERY_ID: u32 = GLAMSTERDAM_TRANSACTION_COUNT_QUERY_ID;
    type Input = ();
    type Output = u32;
}

/// Witnesses the number of transactions of the block and proves it against the
/// transactions trie: the last index has a leaf, the next one has none
pub fn witness_transaction_count<O: IOOracle, A: Allocator + Clone>(
    oracle: &mut O,
    transactions_root: &Bytes32,
    allocator: A,
) -> Result<u32> {
    let count = TransactionCountQuery::get(oracle, ()).map_err(|_| GlamsterdamError::Oracle)?;
    // the two paths are at most 14 nodes of at most 17 * 33 bytes
    let mut interner = BoxInterner::with_capacity_in(1 << 14, allocator);
    let mut preimages = MptPreimagesOracle(oracle);
    let mut hasher = Keccak256::new();
    index_trie::verify_count(
        transactions_root,
        count,
        &mut preimages,
        &mut interner,
        &mut hasher,
    )?;
    Ok(count)
}

/// Witnesses the block access index range, checked against the transaction count
pub fn witness_range<O: IOOracle>(
    oracle: &mut O,
    transaction_count: u32,
) -> Result<BlockAccessIndexRange> {
    let range = AccessIndexRangeQuery::get(oracle, ()).map_err(|_| GlamsterdamError::Oracle)?;
    if range.first > range.last || range.last > transaction_count + 1 {
        return Err(GlamsterdamError::AccessIndexRange);
    }
    Ok(range)
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
