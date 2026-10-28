//! The aggregation of the programs of a Glamsterdam block as a program: given
//! the public output of the pre-execution check and the public outputs of the
//! transaction ranges (what the proofs of those programs establish), it
//! witnesses the preimages of the outputs (`linkage::ProgramOutput`) and the
//! masks of the access list records the ranges accessed, and runs the linkage
//! checks ([`LinkageAggregation`]): the programs describe one block and one
//! set of initial value tables with one transaction count, the ranges tile
//! the block's transactions in order (the pre-transaction phase ran once in
//! the first, the post-transaction phase, which checked the header's requests
//! hash, once in the last), the deposit request hashing, the gas sums, the
//! receipts' cumulative gas and the logs bloom chain through the ranges, the
//! final gas sums, the blob gas and the bloom are the header's, and the union
//! of the masks is the whole access list. Together with the proofs of the
//! programs, that is the proof of the block: every transaction was processed
//! and checked, and nothing of the block was left out.
//!
//! The program returns the hash of [`AggregationOutput`]: the block hash and
//! the public outputs it aggregated.

use super::index_mask::IndexMask;
use super::linkage::{LinkageAggregation, ProgramOutput, ENCODING_LEN};
use super::oracle_queries::*;
use super::GlamsterdamError;
use alloc::vec::Vec;
use core::alloc::Allocator;
use crypto::sha3::Keccak256;
use crypto::MiniDigest;
use zk_ee::oracle::memory_io::{
    get_bytes_with_length_query, ContinuousDeserializable, ContinuousSerializable, OracleQuery,
};
use zk_ee::oracle::IOOracle;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::utils::Bytes32;

type Result<T> = core::result::Result<T, GlamsterdamError>;

/// The public output of a program, as the words it returns
/// (`ProgramOutput::hash_words`). `#[repr(C)]`: the oracle writes it memcpy-like.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PublicOutput(pub [u32; 8]);

const _: () = {
    assert!(core::mem::size_of::<PublicOutput>() == 32);
    assert!(core::mem::align_of::<PublicOutput>() == 4);
};

// SAFETY: eight `u32`, no padding
unsafe impl ContinuousSerializable for PublicOutput {}

// SAFETY: any bytes form a value
unsafe impl ContinuousDeserializable for PublicOutput {
    #[inline(always)]
    unsafe fn validate<'a>(this: *mut Self) -> core::result::Result<&'a mut Self, InternalError> {
        // SAFETY: guaranteed by the caller
        Ok(unsafe { &mut *this })
    }
}

impl PublicOutput {
    /// The words of a hash, as a program returns them
    pub fn of_hash(hash: &Bytes32) -> Self {
        let mut words = [0u32; 8];
        for (word, chunk) in words
            .iter_mut()
            .zip(hash.as_u8_array_ref().as_chunks::<4>().0)
        {
            *word = u32::from_le_bytes(*chunk);
        }
        Self(words)
    }

    /// The hash the words are
    pub fn hash(&self) -> Bytes32 {
        let mut bytes = [0u8; 32];
        for (chunk, word) in bytes.as_chunks_mut::<4>().0.iter_mut().zip(self.0) {
            *chunk = word.to_le_bytes();
        }
        Bytes32::from_array(bytes)
    }
}

pub struct PreExecutionOutputQuery;

impl OracleQuery for PreExecutionOutputQuery {
    const QUERY_ID: u32 = GLAMSTERDAM_AGGREGATION_PRE_EXECUTION_OUTPUT_QUERY_ID;
    type Input = ();
    type Output = PublicOutput;
}

pub struct RangeCountQuery;

impl OracleQuery for RangeCountQuery {
    const QUERY_ID: u32 = GLAMSTERDAM_AGGREGATION_RANGE_COUNT_QUERY_ID;
    type Input = ();
    type Output = u32;
}

pub struct RangeOutputQuery;

impl OracleQuery for RangeOutputQuery {
    const QUERY_ID: u32 = GLAMSTERDAM_AGGREGATION_RANGE_OUTPUT_QUERY_ID;
    type Input = u32;
    type Output = PublicOutput;
}

/// What the aggregation established: the block and the public outputs it
/// aggregated, in order (the pre-execution check, then the ranges)
pub struct AggregationOutput<A: Allocator> {
    pub block_hash: Bytes32,
    pub pre_execution: Bytes32,
    pub ranges: Vec<Bytes32, A>,
}

