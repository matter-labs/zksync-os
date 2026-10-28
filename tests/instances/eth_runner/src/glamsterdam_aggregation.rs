//! Host side of the Glamsterdam aggregation program: the responder serving
//! the public outputs of the programs of a block and their preimages, and the
//! native run of the aggregation, recorded for the simulator.

use basic_bootloader::bootloader::block_flow::ethereum_glamsterdam::block_aggregation::{
    verify_block_aggregation, AggregationOutput, PublicOutput,
};
use basic_bootloader::bootloader::block_flow::ethereum_glamsterdam::linkage::ProgramOutput;
use basic_bootloader::bootloader::block_flow::ethereum_glamsterdam::oracle_queries::*;
use basic_bootloader::bootloader::block_flow::ethereum_glamsterdam::state_consistency::StateConsistencyOutput;
use basic_bootloader::bootloader::block_flow::ethereum_glamsterdam::tx_range::block_flow::GlamsterdamRangeOutput;
use forward_system::run::query_processors::UARTPrintResponder;
use oracle_provider::{
    respond_to_every_target, OracleQueryProcessor, ReadWitnessSource, RunMode,
    ZkEENonDeterminismSource,
};
use std::alloc::Global;
use zk_ee::oracle::memory_io::host::{
    write_dynamic_bytes, QuerierMemory, ReadQueryInput, WriteQueryOutput,
};

/// Serves the inputs of the aggregation program: the public outputs of the
/// programs of a block, and the preimages they commit to
#[derive(Clone, Debug)]
pub struct GlamsterdamAggregationResponder {
    pub pre_execution: PublicOutput,
    pub ranges: Vec<PublicOutput>,
    /// The program outputs' encodings: the pre-execution check, then the ranges
    pub preimages: Vec<Vec<u8>>,
    pub masks: Vec<Vec<u8>>,
}

impl GlamsterdamAggregationResponder {
    /// The inputs of the block the pre-execution output and the range outputs describe
    pub fn of(
        state: &StateConsistencyOutput<Global>,
        outputs: &[GlamsterdamRangeOutput<Global>],
    ) -> Self {
        let pre_execution = ProgramOutput::of_pre_execution(state);
        let range_outputs: Vec<ProgramOutput> =
            outputs.iter().map(ProgramOutput::of_range).collect();
        Self {
            pre_execution: PublicOutput::of_hash(&pre_execution.hash()),
            ranges: range_outputs
                .iter()
                .map(|output| PublicOutput::of_hash(&output.hash()))
                .collect(),
            preimages: core::iter::once(&pre_execution)
                .chain(range_outputs.iter())
                .map(|output| output.as_bytes().to_vec())
                .collect(),
            masks: outputs
                .iter()
                .map(|output| output.accessed.bytes().to_vec())
                .collect(),
        }
    }
}

impl OracleQueryProcessor for GlamsterdamAggregationResponder {
    fn supported_memory_query_ids(&self) -> Vec<u32> {
        vec![
            GLAMSTERDAM_AGGREGATION_PRE_EXECUTION_OUTPUT_QUERY_ID,
            GLAMSTERDAM_AGGREGATION_RANGE_COUNT_QUERY_ID,
            GLAMSTERDAM_AGGREGATION_RANGE_OUTPUT_QUERY_ID,
            GLAMSTERDAM_AGGREGATION_OUTPUT_PREIMAGE_LEN_QUERY_ID,
            GLAMSTERDAM_AGGREGATION_OUTPUT_PREIMAGE_DATA_QUERY_ID,
            GLAMSTERDAM_AGGREGATION_RANGE_MASK_LEN_QUERY_ID,
            GLAMSTERDAM_AGGREGATION_RANGE_MASK_DATA_QUERY_ID,
        ]
    }

