//! The linkage between the programs of a Glamsterdam block: the pre-execution
//! check and the transaction ranges each publish the hash of one
//! [`ProgramOutput`], a plain dense structure, as their public output. The
//! fields that identify the block, the initial value tables and the
//! transaction count must agree across the programs; `bal_mask` commits to
//! the accounts and slots of the block access list a program accessed (all
//! clear for the pre-execution check, which executes nothing), and
//! `num_unique_bal_records` is the length of that mask; `tx_start_mark` and
//! `tx_end_mark` are the transaction range a program covered (the whole
//! block for the pre-execution check); `deposit_requests_hasher_start` and
//! `deposit_requests_hasher_end` are the sha256 states of the deposit request
//! hashing a range started from and ended with (`tx_range::requests`), all
//! zero for the pre-execution check; the gas sums (execution, state and blob
//! gas, EIP-8037 and EIP-4844) and the receipts' cumulative gas a range
//! started from and ended with, and the header's gas used; and the hashes of
//! the block's logs bloom a range started from and ended with (the empty
//! bloom's and the header's for the pre-execution check). The start values
//! are what a range witnesses as the accounting after the transactions
//! before it (`tx_range::witness::BlockProgress`), so that no range reads
//! the receipt of the transaction before it. An aggregation program
//! ([`LinkageAggregation`], run by `block_aggregation`) takes the outputs'
//! preimages and the masks' preimages, checks them against the public
//! outputs, requires the shared fields to agree, the ranges to tile the
//! block's transactions in order (so that the range ending at the
//! transaction count, which checked the header's requests hash, is the last
//! one), the hasher states, the gas sums, the cumulative gas and the bloom
//! hashes to chain from the first range through every range, the final gas
//! sums and bloom to be the header's (EIP-7778: the gas used is the larger
//! of the execution and the state gas), and the union of the masks to be
//! full: every record of the access list is accessed somewhere in the block.

use super::index_mask::IndexMask;
use super::state_consistency::StateConsistencyOutput;
use super::tx_range::block_flow::GlamsterdamRangeOutput;
use super::tx_range::requests::{Sha256Accumulator, SHA256_ACCUMULATOR_LEN};
use super::GlamsterdamError;
use crate::bootloader::transaction_flow::logs_bloom::LogsBloom;
use core::alloc::Allocator;
use crypto::sha3::Keccak256;
use crypto::MiniDigest;
use zk_ee::utils::Bytes32;

type Result<T> = core::result::Result<T, GlamsterdamError>;

/// What every program of a block publishes, hashed. `#[repr(C)]` and dense
/// (asserted below): the hash is taken over the structure's own bytes, the
/// count little-endian, as [`ProgramOutput::as_bytes`] exposes them.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProgramOutput {
    /// The header hash
    pub block_hash: [u8; 32],
    /// keccak of the pre-block account leaf table (`InitialValues::accounts`)
    pub initial_accounts: [u8; 32],
    /// keccak of the pre-block slot leaf table (`InitialValues::slots`)
    pub initial_slots: [u8; 32],
    /// keccak of the fresh account mask (`InitialValues::fresh_accounts`)
    pub fresh_accounts_mask: [u8; 32],
    /// keccak of the mask of the access list records the program accessed
    /// (the accounts by address index, then the slots by position). A pure
    /// marker for the pre-execution check, which accesses nothing: all zero.
    pub bal_mask: [u8; 32],
    /// The sha256 state of the deposit request hashing (EIP-6110 over the
    /// block's logs, EIP-7685 type 0) a range started from and ended with:
    /// the hasher's words and buffer as bytes (`Sha256Accumulator`). All
    /// zero for the pre-execution check, which hashes nothing.
    pub deposit_requests_hasher_start: [u8; SHA256_ACCUMULATOR_LEN],
    pub deposit_requests_hasher_end: [u8; SHA256_ACCUMULATOR_LEN],
    /// The number of records of the access list (accounts and slots): the
    /// length of a range's mask
    pub num_unique_bal_records: u32,
    /// The number of transactions of the block: proven by the pre-execution
    /// check, witnessed by a range, which runs with it (the post-transaction
    /// phase and the index trie checks depend on it)
    pub tx_count: u32,
    /// The transactions the program covered, `tx_start_mark..tx_end_mark`:
    /// `0..tx_count` for the pre-execution check; for a range, its
    /// transaction range, which runs the pre-transaction phase when it starts
    /// at 0 and the post-transaction phase when it ends at `tx_count`
    pub tx_start_mark: u32,
    pub tx_end_mark: u32,
    /// The block's execution and state gas (EIP-8037) before and after the
    /// transactions a range covered: a range's sums link to its neighbours'.
    /// All zero for the pre-execution check, which publishes the header's gas
    /// used instead.
    pub execution_gas_start: u64,
    pub execution_gas_end: u64,
    pub state_gas_start: u64,
    pub state_gas_end: u64,
    /// The header's gas used, published by every program (EIP-7778: the larger
    /// of the block's execution and state gas)
    pub header_gas_used: u64,
    /// The block's blob gas (EIP-4844) before and after the transactions a
    /// range covered; `0` and the header's blob gas used for the pre-execution
    /// check
    pub blob_gas_start: u64,
    pub blob_gas_end: u64,
    /// The receipts' cumulative gas (after refunds and the floor, which the
    /// header does not carry) before and after the transactions a range
    /// covered: a range checks every receipt of its transactions against the
    /// receipts trie from its start, which the chain binds to the previous
    /// range's end. Both zero for the pre-execution check.
    pub cumulative_gas_start: u64,
    pub cumulative_gas_end: u64,
    /// keccak of the block's logs bloom before and after the transactions a
    /// range covered (the bloom of the transactions so far): the empty
    /// bloom's and the header's for the pre-execution check
    pub logs_bloom_start: [u8; 32],
    pub logs_bloom_end: [u8; 32],
}