impl<A: Allocator> AggregationOutput<A> {
    /// keccak of the block hash, the pre-execution output and the range outputs
    /// in order: the public output of the aggregation program
    pub fn hash(&self) -> Bytes32 {
        let mut hasher = Keccak256::new();
        hasher.update(self.block_hash.as_u8_array_ref());
        hasher.update(self.pre_execution.as_u8_array_ref());
        for range in self.ranges.iter() {
            hasher.update(range.as_u8_array_ref());
        }
        hasher.finalize().into()
    }

    /// The hash as the words a program returns
    pub fn hash_words(&self) -> [u32; 8] {
        PublicOutput::of_hash(&self.hash()).0
    }
}

/// Witnesses a length-prefixed buffer by index
fn witness_indexed<O: IOOracle, A: Allocator>(
    oracle: &mut O,
    len_query: u32,
    data_query: u32,
    index: u32,
    allocator: A,
) -> Result<zk_ee::utils::UsizeAlignedByteBox<A>> {
    get_bytes_with_length_query::<_, u32, _>(oracle, len_query, data_query, index, allocator)
        .map_err(|_| GlamsterdamError::Oracle)?
        .ok_or(GlamsterdamError::Aggregation("an empty preimage"))
}

/// The preimage of the public output of program `index` (0: the pre-execution
/// check, `i + 1`: range `i`), which must hash to `public_output`
fn witness_program_output<O: IOOracle, A: Allocator + Clone>(
    oracle: &mut O,
    index: u32,
    public_output: Bytes32,
    allocator: A,
) -> Result<ProgramOutput> {
    let bytes = witness_indexed(
        oracle,
        GLAMSTERDAM_AGGREGATION_OUTPUT_PREIMAGE_LEN_QUERY_ID,
        GLAMSTERDAM_AGGREGATION_OUTPUT_PREIMAGE_DATA_QUERY_ID,
        index,
        allocator,
    )?;
    let bytes: &[u8; ENCODING_LEN] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| GlamsterdamError::Aggregation("a program output of the wrong length"))?;
    let output = ProgramOutput::from_bytes(bytes);
    if output.hash() != public_output {
        return Err(GlamsterdamError::Aggregation(
            "a program output is not the proven one",
        ));
    }
    Ok(output)
}

/// Aggregates the programs of a block from the oracle: the public outputs
/// (the inputs of the program) and their preimages. See the module
/// documentation.
pub fn verify_block_aggregation<O: IOOracle, A: Allocator + Clone>(
    oracle: &mut O,
    allocator: A,
) -> Result<AggregationOutput<A>> {
    let pre_execution = PreExecutionOutputQuery::get(oracle, ())
        .map_err(|_| GlamsterdamError::Oracle)?
        .hash();
    let range_count = RangeCountQuery::get(oracle, ()).map_err(|_| GlamsterdamError::Oracle)?;
    let mut ranges = Vec::with_capacity_in(range_count as usize, allocator.clone());
    for index in 0..range_count {
        let output = RangeOutputQuery::get(oracle, index)
            .map_err(|_| GlamsterdamError::Oracle)?
            .hash();
        ranges.push(output);
    }

    let pre_execution_output = witness_program_output(oracle, 0, pre_execution, allocator.clone())?;
    let num_records = pre_execution_output.num_unique_bal_records as usize;
    let mut aggregation =
        LinkageAggregation::new(pre_execution, &pre_execution_output, allocator.clone())?;
    for (index, public_output) in ranges.iter().enumerate() {
        let index = index as u32;
        let output = witness_program_output(oracle, index + 1, *public_output, allocator.clone())?;
        let mask = witness_indexed(
            oracle,
            GLAMSTERDAM_AGGREGATION_RANGE_MASK_LEN_QUERY_ID,
            GLAMSTERDAM_AGGREGATION_RANGE_MASK_DATA_QUERY_ID,
            index,
            allocator.clone(),
        )?;
        let mask = IndexMask::from_bytes(num_records, mask.as_slice(), allocator.clone())?;
        aggregation.add(*public_output, &output, &mask)?;
    }
    aggregation.finish()?;

    Ok(AggregationOutput {
        block_hash: Bytes32::from_array(pre_execution_output.block_hash),
        pre_execution,
        ranges,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_output_words_round_trip() {
        let hash = Bytes32::from_array(core::array::from_fn(|i| i as u8));
        let words = PublicOutput::of_hash(&hash);
        assert_eq!(words.hash(), hash);
        assert_eq!(words.0[0], u32::from_le_bytes([0, 1, 2, 3]));
    }
}
