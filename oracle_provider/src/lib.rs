#![allow(clippy::bool_comparison)]
#![allow(clippy::precedence)]
#![allow(clippy::len_zero)]

// Hook zk_ee IOOracle to be NonDeterminismCSRSource
use std::collections::BTreeMap;
use zk_ee::oracle::query_ids::{DISCONNECT_ORACLE_QUERY_ID, UART_QUERY_ID};
use zk_ee::oracle::word_layout::WordLayout;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::{internal_error, oracle::IOOracle};

use riscv_transpiler::vm::NonDeterminismCSRSource;
pub use riscv_transpiler::vm::RamPeek;

pub struct DummyMemorySource;

impl RamPeek for DummyMemorySource {
    fn peek_word(&self, _address: u32) -> u32 {
        unreachable!("DummyMemorySource should not be read from")
    }
}

///
/// Structure that is responsible for buffering incoming queries till the end,
/// and then dispatching them to various responders. When constructed it checks
/// that responders do not try to serve the same query ID.
#[derive(Default)]
pub struct ZkEENonDeterminismSource {
    current_query_id: Option<u32>,
    /// Input buffer: collects u32 words written by the guest.
    input_buffer: Vec<u32>,
    /// Expected number of remaining u32 words for the current input.
    input_remaining: Option<u32>,
    /// Response buffer: u32 words to be read by the guest.
    response_buffer: Vec<u32>,
    /// Cursor into response_buffer for reads.
    response_cursor: usize,
    /// Whether a response length word needs to be returned first.
    response_len_pending: bool,
    is_connected_to_external_oracle: bool,
    /// Vector of different processors that are responsible for handling queries.
    processors: Vec<Box<dyn OracleQueryProcessor + 'static>>,
    /// Mapping from query_id to processor that is handling it (represented as index in processors vector above).
    ranges: BTreeMap<u32, usize>,
}

impl ZkEENonDeterminismSource {
    #[track_caller]
    pub fn add_external_processor<P: OracleQueryProcessor + 'static>(&mut self, processor: P) {
        let query_ids = processor.supported_query_ids();
        let processor_id = self.processors.len();
        for id in query_ids.into_iter() {
            let existing = self.ranges.insert(id, processor_id);
            assert!(
                existing.is_none(),
                "more than one processor for query id 0x{id:08x}"
            );
        }
        self.processors.push(Box::new(processor));
        self.is_connected_to_external_oracle = true;
    }

    fn process_buffered_query(&mut self, memory: &dyn RamPeek) {
        let query_id = self.current_query_id.take().expect("must have query id");
        let input = core::mem::take(&mut self.input_buffer);

        if query_id == DISCONNECT_ORACLE_QUERY_ID {
            self.is_connected_to_external_oracle = false;
            self.response_buffer.clear();
            self.response_cursor = 0;
            self.response_len_pending = false;
        } else {
            let Some(processor_id) = self.ranges.get(&query_id).copied() else {
                panic!("Can not process query with ID = 0x{query_id:08x}");
            };
            let processor = &mut self.processors[processor_id];
            let result = processor
                .process(query_id, &input, memory)
                .unwrap_or_else(|e| {
                    panic!("Query processor failed for ID 0x{query_id:08x}: {e:?}")
                });

            self.response_buffer = result;
            self.response_cursor = 0;
            self.response_len_pending = true;
        }
    }

    /// Reads the next 32 bits from the response buffer.
    fn read_impl(&mut self) -> u32 {
        if self.is_connected_to_external_oracle == false {
            return 0;
        }

        if self.response_len_pending {
            self.response_len_pending = false;
            return self.response_buffer.len() as u32;
        }

        if self.response_cursor < self.response_buffer.len() {
            let value = self.response_buffer[self.response_cursor];
            self.response_cursor += 1;
            value
        } else {
            panic!("trying to read, but data is not prepared");
        }
    }

    fn write_impl(&mut self, memory: &dyn RamPeek, value: u32) {
        // If there's an unconsumed response, clear it
        if self.response_cursor < self.response_buffer.len() || self.response_len_pending {
            self.response_buffer.clear();
            self.response_cursor = 0;
            self.response_len_pending = false;
        }

        if self.current_query_id.is_some() {
            // We're in the middle of buffering input
            if let Some(remaining) = self.input_remaining.as_mut() {
                self.input_buffer.push(value);
                *remaining -= 1;
                if *remaining == 0 {
                    self.input_remaining = None;
                    self.process_buffered_query(memory);
                }
            } else {
                // This word is the input length
                let len = value;
                if len == 0 {
                    self.input_remaining = None;
                    self.process_buffered_query(memory);
                } else {
                    self.input_remaining = Some(len);
                }
            }
        } else {
            if self.is_connected_to_external_oracle == false && value != UART_QUERY_ID {
                return;
            }

            self.current_query_id = Some(value);
            self.input_buffer.clear();
            self.input_remaining = None;
        }
    }
}

impl IOOracle for ZkEENonDeterminismSource {
    fn query<I: WordLayout, O: WordLayout>(
        &mut self,
        query_type: u32,
        input: &I,
    ) -> Result<O, InternalError> {
        if query_type == DISCONNECT_ORACLE_QUERY_ID {
            self.is_connected_to_external_oracle = false;
        }
        if self.is_connected_to_external_oracle == false {
            // Return default-decoded from zero words
            return Ok(O::read_words(&mut || 0));
        }
        let Some(processor) = self.ranges.get(&query_type).copied() else {
            return Err(internal_error!("invalid query ID"));
        };
        let mut input_words = Vec::new();
        input.write_words(&mut |w| input_words.push(w));
        let processor = &mut self.processors[processor];
        let response_words = processor.process(query_type, &input_words, &DummyMemorySource)?;
        let mut cursor = 0;
        let result = O::read_words(&mut || {
            let w = response_words.get(cursor).copied().unwrap_or(0);
            cursor += 1;
            w
        });
        Ok(result)
    }

