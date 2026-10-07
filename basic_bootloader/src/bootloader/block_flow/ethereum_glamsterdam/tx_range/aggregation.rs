//! Aggregation of the outputs of the ranges of a block: the checks over the
//! ranges' outputs that make them, together with the pre-execution check, a
//! proof of the block. They run on the host today over
//! [`GlamsterdamRangeOutput`]s; a recursive aggregation would perform the same
//! over the ranges' public outputs.
//!
//! The ranges must tile the block's access indices `0..=n+1` in order. Every
//! range must have run on the pre-execution check's header and initial value
//! tables (by hash). The gas sums a range witnesses as its start, unverified,
//! must be the sums after the previous range. The union of the accounts and
//! slots the ranges accessed must be the whole access list, so that a list
//! with entries nothing accessed is rejected. The checks a whole-block range
//! performs itself against the header (gas used, logs bloom, blob gas) are
//! performed here over the last range's sums and the merged blooms.
//!
//! Not checked here: the requests hash of a block split into ranges, as the
//! deposit requests are the whole block's logs (pending). The subset of these
//! checks that the programs' public outputs carry is `linkage`.

use super::super::index_mask::IndexMask;
use super::super::state_consistency::StateConsistencyOutput;
use super::super::GlamsterdamError;
use super::block_flow::GlamsterdamRangeOutput;
use super::witness::RangeStartGas;
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
            gas_used: output.header.base.gas_used,
            logs_bloom: output.header.base.logs_bloom,
            blob_gas_used: output.header.base.blob_gas_used,
        }
    }
}

/// The ranges of a block, aggregated in order
pub struct RangeAggregation<A: Allocator> {
    summary: PreExecutionSummary,
    /// The index the next range must start at
    next_index: u32,
    transaction_count: Option<u32>,
    /// The block's gas after the last range
    gas: RangeStartGas,
    bloom: LogsBloom,
    accessed: Option<IndexMask<A>>,
}

impl<A: Allocator + Clone> RangeAggregation<A> {
    pub fn new(summary: PreExecutionSummary) -> Self {
        Self {
            summary,
            next_index: 0,
            transaction_count: None,
            gas: RangeStartGas::default(),
            bloom: LogsBloom::default(),
            accessed: None,
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
        match self.transaction_count {
            None => self.transaction_count = Some(output.transaction_count),
            Some(count) if count != output.transaction_count => {
                return Err(fail("the ranges disagree on the transaction count"))
            }
            Some(_) => {}
        }
        if output.range.first != self.next_index {
            return Err(fail("the ranges do not tile the block"));
        }
        if output.start_gas != self.gas {
            return Err(fail(
                "the gas at the start of the range is not the previous range's",
            ));
        }
        or_assign(&mut self.accessed, &output.accessed)?;
        self.gas = output.end_gas();
        self.bloom.merge(&output.bloom);
        self.next_index = output.range.last + 1;
        Ok(())
    }

    /// Checks the block once every range is in
    pub fn finish(self) -> Result<()> {
        let fail = GlamsterdamError::Aggregation;
        let count = self.transaction_count.ok_or(fail("no range"))?;
        if self.next_index != count + 2 {
            return Err(fail("the ranges do not reach the end of the block"));
        }
        if self.gas.block_gas_used() != self.summary.gas_used {
            return Err(fail("gas used"));
        }
        if self.gas.blob_gas_used != self.summary.blob_gas_used {
            return Err(fail("blob gas used"));
        }
        if self.bloom != self.summary.logs_bloom {
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