/// The length of the structure, which is its own encoding
pub const ENCODING_LEN: usize = 5 * 32 + 2 * SHA256_ACCUMULATOR_LEN + 4 * 4 + 9 * 8 + 2 * 32;

// Dense: the byte arrays are 1-aligned, the counts and the sums sit right after
// them at their alignment, so there is no padding anywhere and every byte is
// initialized.
const _: () = {
    assert!(core::mem::size_of::<ProgramOutput>() == ENCODING_LEN);
    assert!(core::mem::align_of::<ProgramOutput>() == 8);
    assert!(core::mem::offset_of!(ProgramOutput, block_hash) == 0);
    assert!(core::mem::offset_of!(ProgramOutput, initial_accounts) == 32);
    assert!(core::mem::offset_of!(ProgramOutput, initial_slots) == 64);
    assert!(core::mem::offset_of!(ProgramOutput, fresh_accounts_mask) == 96);
    assert!(core::mem::offset_of!(ProgramOutput, bal_mask) == 128);
    assert!(core::mem::offset_of!(ProgramOutput, deposit_requests_hasher_start) == 160);
    assert!(core::mem::offset_of!(ProgramOutput, deposit_requests_hasher_end) == 264);
    assert!(core::mem::offset_of!(ProgramOutput, num_unique_bal_records) == 368);
    assert!(core::mem::offset_of!(ProgramOutput, tx_count) == 372);
    assert!(core::mem::offset_of!(ProgramOutput, tx_start_mark) == 376);
    assert!(core::mem::offset_of!(ProgramOutput, tx_end_mark) == 380);
    assert!(core::mem::offset_of!(ProgramOutput, execution_gas_start) == 384);
    assert!(core::mem::offset_of!(ProgramOutput, execution_gas_end) == 392);
    assert!(core::mem::offset_of!(ProgramOutput, state_gas_start) == 400);
    assert!(core::mem::offset_of!(ProgramOutput, state_gas_end) == 408);
    assert!(core::mem::offset_of!(ProgramOutput, header_gas_used) == 416);
    assert!(core::mem::offset_of!(ProgramOutput, blob_gas_start) == 424);
    assert!(core::mem::offset_of!(ProgramOutput, blob_gas_end) == 432);
    assert!(core::mem::offset_of!(ProgramOutput, cumulative_gas_start) == 440);
    assert!(core::mem::offset_of!(ProgramOutput, cumulative_gas_end) == 448);
    assert!(core::mem::offset_of!(ProgramOutput, logs_bloom_start) == 456);
    assert!(core::mem::offset_of!(ProgramOutput, logs_bloom_end) == 488);
    // the counts' bytes are their little-endian encoding on every target
    assert!(cfg!(target_endian = "little"));
};

