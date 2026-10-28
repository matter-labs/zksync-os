//! Aggregation of the outputs of the ranges of a block: the checks over the
//! ranges' outputs that make them, together with the pre-execution check, a
//! proof of the block. They run on the host today over
//! [`GlamsterdamRangeOutput`]s; a recursive aggregation would perform the same
//! over the ranges' public outputs (see `linkage`, which carries the subset
//! of these checks the programs' public outputs allow).
//!
//! The ranges must tile the block's transactions `0..n` in order, each run
//! with the transaction count the pre-execution check proved: the first then
//! ran the pre-transaction phase and the last the post-transaction phase,
//! each once. Every range must have run on the pre-execution check's header
//! and initial value tables (by hash). The block's accounting a range
//! witnesses as its start, unverified (the receipts' cumulative gas, the gas
//! sums, the logs bloom; `witness::BlockProgress`), must be the accounting
//! after the previous range. The union of the accounts and slots the ranges
//! accessed must be the whole access list, so that a list with entries
//! nothing accessed is rejected. The checks the range ending the block
//! performs itself against the header (gas used, logs bloom, blob gas) are
//! repeated here over the last range's end. The requests hash is checked by
//! the last range itself,
//! over the deposit request hashing carried across the ranges as a sha256
//! state: the state a range witnessed as its start must be the previous
//! range's end, the first range's the head.

use super::super::index_mask::IndexMask;
use super::super::state_consistency::StateConsistencyOutput;
use super::super::GlamsterdamError;
use super::block_flow::GlamsterdamRangeOutput;
use super::requests::Sha256Accumulator;
use super::witness::{BlockAccessIndexRange, BlockProgress};
use crate::bootloader::transaction_flow::logs_bloom::LogsBloom;
use core::alloc::Allocator;
use zk_ee::utils::Bytes32;

type Result<T> = core::result::Result<T, GlamsterdamError>;

/// What the pre-execution check establishes for the ranges
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreExecutionSummary {
    pub header_hash: Bytes32,
    /// keccak of the initial value tables (see `StateConsistencyOutput::initial_values_hashes`)
    pub initial_values_hashes: [Bytes32; 3],
    /// The number of transactions, proven against the index tries
    pub transaction_count: u32,
    /// The header fields the ranges are checked against
    pub gas_used: u64,
    pub logs_bloom: LogsBloom,
    pub blob_gas_used: u64,
}

impl PreExecutionSummary {
    pub fn of<A: Allocator>(output: &StateConsistencyOutput<A>) -> Self {
        Self {
            header_hash: output.header_hash,
            initial_values_hashes: output.initial_values_hashes(),
            transaction_count: output.transaction_count,
            gas_used: output.header.base.gas_used,
            logs_bloom: output.header.base.logs_bloom,
            blob_gas_used: output.header.base.blob_gas_used,
        }
    }
}

/// The ranges of a block, aggregated in order
pub struct RangeAggregation<A: Allocator> {
    summary: PreExecutionSummary,
    /// The transaction the next range must start at
    next_start: u32,
    /// The block's accounting after the last range
    progress: BlockProgress,
    accessed: Option<IndexMask<A>>,
    /// The deposit request hashing state the next range must start from
    hasher: Sha256Accumulator,
    /// Whether the range ending the block is in
    reached_end: bool,
}

impl<A: Allocator + Clone> RangeAggregation<A> {
    pub fn new(summary: PreExecutionSummary) -> Self {
        Self {
            summary,
            next_start: 0,
            progress: BlockProgress::default(),
            accessed: None,
            hasher: Sha256Accumulator::deposit_requests_head(),
            reached_end: false,
        }
    }

    /// Adds the next range, which must start where the previous one ended
    pub fn add(&mut self, output: &GlamsterdamRangeOutput<A>) -> Result<()> {
        let fail = GlamsterdamError::Aggregation;
        if output.header_hash != self.summary.header_hash {
            return Err(fail("the range ran on another header"));
        }
        if output.initial_values.hashes != self.summary.initial_values_hashes {
            return Err(fail("the range ran on other initial value tables"));
        }
        let count = self.summary.transaction_count;
        if output.transaction_count != count {
            return Err(fail("the range ran with another transaction count"));
        }
        if self.reached_end {
            return Err(fail("a range after the last one"));
        }
        if output.transactions.start != self.next_start {
            return Err(fail("the ranges do not tile the block"));
        }
        // the access indices follow from the transactions and the count
        if BlockAccessIndexRange::of(output.transactions, count)? != output.range {
            return Err(fail(
                "the access index range is not the transaction range's",
            ));
        }
        // the last range checked the requests hash over the deposit request hashing
        // it continued: the state it started from must be the previous range's end
        let last = output.transactions.includes_post_transactions(count);
        if output.requests.start != self.hasher {
            return Err(fail(
                "the deposit request hashing does not continue the previous range's",
            ));
        }
        if output.requests.system.is_some() != last {
            return Err(fail(
                "the post-transaction phase belongs to the last range only",
            ));
        }
        if output.start != self.progress {
            return Err(fail(
                "the block's accounting at the start of the range is not the previous range's",
            ));
        }
        or_assign(&mut self.accessed, &output.accessed)?;
        self.progress = output.end();
        self.hasher = output.requests.end;
        self.reached_end = last;
        self.next_start = output.transactions.end;
        Ok(())
    }

    /// Checks the block once every range is in
    pub fn finish(self) -> Result<()> {
        let fail = GlamsterdamError::Aggregation;
        if !self.reached_end {
            return Err(fail("the ranges do not reach the end of the block"));
        }
        if self.progress.block_gas_used() != self.summary.gas_used {
            return Err(fail("gas used"));
        }
        if self.progress.blob_gas_used != self.summary.blob_gas_used {
            return Err(fail("blob gas used"));
        }
        if self.progress.logs_bloom != self.summary.logs_bloom {
            return Err(fail("logs bloom"));
        }
        if !self.accessed.is_some_and(|mask| mask.is_full()) {
            return Err(fail("a record of the access list is never accessed"));
        }
        Ok(())
    }
}

fn or_assign<A: Allocator + Clone>(
    accumulated: &mut Option<IndexMask<A>>,
    mask: &IndexMask<A>,
) -> Result<()> {
    match accumulated {
        Some(accumulated) => accumulated.or_assign(mask),
        None => {
            *accumulated = Some(mask.clone());
            Ok(())
        }
    }
}