    fn query_into<I: WordLayout, O: WordLayout>(
        &mut self,
        query_type: u32,
        input: &I,
        output: &mut O,
    ) -> Result<(), InternalError> {
        if query_type == DISCONNECT_ORACLE_QUERY_ID {
            self.is_connected_to_external_oracle = false;
        }
        if self.is_connected_to_external_oracle == false {
            output.read_words_into(&mut || 0);
            return Ok(());
        }
        let Some(processor) = self.ranges.get(&query_type).copied() else {
            return Err(internal_error!("invalid query ID"));
        };
        let mut input_words = Vec::new();
        input.write_words(&mut |w| input_words.push(w));
        let processor = &mut self.processors[processor];
        let response_words = processor.process(query_type, &input_words, &DummyMemorySource)?;
        let mut cursor = 0;
        output.read_words_into(&mut || {
            let w = response_words.get(cursor).copied().unwrap_or(0);
            cursor += 1;
            w
        });
        Ok(())
    }
}

// Now we hook an access
impl NonDeterminismCSRSource for ZkEENonDeterminismSource {
    #[allow(clippy::let_and_return)]
    fn read(&mut self) -> u32 {
        let value = self.read_impl();
        // println!("`NonDeterminismCSRSource` returned 0x{:08x}", value);
        value
    }

    fn write_with_memory_access<R: RamPeek>(&mut self, ram: &R, value: u32) {
        // println!("`NonDeterminismCSRSource` received 0x{:08x}", value);
        self.write_impl(ram, value);
    }

    fn write_with_memory_access_dyn(&mut self, ram: &dyn RamPeek, value: u32) {
        self.write_impl(ram, value);
    }
}

pub trait OracleQueryProcessor {
    /// List of different query ids that are supported (for example NextTxSize or BlockLevelMetadataIterator).
    fn supported_query_ids(&self) -> Vec<u32>;
    fn supports_query_id(&self, query_id: u32) -> bool {
        self.supported_query_ids().contains(&query_id)
    }

    fn process(
        &mut self,
        query_id: u32,
        input: &[u32],
        memory: &dyn RamPeek,
    ) -> Result<Vec<u32>, InternalError>;
}

/// Wraps an inner IOOracle and records all response words for witness generation.
pub struct WitnessRecordingOracle<O: IOOracle> {
    inner: O,
    witness_words: Vec<u32>,
}

impl<O: IOOracle> WitnessRecordingOracle<O> {
    pub fn new(inner: O) -> Self {
        Self {
            inner,
            witness_words: Vec::new(),
        }
    }

    pub fn into_witness(self) -> (O, Vec<u32>) {
        (self.inner, self.witness_words)
    }
}

impl<O: IOOracle> IOOracle for WitnessRecordingOracle<O> {
    fn query<I: WordLayout, R: WordLayout>(
        &mut self,
        query_type: u32,
        input: &I,
    ) -> Result<R, InternalError> {
        let response: R = self.inner.query(query_type, input)?;
        response.write_words(&mut |w| self.witness_words.push(w));
        Ok(response)
    }

    fn query_into<I: WordLayout, R: WordLayout>(
        &mut self,
        query_type: u32,
        input: &I,
        output: &mut R,
    ) -> Result<(), InternalError> {
        self.inner.query_into(query_type, input, output)?;
        output.write_words(&mut |w| self.witness_words.push(w));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_QUERY_ID: u32 = 0x1234_5678;

    struct FixedResponseProcessor;

    impl OracleQueryProcessor for FixedResponseProcessor {
        fn supported_query_ids(&self) -> Vec<u32> {
            vec![TEST_QUERY_ID]
        }

        fn process(
            &mut self,
            query_id: u32,
            input: &[u32],
            _memory: &dyn RamPeek,
        ) -> Result<Vec<u32>, InternalError> {
            assert_eq!(query_id, TEST_QUERY_ID);
            // Input should be the WordLayout encoding of 7u64: [7, 0]
            assert_eq!(input, &[7u32, 0]);
            // Return two u64 values as u32 words
            let mut result = Vec::new();
            // 0x1122_3344_5566_7788u64 as LE u32 words
            result.push(0x5566_7788u32);
            result.push(0x1122_3344u32);
            // 0x99aa_bbcc_ddee_ff00u64 as LE u32 words
            result.push(0xddee_ff00u32);
            result.push(0x99aa_bbccu32);
            Ok(result)
        }
    }

    #[test]
    fn witness_recording_oracle_records_response_words() {
        let mut oracle = ZkEENonDeterminismSource::default();
        oracle.add_external_processor(FixedResponseProcessor);

        let mut source = WitnessRecordingOracle::new(oracle);
        let response: [u64; 2] = source.query(TEST_QUERY_ID, &7u64).unwrap();
        assert_eq!(
            response,
            [0x1122_3344_5566_7788u64, 0x99aa_bbcc_ddee_ff00u64]
        );
        let (_inner, witness) = source.into_witness();
        // witness should contain the u32 LE words of the response
        assert_eq!(
            witness,
            vec![
                0x5566_7788u32,
                0x1122_3344u32,
                0xddee_ff00u32,
                0x99aa_bbccu32
            ]
        );
    }
}