impl Default for ProgramOutput {
    fn default() -> Self {
        Self {
            block_hash: [0; 32],
            initial_accounts: [0; 32],
            initial_slots: [0; 32],
            fresh_accounts_mask: [0; 32],
            bal_mask: [0; 32],
            deposit_requests_hasher_start: [0; SHA256_ACCUMULATOR_LEN],
            deposit_requests_hasher_end: [0; SHA256_ACCUMULATOR_LEN],
            num_unique_bal_records: 0,
            tx_count: 0,
            tx_start_mark: 0,
            tx_end_mark: 0,
            execution_gas_start: 0,
            execution_gas_end: 0,
            state_gas_start: 0,
            state_gas_end: 0,
            header_gas_used: 0,
            blob_gas_start: 0,
            blob_gas_end: 0,
            cumulative_gas_start: 0,
            cumulative_gas_end: 0,
            logs_bloom_start: [0; 32],
            logs_bloom_end: [0; 32],
        }
    }
}

impl ProgramOutput {
    pub const ENCODING_LEN: usize = ENCODING_LEN;

    /// The structure from its encoding (every bit pattern is a value)
    pub fn from_bytes(bytes: &[u8; ENCODING_LEN]) -> Self {
        let mut out = Self::default();
        // SAFETY: the structure is `repr(C)` without padding, of integers and byte
        // arrays, so any `ENCODING_LEN` bytes form a value; `out` is a live, unaliased
        // destination of exactly that size
        unsafe {
            core::ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                &mut out as *mut Self as *mut u8,
                ENCODING_LEN,
            );
        }
        out
    }

    /// The structure's bytes: its encoding, without a copy
    pub fn as_bytes(&self) -> &[u8; ENCODING_LEN] {
        // SAFETY: the structure is `repr(C)` with no padding (asserted above),
        // so its `ENCODING_LEN` bytes are all initialized, and the reference
        // keeps it alive and unaliased for writes
        unsafe { &*(self as *const Self as *const [u8; ENCODING_LEN]) }
    }

    /// keccak of the bytes: the public output of the program
    pub fn hash(&self) -> Bytes32 {
        keccak(self.as_bytes())
    }

    /// The hash as the words a program returns
    pub fn hash_words(&self) -> [u32; 8] {
        let mut words = [0u32; 8];
        for (word, chunk) in words
            .iter_mut()
            .zip(self.hash().as_u8_array_ref().as_chunks::<4>().0)
        {
            *word = u32::from_le_bytes(*chunk);
        }
        words
    }

    /// The output of the pre-execution check: the tables it produced, the
    /// marker mask, the number of records of the access list and the proven
    /// transaction count as the range `0..count`
    pub fn of_pre_execution<A: Allocator>(state: &StateConsistencyOutput<A>) -> Self {
        let [initial_accounts, fresh_accounts_mask, initial_slots] =
            state.initial_values_hashes().map(|hash| hash.as_u8_array());
        Self {
            block_hash: state.header_hash.as_u8_array(),
            initial_accounts,
            initial_slots,
            fresh_accounts_mask,
            bal_mask: [0; 32],
            deposit_requests_hasher_start: [0; SHA256_ACCUMULATOR_LEN],
            deposit_requests_hasher_end: [0; SHA256_ACCUMULATOR_LEN],
            num_unique_bal_records: (state.counts.accounts + state.counts.slots) as u32,
            tx_count: state.transaction_count,
            tx_start_mark: 0,
            tx_end_mark: state.transaction_count,
            execution_gas_start: 0,
            execution_gas_end: 0,
            state_gas_start: 0,
            state_gas_end: 0,
            header_gas_used: state.header.base.gas_used,
            blob_gas_start: 0,
            blob_gas_end: state.header.base.blob_gas_used,
            cumulative_gas_start: 0,
            cumulative_gas_end: 0,
            logs_bloom_start: bloom_hash(&LogsBloom::default()),
            logs_bloom_end: bloom_hash(&state.header.base.logs_bloom),
        }
    }

    /// The output of a transaction range: the tables it witnessed, the
    /// records it accessed and the transactions it covered
    pub fn of_range<A: Allocator>(output: &GlamsterdamRangeOutput<A>) -> Self {
        let [initial_accounts, fresh_accounts_mask, initial_slots] =
            output.initial_values.hashes.map(|hash| hash.as_u8_array());
        Self {
            block_hash: output.header_hash.as_u8_array(),
            initial_accounts,
            initial_slots,
            fresh_accounts_mask,
            bal_mask: mask_hash(&output.accessed).as_u8_array(),
            deposit_requests_hasher_start: *output.requests.start.as_bytes(),
            deposit_requests_hasher_end: *output.requests.end.as_bytes(),
            num_unique_bal_records: output.accessed.len(),
            tx_count: output.transaction_count,
            tx_start_mark: output.transactions.start,
            tx_end_mark: output.transactions.end,
            execution_gas_start: output.start.execution_gas_used,
            execution_gas_end: output.block_execution_gas_used,
            state_gas_start: output.start.state_gas_used,
            state_gas_end: output.block_state_gas_used,
            header_gas_used: output.header_gas_used,
            blob_gas_start: output.start.blob_gas_used,
            blob_gas_end: output.block_blob_gas_used,
            cumulative_gas_start: output.start.cumulative_gas_used,
            cumulative_gas_end: output.cumulative_gas_used,
            logs_bloom_start: bloom_hash(&output.start.logs_bloom),
            logs_bloom_end: bloom_hash(&output.bloom),
        }
    }

    /// Whether `other` describes the same block, tables and transaction count
    fn same_block(&self, other: &Self) -> bool {
        self.block_hash == other.block_hash
            && self.initial_accounts == other.initial_accounts
            && self.initial_slots == other.initial_slots
            && self.fresh_accounts_mask == other.fresh_accounts_mask
            && self.num_unique_bal_records == other.num_unique_bal_records
            && self.tx_count == other.tx_count
            && self.header_gas_used == other.header_gas_used
    }
}