    fn process_memory_query(
        &mut self,
        query_id: u32,
        input_word: usize,
        memory: &dyn QuerierMemory,
        mode: RunMode,
        native_run_responses: &mut Vec<u32>,
        guest_run_responses: &mut Vec<u32>,
    ) {
        let index = || -> usize {
            u32::read_input(memory, input_word).expect("a program or range index") as usize
        };
        let indexed = |table: &Vec<Vec<u8>>| -> Vec<u8> {
            let i = index();
            table
                .get(i)
                .unwrap_or_else(|| panic!("no entry {i} for query 0x{query_id:08x}"))
                .clone()
        };
        let mut response = Vec::new();
        match query_id {
            GLAMSTERDAM_AGGREGATION_PRE_EXECUTION_OUTPUT_QUERY_ID => {
                self.pre_execution.write_output(&mut response)
            }
            GLAMSTERDAM_AGGREGATION_RANGE_COUNT_QUERY_ID => {
                (self.ranges.len() as u32).write_output(&mut response)
            }
            GLAMSTERDAM_AGGREGATION_RANGE_OUTPUT_QUERY_ID => {
                self.ranges[index()].write_output(&mut response)
            }
            GLAMSTERDAM_AGGREGATION_OUTPUT_PREIMAGE_LEN_QUERY_ID => {
                (indexed(&self.preimages).len() as u32).write_output(&mut response)
            }
            GLAMSTERDAM_AGGREGATION_OUTPUT_PREIMAGE_DATA_QUERY_ID => {
                write_dynamic_bytes(&indexed(&self.preimages), &mut response)
            }
            GLAMSTERDAM_AGGREGATION_RANGE_MASK_LEN_QUERY_ID => {
                (indexed(&self.masks).len() as u32).write_output(&mut response)
            }
            GLAMSTERDAM_AGGREGATION_RANGE_MASK_DATA_QUERY_ID => {
                write_dynamic_bytes(&indexed(&self.masks), &mut response)
            }
            _ => unreachable!("not an aggregation query: 0x{query_id:08x}"),
        }
        respond_to_every_target(mode, response, native_run_responses, guest_run_responses);
    }
}

/// The oracle of the aggregation program for a block
pub fn make_aggregation_oracle(
    state: &StateConsistencyOutput<Global>,
    outputs: &[GlamsterdamRangeOutput<Global>],
    mode: RunMode,
) -> ZkEENonDeterminismSource {
    let mut oracle = ZkEENonDeterminismSource::new(mode);
    oracle.add_external_processor(GlamsterdamAggregationResponder::of(state, outputs));
    oracle.add_external_processor(UARTPrintResponder);
    oracle
}

/// Runs the aggregation natively while recording the oracle's responses as the
/// RISC-V guest reads them: the words the `glamsterdam_aggregation` program
/// runs from in the simulator
pub fn run_aggregation_recording(
    state: &StateConsistencyOutput<Global>,
    outputs: &[GlamsterdamRangeOutput<Global>],
) -> anyhow::Result<(AggregationOutput<Global>, Vec<u32>)> {
    let oracle = make_aggregation_oracle(state, outputs, RunMode::NativeRunSavingForRiscV);
    let mut source = ReadWitnessSource::new(oracle);
    let output = verify_block_aggregation(&mut source, Global)
        .map_err(|e| anyhow::anyhow!("the programs do not aggregate to the block: {e:?}"))?;
    let words = source.get_read_items().borrow().clone();
    Ok((output, words))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glamsterdam::GlamsterdamBlock;
    use crate::glamsterdam_range::{run_ranges, split_by_gas, IndexTries, InitialValueTables};
    use crate::glamsterdam_state::run_state_consistency_recording;

    /// The aggregation program's check passes over every fixture split into
    /// ranges, and fails when a range is left out or belongs to another block
    #[test]
    fn aggregation_program_accepts_the_fixtures() {
        let dirs = crate::glamsterdam::fixture_dirs(&crate::glamsterdam::default_fixture_dir())
            .expect("fixtures");
        let mut foreign: Option<GlamsterdamRangeOutput<Global>> = None;
        for dir in &dirs {
            let fixture = GlamsterdamBlock::from_dir(dir).expect("fixture");
            let tries = IndexTries::of(&fixture).expect("tries");
            let (state, _) = run_state_consistency_recording(&fixture).expect("pre-execution");
            let tables = InitialValueTables::of(&state);
            let ranges = split_by_gas(&tries, 3).expect("split");
            let mut outputs = run_ranges(&fixture, &tries, &tables, &ranges).expect("ranges");
            let (output, words) = run_aggregation_recording(&state, &outputs).expect("aggregation");
            assert_eq!(output.block_hash, state.header_hash);
            assert_eq!(output.ranges.len(), outputs.len());
            assert!(!words.is_empty());
            if outputs.len() > 1 {
                let last = outputs.pop().unwrap();
                assert!(run_aggregation_recording(&state, &outputs).is_err());
                outputs.push(last);
            }
            if let Some(foreign) = foreign.take() {
                let mixed: Vec<GlamsterdamRangeOutput<Global>> = vec![foreign];
                assert!(run_aggregation_recording(&state, &mixed).is_err());
            }
            foreign = outputs.into_iter().next();
        }
    }
}