/// keccak of a mask's bytes
pub fn mask_hash<A: Allocator>(mask: &IndexMask<A>) -> Bytes32 {
    keccak(mask.bytes())
}

/// keccak of a bloom's bytes, as the outputs carry it
pub fn bloom_hash(bloom: &LogsBloom) -> [u8; 32] {
    keccak(bloom.as_bytes()).as_u8_array()
}

fn keccak(bytes: &[u8]) -> Bytes32 {
    let mut hasher = Keccak256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

/// The aggregation program over the linkage: given the public outputs of the
/// pre-execution check and of the ranges (as proven), their preimages and the
/// masks' preimages, checks that the programs describe one block, that the
/// ranges tile its transactions in order (so the pre-transaction phase ran
/// once, in the first, and the post-transaction phase, which checks the
/// header's requests hash, once, in the last), that the deposit request
/// hashing, the gas sums, the receipts' cumulative gas and the logs bloom
/// chain through the ranges from their starts to the header's totals, and
/// that together they accessed every record of the access list. The ranges
/// are added in block order.
pub struct LinkageAggregation<A: Allocator> {
    pre_execution: ProgramOutput,
    union: IndexMask<A>,
    /// The transaction the next range must start at
    next_start: u32,
    /// The deposit request hashing state the next range must start from
    next_hasher: [u8; SHA256_ACCUMULATOR_LEN],
    /// The gas sums the next range must start from: execution, state, blob
    next_gas: [u64; 3],
    /// The receipts' cumulative gas the next range must start from
    next_cumulative_gas: u64,
    /// The hash of the block's logs bloom the next range must start from
    next_bloom: [u8; 32],
    /// Whether the range ending the block is in
    reached_end: bool,
}

impl<A: Allocator> LinkageAggregation<A> {
    /// Starts from the pre-execution check: `public_output` is its proven
    /// output, `pre_execution` the preimage, whose mask must be clear and
    /// whose transaction range is the whole block
    pub fn new(
        public_output: Bytes32,
        pre_execution: &ProgramOutput,
        allocator: A,
    ) -> Result<Self> {
        let fail = GlamsterdamError::Aggregation;
        if pre_execution.hash() != public_output {
            return Err(fail("the pre-execution output is not the proven one"));
        }
        if pre_execution.bal_mask != [0; 32] {
            return Err(fail("the pre-execution check publishes no mask"));
        }
        if pre_execution.tx_start_mark != 0 || pre_execution.tx_end_mark != pre_execution.tx_count {
            return Err(fail("the pre-execution check covers the whole block"));
        }
        if pre_execution.deposit_requests_hasher_start != [0; SHA256_ACCUMULATOR_LEN]
            || pre_execution.deposit_requests_hasher_end != [0; SHA256_ACCUMULATOR_LEN]
        {
            return Err(fail("the pre-execution check hashes no requests"));
        }
        if pre_execution.execution_gas_start != 0
            || pre_execution.execution_gas_end != 0
            || pre_execution.state_gas_start != 0
            || pre_execution.state_gas_end != 0
            || pre_execution.blob_gas_start != 0
            || pre_execution.cumulative_gas_start != 0
            || pre_execution.cumulative_gas_end != 0
        {
            return Err(fail("the pre-execution check runs no transactions"));
        }
        // the block's bloom starts empty
        let empty_bloom = bloom_hash(&LogsBloom::default());
        if pre_execution.logs_bloom_start != empty_bloom {
            return Err(fail("the pre-execution check starts from the empty bloom"));
        }
        let union = IndexMask::zeroed(pre_execution.num_unique_bal_records as usize, allocator)?;
        Ok(Self {
            pre_execution: *pre_execution,
            union,
            next_start: 0,
            next_hasher: *Sha256Accumulator::deposit_requests_head().as_bytes(),
            next_gas: [0; 3],
            next_cumulative_gas: 0,
            next_bloom: empty_bloom,
            reached_end: false,
        })
    }

    /// Adds the next range: `public_output` is its proven output, `range` the
    /// preimage and `bal_mask` the preimage of its mask
    pub fn add(
        &mut self,
        public_output: Bytes32,
        range: &ProgramOutput,
        bal_mask: &IndexMask<A>,
    ) -> Result<()> {
        let fail = GlamsterdamError::Aggregation;
        if range.hash() != public_output {
            return Err(fail("the range output is not the proven one"));
        }
        if !self.pre_execution.same_block(range) {
            return Err(fail(
                "the range describes another block, other tables or another transaction count",
            ));
        }
        if mask_hash(bal_mask).as_u8_array() != range.bal_mask {
            return Err(fail("the mask is not the range's"));
        }
        // the tiling: the ranges follow each other, the last one ends the block
        let count = self.pre_execution.tx_count;
        if self.reached_end {
            return Err(fail("a range after the last one"));
        }
        if range.tx_start_mark != self.next_start {
            return Err(fail("the ranges do not tile the block"));
        }
        if range.tx_end_mark > count
            || range.tx_end_mark < range.tx_start_mark
            || (range.tx_end_mark == range.tx_start_mark && count != 0)
        {
            return Err(fail("the range is outside the block or empty"));
        }
        if range.deposit_requests_hasher_start != self.next_hasher {
            return Err(fail(
                "the deposit request hashing does not continue the previous range's",
            ));
        }
        if [
            range.execution_gas_start,
            range.state_gas_start,
            range.blob_gas_start,
        ] != self.next_gas
        {
            return Err(fail(
                "the gas at the start of the range is not the previous range's",
            ));
        }
        if range.cumulative_gas_start != self.next_cumulative_gas {
            return Err(fail(
                "the cumulative gas at the start of the range is not the previous range's",
            ));
        }
        if range.logs_bloom_start != self.next_bloom {
            return Err(fail(
                "the logs bloom at the start of the range is not the previous range's",
            ));
        }
        self.union.or_assign(bal_mask)?;
        self.reached_end = range.tx_end_mark == count;
        self.next_start = range.tx_end_mark;
        self.next_hasher = range.deposit_requests_hasher_end;
        self.next_gas = [
            range.execution_gas_end,
            range.state_gas_end,
            range.blob_gas_end,
        ];
        self.next_cumulative_gas = range.cumulative_gas_end;
        self.next_bloom = range.logs_bloom_end;
        Ok(())
    }

    /// The ranges reached the end of the block, their gas sums and the final
    /// bloom are the header's, and every record of the access list was
    /// accessed by some range. The final cumulative gas is bound by the
    /// receipts trie, which the last range checked its receipts against.
    pub fn finish(self) -> Result<()> {
        let fail = GlamsterdamError::Aggregation;
        if !self.reached_end {
            return Err(fail("the ranges do not reach the end of the block"));
        }
        let [execution, state, blob] = self.next_gas;
        if execution.max(state) != self.pre_execution.header_gas_used {
            return Err(fail("gas used"));
        }
        if blob != self.pre_execution.blob_gas_end {
            return Err(fail("blob gas used"));
        }
        if self.next_bloom != self.pre_execution.logs_bloom_end {
            return Err(fail("logs bloom"));
        }
        if !self.union.is_full() {
            return Err(fail("a record of the access list is never accessed"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::alloc::Global;

    const TX_COUNT: u32 = 10;
    const GAS_USED: u64 = 1_000_000;
    /// Five blobs: a fifth per transaction for the first five
    const BLOB_GAS: u64 = 5 * 131_072;

    /// The bloom of a range: one bit per transaction of the range
    fn bloom_of(transactions: &core::ops::Range<u32>) -> LogsBloom {
        let mut bytes = [0u8; 256];
        for i in transactions.clone() {
            bytes[i as usize] = 1;
        }
        LogsBloom::from_bytes(&bytes)
    }

    fn pre_execution(num: u32) -> ProgramOutput {
        ProgramOutput {
            block_hash: [1; 32],
            initial_accounts: [2; 32],
            initial_slots: [3; 32],
            fresh_accounts_mask: [4; 32],
            bal_mask: [0; 32],
            deposit_requests_hasher_start: [0; SHA256_ACCUMULATOR_LEN],
            deposit_requests_hasher_end: [0; SHA256_ACCUMULATOR_LEN],
            num_unique_bal_records: num,
            tx_count: TX_COUNT,
            tx_start_mark: 0,
            tx_end_mark: TX_COUNT,
            execution_gas_start: 0,
            execution_gas_end: 0,
            state_gas_start: 0,
            state_gas_end: 0,
            header_gas_used: GAS_USED,
            blob_gas_start: 0,
            blob_gas_end: BLOB_GAS,
            cumulative_gas_start: 0,
            cumulative_gas_end: 0,
            logs_bloom_start: bloom_hash(&LogsBloom::default()),
            logs_bloom_end: bloom_hash(&bloom_of(&(0..TX_COUNT))),
        }
    }

    /// The deposit request hashing states of a range: from the head, 192 bytes
    /// per transaction of the range absorbed
    fn hashers(transactions: &core::ops::Range<u32>) -> [Sha256Accumulator; 2] {
        let mut start = Sha256Accumulator::deposit_requests_head();
        for _ in 0..transactions.start {
            start.absorb(&[7; 192]);
        }
        let mut end = start;
        for _ in transactions.clone() {
            end.absorb(&[7; 192]);
        }
        [start, end]
    }

    /// A range over `transactions`: every transaction uses a tenth of the block's
    /// gas (execution, a thousand less in its receipt), a fifth of its blob gas
    /// for the first five, no state gas, and marks its own bit in the bloom
    fn range_output(
        num: u32,
        mask: &IndexMask<Global>,
        transactions: core::ops::Range<u32>,
    ) -> ProgramOutput {
        let [start, end] = hashers(&transactions);
        let per_tx = GAS_USED / TX_COUNT as u64;
        let blob = |tx: u32| (tx.min(5) as u64) * (BLOB_GAS / 5);
        ProgramOutput {
            bal_mask: mask_hash(mask).as_u8_array(),
            deposit_requests_hasher_start: *start.as_bytes(),
            deposit_requests_hasher_end: *end.as_bytes(),
            tx_start_mark: transactions.start,
            tx_end_mark: transactions.end,
            execution_gas_start: per_tx * transactions.start as u64,
            execution_gas_end: per_tx * transactions.end as u64,
            state_gas_start: 0,
            state_gas_end: 0,
            blob_gas_start: blob(transactions.start),
            blob_gas_end: blob(transactions.end),
            cumulative_gas_start: (per_tx - 1000) * transactions.start as u64,
            cumulative_gas_end: (per_tx - 1000) * transactions.end as u64,
            logs_bloom_start: bloom_hash(&bloom_of(&(0..transactions.start))),
            logs_bloom_end: bloom_hash(&bloom_of(&(0..transactions.end))),
            ..pre_execution(num)
        }
    }

    #[test]
    fn the_bytes_are_the_encoding_and_the_hash_covers_them() {
        let out = pre_execution(40);
        let bytes = out.as_bytes();
        assert_eq!(bytes.len(), 520);
        assert_eq!(&bytes[..32], &[1; 32]);
        assert_eq!(&bytes[128..160], &[0; 32]);
        assert_eq!(&bytes[160..368], &[0; 2 * SHA256_ACCUMULATOR_LEN]);
        assert_eq!(&bytes[368..372], &40u32.to_le_bytes());
        assert_eq!(&bytes[372..376], &TX_COUNT.to_le_bytes());
        assert_eq!(&bytes[376..380], &0u32.to_le_bytes());
        assert_eq!(&bytes[380..384], &TX_COUNT.to_le_bytes());
        assert_eq!(&bytes[416..424], &GAS_USED.to_le_bytes());
        assert_eq!(&bytes[432..440], &BLOB_GAS.to_le_bytes());
        assert_eq!(&bytes[440..456], &[0; 16]);
        assert_eq!(&bytes[456..488], &out.logs_bloom_start);
        assert_eq!(&bytes[488..], &out.logs_bloom_end);
        assert_eq!(ProgramOutput::from_bytes(bytes), out);
        let mut other = out;
        other.num_unique_bal_records = 41;
        assert_ne!(out.hash(), other.hash());
        let mut other = out;
        other.tx_end_mark = 9;
        assert_ne!(out.hash(), other.hash());
        let words = out.hash_words();
        assert_eq!(words[0].to_le_bytes(), out.hash().as_u8_array()[..4]);
        let range = range_output(40, &IndexMask::zeroed(40, Global).unwrap(), 2..5);
        let [start, end] = hashers(&(2..5));
        assert_eq!(&range.as_bytes()[160..264], start.as_bytes());
        assert_eq!(&range.as_bytes()[264..368], end.as_bytes());
    }

    #[test]
    fn aggregation_requires_full_union_tiling_gas_and_blooms() {
        let pre = pre_execution(40);
        let first = IndexMask::from_flags((0..40).map(|i| i < 25), Global).unwrap();
        let second = IndexMask::from_flags((0..40).map(|i| i >= 20), Global).unwrap();
        let first_out = range_output(40, &first, 0..4);
        let second_out = range_output(40, &second, 4..TX_COUNT);

        let aggregate = |outputs: &[(&ProgramOutput, &IndexMask<Global>)]| -> Result<()> {
            let mut aggregation = LinkageAggregation::new(pre.hash(), &pre, Global)?;
            for (out, mask) in outputs {
                aggregation.add(out.hash(), out, mask)?;
            }
            aggregation.finish()
        };
        aggregate(&[(&first_out, &first), (&second_out, &second)]).unwrap();

        // a missing range (the block is not reached), a missing first range
        assert!(aggregate(&[(&first_out, &first)]).is_err());
        assert!(aggregate(&[(&second_out, &second)]).is_err());
        // a gap, an overlap, a wrong order, a range after the last
        let gapped = range_output(40, &second, 5..TX_COUNT);
        assert!(aggregate(&[(&first_out, &first), (&gapped, &second)]).is_err());
        let overlapping = range_output(40, &second, 3..TX_COUNT);
        assert!(aggregate(&[(&first_out, &first), (&overlapping, &second)]).is_err());
        assert!(aggregate(&[(&second_out, &second), (&first_out, &first)]).is_err());
        let whole = range_output(40, &second, 0..TX_COUNT);
        assert!(aggregate(&[(&whole, &second), (&whole, &second)]).is_err());
        // a partial union
        let few = IndexMask::from_flags((0..40).map(|i| i < 10), Global).unwrap();
        let few_out = range_output(40, &few, 0..4);
        assert!(aggregate(&[(&few_out, &few), (&second_out, &second)]).is_err());
        // a broken hashing chain: a first range not starting from the head, a
        // second one not continuing the first, the pre-execution check hashing
        let mut unchained = first_out;
        unchained.deposit_requests_hasher_start = *Sha256Accumulator::new().as_bytes();
        assert!(aggregate(&[(&unchained, &first), (&second_out, &second)]).is_err());
        let mut skipped = second_out;
        skipped.deposit_requests_hasher_start = first_out.deposit_requests_hasher_start;
        assert!(aggregate(&[(&first_out, &first), (&skipped, &second)]).is_err());
        let mut hashing = pre;
        hashing.deposit_requests_hasher_end = first_out.deposit_requests_hasher_end;
        assert!(LinkageAggregation::new(hashing.hash(), &hashing, Global).is_err());
        // a broken gas chain, a wrong total, a wrong blob total
        let mut jumped = second_out;
        jumped.execution_gas_start += 1;
        assert!(aggregate(&[(&first_out, &first), (&jumped, &second)]).is_err());
        let mut short = second_out;
        short.execution_gas_end -= 1;
        assert!(aggregate(&[(&first_out, &first), (&short, &second)]).is_err());
        let mut blobless = second_out;
        blobless.blob_gas_end -= 1;
        assert!(aggregate(&[(&first_out, &first), (&blobless, &second)]).is_err());
        // a broken cumulative gas chain: a second range not starting from the
        // first's end, a pre-execution check that counts
        let mut recounted = second_out;
        recounted.cumulative_gas_start = first_out.cumulative_gas_end + 1;
        assert!(aggregate(&[(&first_out, &first), (&recounted, &second)]).is_err());
        let mut counting = pre;
        counting.cumulative_gas_end = second_out.cumulative_gas_end;
        assert!(LinkageAggregation::new(counting.hash(), &counting, Global).is_err());
        // a broken bloom chain: a first range not starting from the empty bloom,
        // a second one not starting from the first's end, a last one not ending
        // with the header's (a bit the header's bloom does not have), a
        // pre-execution check not starting from the empty bloom
        let mut marked = first_out;
        marked.logs_bloom_start = bloom_hash(&bloom_of(&(0..1)));
        assert!(aggregate(&[(&marked, &first), (&second_out, &second)]).is_err());
        let mut restarted = second_out;
        restarted.logs_bloom_start = first_out.logs_bloom_start;
        assert!(aggregate(&[(&first_out, &first), (&restarted, &second)]).is_err());
        let mut noisy = second_out;
        noisy.logs_bloom_end = bloom_hash(&bloom_of(&(0..TX_COUNT + 1)));
        assert!(aggregate(&[(&first_out, &first), (&noisy, &second)]).is_err());
        let mut preset = pre;
        preset.logs_bloom_start = preset.logs_bloom_end;
        assert!(LinkageAggregation::new(preset.hash(), &preset, Global).is_err());
        // the state gas counts: a block whose state gas exceeds its execution gas
        let mut stateful = pre;
        stateful.header_gas_used = GAS_USED + 1;
        let mut first_heavy = first_out;
        first_heavy.header_gas_used = GAS_USED + 1;
        let mut heavy = second_out;
        heavy.header_gas_used = GAS_USED + 1;
        heavy.state_gas_end = GAS_USED + 1;
        let mut aggregation = LinkageAggregation::new(stateful.hash(), &stateful, Global).unwrap();
        aggregation
            .add(first_heavy.hash(), &first_heavy, &first)
            .unwrap();
        aggregation.add(heavy.hash(), &heavy, &second).unwrap();
        aggregation.finish().unwrap();
        // a wrong public output, a mask that is not the range's, another block
        // or count, a pre-execution output that carries a mask, a partial range
        // or gas sums, a wrong record count
        let mut wrong = LinkageAggregation::new(pre.hash(), &pre, Global).unwrap();
        assert!(wrong.add(second_out.hash(), &first_out, &first).is_err());
        assert!(wrong.add(first_out.hash(), &first_out, &second).is_err());
        let mut foreign = first_out;
        foreign.block_hash = [0; 32];
        assert!(wrong.add(foreign.hash(), &foreign, &first).is_err());
        let mut miscounted = first_out;
        miscounted.tx_count = TX_COUNT + 1;
        assert!(wrong.add(miscounted.hash(), &miscounted, &first).is_err());
        assert!(LinkageAggregation::new(first_out.hash(), &first_out, Global).is_err());
        let mut partial = pre;
        partial.tx_end_mark = 5;
        assert!(LinkageAggregation::new(partial.hash(), &partial, Global).is_err());
        let mut summed = pre;
        summed.execution_gas_end = 1;
        assert!(LinkageAggregation::new(summed.hash(), &summed, Global).is_err());
        let short = pre_execution(39);
        let mut counted = LinkageAggregation::new(short.hash(), &short, Global).unwrap();
        assert!(counted.add(first_out.hash(), &first_out, &first).is_err());
    }

    /// A block without transactions is one range `0..0`, which runs both phases
    #[test]
    fn an_empty_block_is_one_range() {
        let mut pre = pre_execution(4);
        pre.tx_count = 0;
        pre.tx_end_mark = 0;
        pre.header_gas_used = 0;
        pre.blob_gas_end = 0;
        pre.logs_bloom_end = pre.logs_bloom_start;
        let mask = IndexMask::from_flags((0..4).map(|_| true), Global).unwrap();
        let head = *Sha256Accumulator::deposit_requests_head().as_bytes();
        let out = ProgramOutput {
            bal_mask: mask_hash(&mask).as_u8_array(),
            deposit_requests_hasher_start: head,
            deposit_requests_hasher_end: head,
            tx_end_mark: 0,
            ..pre
        };
        let mut aggregation = LinkageAggregation::new(pre.hash(), &pre, Global).unwrap();
        aggregation.add(out.hash(), &out, &mask).unwrap();
        aggregation.finish().unwrap();
    }
}
