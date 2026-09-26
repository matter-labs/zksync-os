#![allow(clippy::bool_comparison)]
#![allow(clippy::precedence)]
#![allow(clippy::len_zero)]

#[cfg(all(
    not(target_arch = "riscv32"),
    not(all(target_pointer_width = "64", target_endian = "little"))
))]
compile_error!("ReadWitnessSource host recording requires a 64-bit little-endian host target");

// Hook zk_ee IOOracle to be NonDeterminismCSRSource

use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::rc::Rc;
use zk_ee::oracle::memory_io::host::{NativeQuerierMemory, QuerierMemory};
use zk_ee::oracle::memory_io::MemoryOracle;
use zk_ee::oracle::query_ids::{DISCONNECT_ORACLE_QUERY_ID, UART_QUERY_ID};
use zk_ee::oracle::usize_serialization::{UsizeDeserializable, UsizeSerializable};
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

/// The memory of the RISC-V guest, for the processors of memory-based queries (see
/// [`OracleQueryProcessor::process_memory_query`]).
pub struct GuestMemory<'a, R: RamPeek + ?Sized>(pub &'a R);

impl<R: RamPeek + ?Sized> QuerierMemory for GuestMemory<'_, R> {
    fn word_size(&self) -> usize {
        size_of::<u32>()
    }

    fn read_u32(&self, address: usize) -> Result<u32, InternalError> {
        let address = u32::try_from(address)
            .map_err(|_| internal_error!("guest address does not fit into u32"))?;
        if !address.is_multiple_of(4) {
            return Err(internal_error!("unaligned guest address"));
        }
        Ok(self.0.peek_word(address))
    }
}

/// Sized adapter so unsized peekers (`[u32]`, generic `R: ?Sized`) can be
/// passed to the `&dyn RamPeek` based query processors.
struct PeekRef<'a, R: RamPeek + ?Sized>(&'a R);

impl<R: RamPeek + ?Sized> RamPeek for PeekRef<'_, R> {
    #[inline(always)]
    fn peek_word(&self, address: u32) -> u32 {
        self.0.peek_word(address)
    }
}

/// The run an oracle serves: where the queries come from, and which responses its processors produce
/// (see [`OracleQueryProcessor::process_memory_query`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RunMode {
    /// A native run: the queries come from a querier in this process, and are answered for it only.
    #[default]
    NativeRunOnly,
    /// A native run that records the prover input: the queries come from a querier in this process, and
    /// are also answered for the RISC-V guest, whose responses are saved for its replay (see
    /// [`ReadWitnessSource`]).
    NativeRunSavingForRiscV,
    /// A run of the RISC-V guest in the simulator: the queries, and the memory pointers in them, come from
    /// the guest, and are answered for it only.
    RiscVRun,
}

impl RunMode {
    /// Whether the queries come from a querier in this process, which the responses of the native run are
    /// produced for
    pub const fn produces_native_run_responses(self) -> bool {
        match self {
            Self::NativeRunOnly | Self::NativeRunSavingForRiscV => true,
            Self::RiscVRun => false,
        }
    }

    /// Whether the responses of the RISC-V guest run are produced: to serve the guest, or to save them for
    /// its replay
    pub const fn produces_guest_run_responses(self) -> bool {
        match self {
            Self::NativeRunOnly => false,
            Self::NativeRunSavingForRiscV | Self::RiscVRun => true,
        }
    }
}

///
/// Structure that is responsible for buffering incoming queries till the end,
/// and then dispatching them to various responders. When constructed it checks
/// that responders do not try to serve the same query ID.
#[derive(Default)]
pub struct ZkEENonDeterminismSource {
    /// The run the oracle serves, which the processors of memory-based queries respond for.
    mode: RunMode,
    query_buffer: Option<QueryBuffer>,
    current_query_id: Option<u32>,
    current_iterator: Option<Box<dyn ExactSizeIterator<Item = usize> + 'static>>,
    iterator_len_to_indicate: Option<u32>,
    high_half: Option<u32>,
    is_connected_to_external_oracle: bool,
    /// Vector of different processors that are responsible for handling queries.
    processors: Vec<Box<dyn OracleQueryProcessor + 'static>>,
    /// Mapping from query_id to processor that is handling it (represented as index in processors vector above).
    ranges: BTreeMap<u32, usize>,
    /// The same for the queries served with the memory-based protocol.
    memory_query_ranges: BTreeMap<u32, usize>,
    /// Words of the response to the current memory-based query that the querier did not read yet.
    memory_response: VecDeque<u32>,
    /// The response of the RISC-V guest run to the current memory-based query of a querier in this process:
    /// the words the guest reads in place of the querier, for a native run that records them (see
    /// [`ReadWitnessSource`]).
    guest_run_response: Vec<u32>,
    /// The memory-based query whose input word the guest sends next, when running as the CSR source.
    memory_query_awaiting_input: Option<u32>,
}

impl ZkEENonDeterminismSource {
    /// An oracle without processors for the run `mode` (the default is a native run only).
    pub fn new(mode: RunMode) -> Self {
        Self {
            mode,
            ..Default::default()
        }
    }

    /// Sets the run the oracle serves, before it serves queries.
    pub fn set_run_mode(&mut self, mode: RunMode) {
        self.mode = mode;
    }

    #[track_caller]
    pub fn add_external_processor<P: OracleQueryProcessor + 'static>(&mut self, processor: P) {
        let processor_id = self.processors.len();
        for id in processor.supported_query_ids() {
            let existing = self.ranges.insert(id, processor_id);
            assert!(
                existing.is_none() && !self.memory_query_ranges.contains_key(&id),
                "more than one processor for query id 0x{id:08x}"
            );
        }
        for id in processor.supported_memory_query_ids() {
            let existing = self.memory_query_ranges.insert(id, processor_id);
            assert!(
                existing.is_none() && !self.ranges.contains_key(&id),
                "more than one processor for query id 0x{id:08x}"
            );
        }
        self.processors.push(Box::new(processor));
        self.is_connected_to_external_oracle = true;
    }

    /// Serves a memory-based query of the querier of the run, and keeps the response for it to read, and in
    /// a native run that records the prover input the response of the RISC-V guest run.
    fn process_memory_query(
        &mut self,
        query_id: u32,
        input_word: usize,
        memory: &dyn QuerierMemory,
    ) -> Result<(), InternalError> {
        let Some(processor_id) = self.memory_query_ranges.get(&query_id).copied() else {
            return Err(internal_error!("invalid query ID"));
        };
        let mode = self.mode;
        let mut native_run_responses = Vec::new();
        let mut guest_run_responses = Vec::new();
        self.processors[processor_id].process_memory_query(
            query_id,
            input_word,
            memory,
            mode,
            &mut native_run_responses,
            &mut guest_run_responses,
        );
        // the responses of the runs of the mode, and only those
        let responds_for_mode = match mode {
            RunMode::NativeRunOnly => guest_run_responses.is_empty(),
            RunMode::NativeRunSavingForRiscV => {
                native_run_responses.is_empty() == guest_run_responses.is_empty()
            }
            RunMode::RiscVRun => native_run_responses.is_empty(),
        };
        if !responds_for_mode {
            return Err(internal_error!(
                "the query processor does not respond for the run mode"
            ));
        }
        match mode {
            RunMode::NativeRunOnly => self.memory_response = native_run_responses.into(),
            RunMode::NativeRunSavingForRiscV => {
                self.memory_response = native_run_responses.into();
                self.guest_run_response = guest_run_responses;
            }
            RunMode::RiscVRun => self.memory_response = guest_run_responses.into(),
        }
        Ok(())
    }

    fn process_buffered_query(&mut self, memory: &dyn RamPeek) {
        assert!(self.current_iterator.is_none());
        assert!(self.current_query_id.is_none());

        let buffer = self.query_buffer.take().expect("must exist");
        let query_id = buffer.query_type;
        let buffer = buffer.buffer;
        let Some(processor_id) = self.ranges.get(&query_id).copied() else {
            panic!("Can not process query with ID = 0x{query_id:08x}");
        };
        let processor = &mut self.processors[processor_id];
        let new_iterator = processor.process_buffered_query(query_id, buffer, memory);

        let result_len = new_iterator.len() * 2; // NOTE for mismatch of 32/64-bit archs
        self.iterator_len_to_indicate = Some(result_len as u32);
        if result_len > 0 {
            self.current_query_id = Some(query_id);
            self.current_iterator = Some(new_iterator);
        }
    }

    /// Reads the next 32bits.
    /// Our iterators and queues hold usize elements (u64), so we have to do some splitting and caching.
    fn read_impl(&mut self) -> u32 {
        // We mocked reads, so it's filtered out before
        if self.is_connected_to_external_oracle == false {
            return 0;
        }

        if let Some(word) = self.memory_response.pop_front() {
            return word;
        }

        if let Some(iterator_len_to_indicate) = self.iterator_len_to_indicate.take() {
            return iterator_len_to_indicate;
        }

        // This is the 32 bits remaining from the previous item - return them now.
        if let Some(high) = self.high_half.take() {
            return high;
        }
        // If we didn't have any partial data left, we should fetch another element from the iterator.
        let Some(current_iterator) = self.current_iterator.as_mut() else {
            panic!("trying to read, but data is not prepared");
        };
        let next = current_iterator.next().expect("must contain next element");
        if current_iterator.len() == 0 {
            // we are done - there are no more elements left after this one.
            self.current_query_id = None;
            self.current_iterator = None;
        }
        // Split the 64 bits into 2 pieces - one is put into 'high' field, to be returned later
        // and the other one is returned immediately.
        let high = (next >> 32) as u32;
        let low = next as u32;
        self.high_half = Some(high);

        low
    }

    fn write_impl(&mut self, memory: &dyn RamPeek, value: u32) {
        if self.current_query_id.is_some() {
            println!(
                "Current query ID = 0x{:08x} iterator is not consumed in full, but received value 0x{:08x}",
                self.current_query_id.unwrap(),
                value
            );
            self.current_query_id = None;
        }

        // may have something from remains
        if self.current_iterator.is_some() {
            if self.current_iterator.as_ref().unwrap().len() != 0 {
                println!(
                    "Current iterator is not consumed in full, but received value 0x{value:08x}"
                );
            }
            self.current_iterator = None;
        }
        if self.iterator_len_to_indicate.is_some() {
            self.iterator_len_to_indicate = None;
        }
        if self.high_half.is_some() {
            self.high_half = None;
        }
        if !self.memory_response.is_empty() {
            println!(
                "Response to a memory-based query is not consumed in full, but received value 0x{value:08x}"
            );
            self.memory_response.clear();
        }

        if let Some(query_id) = self.memory_query_awaiting_input.take() {
            // a memory-based query is two words: the ID and the input word, which may be a guest
            // address that the processor reads through while the guest waits
            if query_id == DISCONNECT_ORACLE_QUERY_ID {
                self.is_connected_to_external_oracle = false;
            } else {
                assert_eq!(
                    self.mode,
                    RunMode::RiscVRun,
                    "the oracle serves a native run, not the RISC-V guest"
                );
                self.process_memory_query(query_id, value as usize, &GuestMemory(memory))
                    .expect("must serve the memory-based query");
            }
        } else if let Some(query_buffer) = self.query_buffer.as_mut() {
            let complete = query_buffer.write(value);
            if complete {
                self.process_buffered_query(memory);
            }
        } else {
            if self.is_connected_to_external_oracle == false && value != UART_QUERY_ID {
                // we are not interested in general to start another query
                return;
            }

            if self.memory_query_ranges.contains_key(&value) || value == DISCONNECT_ORACLE_QUERY_ID
            {
                self.memory_query_awaiting_input = Some(value);
                return;
            }

            let new_buffer = QueryBuffer::empty_for_query_type(value);
            self.query_buffer = Some(new_buffer);
        }
    }
}

impl IOOracle for ZkEENonDeterminismSource {
    type RawIterator<'a> = Box<dyn ExactSizeIterator<Item = usize> + 'static>;

    fn raw_query<'a, I: UsizeSerializable + UsizeDeserializable>(
        &'a mut self,
        query_type: u32,
        input: &I,
    ) -> Result<Self::RawIterator<'a>, InternalError> {
        if self.is_connected_to_external_oracle == false {
            return Ok(Box::new([].into_iter()));
        }
        let Some(processor) = self.ranges.get(&query_type).copied() else {
            return Err(internal_error!("invalid query ID"));
        };
        let processor = &mut self.processors[processor];
        let response = processor.process_buffered_query(
            query_type,
            UsizeSerializable::iter(input).collect::<Vec<usize>>(),
            &DummyMemorySource,
        );

        Ok(response)
    }
}

/// The querier runs in this process (forward mode, and the native run that records the prover input).
impl MemoryOracle for ZkEENonDeterminismSource {
    fn send_query(&mut self, query_id: u32, input_word: usize) -> Result<(), InternalError> {
        if !self.mode.produces_native_run_responses() {
            return Err(internal_error!(
                "the oracle serves the RISC-V guest, not a querier in this process"
            ));
        }
        self.guest_run_response.clear();
        if !self.memory_response.is_empty() {
            self.memory_response.clear();
            return Err(internal_error!(
                "previous oracle response was not consumed in full"
            ));
        }
        if query_id == DISCONNECT_ORACLE_QUERY_ID {
            self.is_connected_to_external_oracle = false;
        }
        if self.is_connected_to_external_oracle == false {
            // as when running as the CSR source, reads return zeroes
            return Ok(());
        }
        // SAFETY: the querier of this process exposes the memory the input word refers to while it sends
        // the query, i.e. during this call
        let memory = unsafe { NativeQuerierMemory::new() };
        self.process_memory_query(query_id, input_word, &memory)
    }

    fn read_word(&mut self) -> Result<u32, InternalError> {
        match self.memory_response.pop_front() {
            Some(word) => Ok(word),
            None if self.is_connected_to_external_oracle == false => Ok(0),
            None => Err(internal_error!("oracle response is shorter than expected")),
        }
    }

    unsafe fn write_words(&mut self, dst: *mut u32, num_words: usize) -> Result<(), InternalError> {
        if self.memory_response.len() < num_words {
            if self.is_connected_to_external_oracle == false {
                // SAFETY: guaranteed by the caller
                unsafe { dst.write_bytes(0, num_words) };
                return Ok(());
            }
            return Err(internal_error!("oracle response is shorter than expected"));
        }
        for (i, word) in self.memory_response.drain(..num_words).enumerate() {
            // SAFETY: `i < num_words`, and the caller guarantees `dst` is valid for `num_words` words
            unsafe { dst.add(i).write(word) };
        }
        Ok(())
    }

    fn finish_query(&mut self) -> Result<(), InternalError> {
        if self.memory_response.is_empty() {
            Ok(())
        } else {
            self.memory_response.clear();
            Err(internal_error!("oracle response contains excess data"))
        }
    }
}

pub trait OracleQueryProcessor {
    /// IDs of the queries served with the iterator-based protocol (for example BlockLevelMetadataIterator).
    fn supported_query_ids(&self) -> Vec<u32> {
        Vec::new()
    }
    fn supports_query_id(&self, query_id: u32) -> bool {
        self.supported_query_ids().contains(&query_id)
    }

    fn process_buffered_query(
        &mut self,
        query_id: u32,
        _query: Vec<usize>,
        _memory: &dyn RamPeek,
    ) -> Box<dyn ExactSizeIterator<Item = usize> + 'static + Send + Sync> {
        panic!("query ID 0x{query_id:08x} is not served with the iterator-based protocol")
    }

    /// IDs of the queries served with the memory-based protocol (`zk_ee::oracle::memory_io`).
    fn supported_memory_query_ids(&self) -> Vec<u32> {
        Vec::new()
    }

    /// Serves a query of the memory-based protocol. `input_word` is the input word the querier sent, and
    /// `memory` reads the memory of the querier it may refer to (see `zk_ee::oracle::memory_io::host`).
    /// `mode` tells the querier apart: a querier in this process (a native run), or the RISC-V guest in the
    /// simulator, which the memory pointers of the query then belong to.
    ///
    /// Appends the response words, exactly as the querier reads them, for the runs of `mode`: to
    /// `native_run_responses` as a querier in this process reads them, to `guest_run_responses` as the
    /// RISC-V guest reads them. The oracle serves the responses of its querier, and a native run that
    /// records the prover input saves the responses of the guest run, which the guest reads in place of
    /// the native querier on replay (see [`ReadWitnessSource`]); a run on the RISC-V guest has no
    /// responses of a native run. A response that is the same on every target goes through
    /// [`respond_to_every_target`]; the field hints, for example, follow the representation of field
    /// elements on each target.
    fn process_memory_query(
        &mut self,
        query_id: u32,
        _input_word: usize,
        _memory: &dyn QuerierMemory,
        _mode: RunMode,
        _native_run_responses: &mut Vec<u32>,
        _guest_run_responses: &mut Vec<u32>,
    ) {
        panic!("query ID 0x{query_id:08x} is not served with the memory-based protocol")
    }
}

/// Appends a response that is the same on every target to the responses of the runs of `mode` (see
/// [`OracleQueryProcessor::process_memory_query`]).
pub fn respond_to_every_target(
    mode: RunMode,
    mut response: Vec<u32>,
    native_run_responses: &mut Vec<u32>,
    guest_run_responses: &mut Vec<u32>,
) {
    match mode {
        RunMode::NativeRunOnly => native_run_responses.append(&mut response),
        RunMode::NativeRunSavingForRiscV => {
            guest_run_responses.extend_from_slice(&response);
            native_run_responses.append(&mut response);
        }
        RunMode::RiscVRun => guest_run_responses.append(&mut response),
    }
}

struct QueryBuffer {
    query_type: u32,
    remaining_len: Option<usize>,
    write_low: bool,
    buffer: Vec<usize>,
}

impl QueryBuffer {
    fn empty_for_query_type(query_type: u32) -> Self {
        Self {
            query_type,
            remaining_len: None,
            write_low: true,
            buffer: Vec::new(),
        }
    }

    fn write(&mut self, value: u32) -> bool {
        // NOTE: we have to match between 32 bit inner env and 64 bit outer env
        if let Some(remaining_len) = self.remaining_len.as_mut() {
            // println!("Writing word 0x{:08x} for query ID = 0x{:08x}", value, self.query_type);
            if self.write_low {
                self.buffer.push(value as usize);
                self.write_low = false;
            } else {
                let last = self.buffer.last_mut().unwrap();
                *last |= (value as usize) << 32;
                self.write_low = true;
            }
            *remaining_len -= 1;

            *remaining_len == 0
        } else {
            // println!("Expecting {} words for query ID = 0x{:08x}", value, self.query_type);
            self.remaining_len = Some(value as usize);
            if value == 0 {
                // nothing else to expect
                true
            } else {
                false
            }
        }
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

    fn write_with_memory_access<R: RamPeek + ?Sized>(&mut self, ram: &R, value: u32) {
        // println!("`NonDeterminismCSRSource` received 0x{:08x}", value);
        self.write_impl(&PeekRef(ram), value);
    }

    fn write_with_memory_access_raw(&mut self, ram: &[u32], value: u32) {
        self.write_impl(&PeekRef(ram), value);
    }

    fn write_with_memory_access_dyn(&mut self, ram: &dyn RamPeek, value: u32) {
        self.write_impl(ram, value);
    }
}

/// Wraps the original source and remembers all the read accesses.
pub struct ReadWitnessSource {
    original_source: ZkEENonDeterminismSource,
    read_items: Rc<RefCell<Vec<u32>>>,
}

impl ReadWitnessSource {
    pub fn new(original_source: ZkEENonDeterminismSource) -> Self {
        Self {
            original_source,
            read_items: Rc::new(RefCell::new(vec![])),
        }
    }

    pub fn get_read_items(&self) -> Rc<RefCell<Vec<u32>>> {
        self.read_items.clone()
    }
}

impl NonDeterminismCSRSource for ReadWitnessSource {
    fn read(&mut self) -> u32 {
        let item = self.original_source.read();
        // On read - remember the items.
        self.read_items.borrow_mut().push(item);
        item
    }

    fn write_with_memory_access<R: RamPeek + ?Sized>(&mut self, ram: &R, value: u32) {
        self.original_source.write_with_memory_access(ram, value);
    }

    fn write_with_memory_access_raw(&mut self, ram: &[u32], value: u32) {
        self.original_source
            .write_with_memory_access_raw(ram, value);
    }

    fn write_with_memory_access_dyn(&mut self, ram: &dyn RamPeek, value: u32) {
        self.original_source
            .write_with_memory_access_dyn(ram, value);
    }
}

impl IOOracle for ReadWitnessSource {
    type RawIterator<'a> = <ZkEENonDeterminismSource as IOOracle>::RawIterator<'a>;

    fn raw_query<'a, I>(
        &'a mut self,
        query_type: u32,
        input: &I,
    ) -> Result<Self::RawIterator<'a>, InternalError>
    where
        I: UsizeSerializable + UsizeDeserializable,
    {
        let inner = self.original_source.raw_query(query_type, input)?;
        // First add the length of the iterator.
        let len = inner.len();
        {
            let mut read_items = self.read_items.borrow_mut();
            // Len is multiplied by 2 to account for 32/64-bit mismatch
            let len_u32 = u32::try_from(len.checked_mul(2).expect("response length overflow"))
                .expect("iterator length does not fit into u32");
            read_items.push(len_u32);
        }
        let read_items = Rc::clone(&self.read_items);
        let wrapped: Self::RawIterator<'a> = Box::new(inner.inspect(move |v| {
            record_usize_as_u32_words(&mut read_items.borrow_mut(), *v);
        }));

        Ok(wrapped)
    }
}

/// Records the responses of the RISC-V guest run, the words the guest reads in place of the querier of this
/// process, which the processors produce along with the responses they serve in the
/// [`RunMode::NativeRunSavingForRiscV`] mode (see [`OracleQueryProcessor::process_memory_query`]). A
/// disconnected oracle does not respond, and the zeroes the querier reads then are recorded as read: the
/// guest reads zeroes then too.
impl MemoryOracle for ReadWitnessSource {
    fn send_query(&mut self, query_id: u32, input_word: usize) -> Result<(), InternalError> {
        if self.original_source.mode != RunMode::NativeRunSavingForRiscV {
            return Err(internal_error!(
                "recording the prover input needs the run mode that saves the responses for the RISC-V guest"
            ));
        }
        self.original_source.send_query(query_id, input_word)?;
        let guest_run_response = core::mem::take(&mut self.original_source.guest_run_response);
        self.read_items.borrow_mut().extend(guest_run_response);
        Ok(())
    }

    fn read_word(&mut self) -> Result<u32, InternalError> {
        let word = self.original_source.read_word()?;
        if !self.original_source.is_connected_to_external_oracle {
            self.read_items.borrow_mut().push(word);
        }
        Ok(word)
    }

    unsafe fn write_words(&mut self, dst: *mut u32, num_words: usize) -> Result<(), InternalError> {
        // SAFETY: guaranteed by the caller
        unsafe { self.original_source.write_words(dst, num_words)? };
        if !self.original_source.is_connected_to_external_oracle {
            // SAFETY: the words were just written
            let words = unsafe { core::slice::from_raw_parts(dst, num_words) };
            self.read_items.borrow_mut().extend_from_slice(words);
        }
        Ok(())
    }

    fn finish_query(&mut self) -> Result<(), InternalError> {
        self.original_source.finish_query()
    }
}

fn record_usize_as_u32_words(dst: &mut Vec<u32>, value: usize) {
    {
        let v = value as u64;
        // LE
        dst.push((v & 0xFFFF_FFFF) as u32);
        dst.push((v >> 32) as u32);
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

        fn process_buffered_query(
            &mut self,
            query_id: u32,
            query: Vec<usize>,
            _memory: &dyn RamPeek,
        ) -> Box<dyn ExactSizeIterator<Item = usize> + 'static + Send + Sync> {
            assert_eq!(query_id, TEST_QUERY_ID);
            assert_eq!(query, vec![7usize]);
            Box::new(vec![0x1122_3344_5566_7788usize, 0x99aa_bbcc_ddee_ff00usize].into_iter())
        }
    }

    #[test]
    fn read_witness_source_records_length_and_words() {
        let mut oracle = ZkEENonDeterminismSource::default();
        oracle.add_external_processor(FixedResponseProcessor);

        let mut source = ReadWitnessSource::new(oracle);
        let response: Vec<usize> = source.raw_query(TEST_QUERY_ID, &7u64).unwrap().collect();
        assert_eq!(
            response,
            vec![0x1122_3344_5566_7788usize, 0x99aa_bbcc_ddee_ff00usize]
        );
        assert_eq!(
            *source.get_read_items().borrow(),
            vec![4, 0x5566_7788, 0x1122_3344, 0xddee_ff00, 0x99aa_bbcc,]
        );
    }

    use zk_ee::oracle::memory_io::host::{write_dynamic_bytes, ReadQueryInput, WriteQueryOutput};
    use zk_ee::oracle::memory_io::{DynamicOracleQuery, OracleQuery};
    use zk_ee::utils::Bytes32;

    const SWAP_QUERY_ID: u32 = 0x1234_0001;
    const BYTES_QUERY_ID: u32 = 0x1234_0002;

    /// A composite input, and a composite output.
    struct SwapQuery;

    impl OracleQuery for SwapQuery {
        const QUERY_ID: u32 = SWAP_QUERY_ID;
        type Input = (Bytes32, Bytes32);
        type Output = (Bytes32, Bytes32);
    }

    /// An input passed by address, and a dynamically sized output of the first 13 bytes of it.
    struct BytesQuery;

    impl DynamicOracleQuery for BytesQuery {
        const QUERY_ID: u32 = BYTES_QUERY_ID;
        type Input = Bytes32;
    }

    struct MemoryProcessor;

    impl OracleQueryProcessor for MemoryProcessor {
        fn supported_query_ids(&self) -> Vec<u32> {
            vec![]
        }

        fn process_buffered_query(
            &mut self,
            _query_id: u32,
            _query: Vec<usize>,
            _memory: &dyn RamPeek,
        ) -> Box<dyn ExactSizeIterator<Item = usize> + 'static + Send + Sync> {
            unreachable!()
        }

        fn supported_memory_query_ids(&self) -> Vec<u32> {
            vec![SWAP_QUERY_ID, BYTES_QUERY_ID]
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
            let mut response = vec![];
            match query_id {
                SWAP_QUERY_ID => {
                    let (a, b) = <(Bytes32, Bytes32)>::read_input(memory, input_word).unwrap();
                    (b, a).write_output(&mut response);
                }
                BYTES_QUERY_ID => {
                    let value = Bytes32::read_input(memory, input_word).unwrap();
                    write_dynamic_bytes(&value.as_u8_array_ref()[..13], &mut response);
                }
                _ => unreachable!(),
            }
            respond_to_every_target(mode, response, native_run_responses, guest_run_responses);
        }
    }

    const REPRESENTATION_QUERY_ID: u32 = 0x1234_0003;

    /// Echoes a value, in a representation that depends on the target: as is for a querier in this
    /// process, with the words in reverse order for the guest.
    struct RepresentationQuery;

    impl OracleQuery for RepresentationQuery {
        const QUERY_ID: u32 = REPRESENTATION_QUERY_ID;
        type Input = Bytes32;
        type Output = Bytes32;
    }

    /// Serves [`RepresentationQuery`] for the runs of the mode, or, when it does not follow the mode,
    /// for a native run only.
    struct RepresentationProcessor {
        follows_mode: bool,
    }

    impl OracleQueryProcessor for RepresentationProcessor {
        fn supported_memory_query_ids(&self) -> Vec<u32> {
            vec![REPRESENTATION_QUERY_ID]
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
            assert_eq!(query_id, REPRESENTATION_QUERY_ID);
            let value = Bytes32::read_input(memory, input_word).unwrap();
            let words = words(value.as_u8_array_ref());
            if !self.follows_mode || mode.produces_native_run_responses() {
                native_run_responses.extend_from_slice(&words);
            }
            if self.follows_mode && mode.produces_guest_run_responses() {
                guest_run_responses.extend(words.iter().rev());
            }
        }
    }

    const REPRESENTATION: RepresentationProcessor = RepresentationProcessor { follows_mode: true };

    fn bytes32(seed: u8) -> Bytes32 {
        Bytes32::from_array(core::array::from_fn(|i| seed.wrapping_add(i as u8)))
    }

    fn words(bytes: &[u8]) -> Vec<u32> {
        bytes
            .chunks(4)
            .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
            .collect()
    }

    #[test]
    fn memory_queries_in_process_record_the_words_the_guest_reads() {
        let mut oracle = ZkEENonDeterminismSource::new(RunMode::NativeRunSavingForRiscV);
        oracle.add_external_processor(MemoryProcessor);
        let mut source = ReadWitnessSource::new(oracle);
        let (a, b) = (bytes32(1), bytes32(100));

        assert_eq!(SwapQuery::get(&mut source, (&a, &b)).unwrap(), (b, a));

        let mut vector = Vec::with_capacity(4);
        assert_eq!(
            BytesQuery::get_into(&mut source, &a, &mut vector).unwrap(),
            2
        );
        let bytes: Vec<u8> = vector.iter().flat_map(|word| word.to_le_bytes()).collect();
        assert_eq!(bytes[..13], a.as_u8_array_ref()[..13]);
        assert_eq!(bytes[13..], [0, 0, 0]);

        // no length prefix for the fixed-size output; the claim of the dynamic one counts u32 words
        let mut padded = a.as_u8_array_ref()[..16].to_vec();
        padded[13..].fill(0);
        let expected = [
            words(b.as_u8_array_ref()),
            words(a.as_u8_array_ref()),
            vec![4],
            words(&padded),
        ]
        .concat();
        assert_eq!(*source.get_read_items().borrow(), expected);
    }

    #[test]
    fn memory_queries_as_the_csr_source_of_the_guest() {
        let mut oracle = ZkEENonDeterminismSource::new(RunMode::RiscVRun);
        oracle.add_external_processor(FixedResponseProcessor);
        oracle.add_external_processor(MemoryProcessor);
        let (a, b) = (bytes32(1), bytes32(100));
        // guest memory: `a` at 0x100, `b` at 0x200, and the address array of the composite at 0x300
        let mut ram = [0u32; 256];
        ram[0x40..0x48].copy_from_slice(&words(a.as_u8_array_ref()));
        ram[0x80..0x88].copy_from_slice(&words(b.as_u8_array_ref()));
        ram[0xc0] = 0x100;
        ram[0xc1] = 0x200;

        for _ in 0..2 {
            // a memory-based query: the ID and the input word, then exactly the response words
            oracle.write_with_memory_access(&ram, SWAP_QUERY_ID);
            oracle.write_with_memory_access(&ram, 0x300);
            let response: Vec<u32> = (0..16).map(|_| oracle.read()).collect();
            assert_eq!(
                response,
                [words(b.as_u8_array_ref()), words(a.as_u8_array_ref())].concat()
            );

            // an iterator-based query in between keeps its framing
            for word in [TEST_QUERY_ID, 2, 7, 0] {
                oracle.write_with_memory_access(&ram, word);
            }
            let response: Vec<u32> = (0..5).map(|_| oracle.read()).collect();
            assert_eq!(
                response,
                [4, 0x5566_7788, 0x1122_3344, 0xddee_ff00, 0x99aa_bbcc]
            );
        }
    }

    fn reversed_words(value: &Bytes32) -> Vec<u32> {
        let mut words = words(value.as_u8_array_ref());
        words.reverse();
        words
    }

    #[test]
    fn a_native_run_is_served_its_responses_and_records_those_of_the_guest_run() {
        let (a, b) = (bytes32(1), bytes32(100));

        let mut oracle = ZkEENonDeterminismSource::new(RunMode::NativeRunOnly);
        oracle.add_external_processor(REPRESENTATION);
        assert_eq!(RepresentationQuery::get(&mut oracle, &a).unwrap(), a);

        let mut oracle = ZkEENonDeterminismSource::new(RunMode::NativeRunSavingForRiscV);
        oracle.add_external_processor(REPRESENTATION);
        oracle.add_external_processor(MemoryProcessor);
        let mut source = ReadWitnessSource::new(oracle);
        assert_eq!(RepresentationQuery::get(&mut source, &a).unwrap(), a);
        // a response that is the same on every target
        assert_eq!(SwapQuery::get(&mut source, (&a, &b)).unwrap(), (b, a));
        let expected = [
            reversed_words(&a),
            words(b.as_u8_array_ref()),
            words(a.as_u8_array_ref()),
        ]
        .concat();
        assert_eq!(*source.get_read_items().borrow(), expected);
    }

    #[test]
    fn the_guest_is_served_the_responses_of_the_guest_run() {
        let mut oracle = ZkEENonDeterminismSource::new(RunMode::RiscVRun);
        oracle.add_external_processor(REPRESENTATION);
        let a = bytes32(1);
        // guest memory: `a` at 0x100
        let mut ram = [0u32; 128];
        ram[0x40..0x48].copy_from_slice(&words(a.as_u8_array_ref()));
        oracle.write_with_memory_access(&ram, REPRESENTATION_QUERY_ID);
        oracle.write_with_memory_access(&ram, 0x100);
        let response: Vec<u32> = (0..8).map(|_| oracle.read()).collect();
        assert_eq!(response, reversed_words(&a));
    }

    #[test]
    fn recording_needs_the_mode_that_saves_the_responses_of_the_guest_run() {
        let mut oracle = ZkEENonDeterminismSource::new(RunMode::NativeRunOnly);
        oracle.add_external_processor(MemoryProcessor);
        let mut source = ReadWitnessSource::new(oracle);
        let a = bytes32(1);
        assert!(SwapQuery::get(&mut source, (&a, &a)).is_err());
        assert!(source.get_read_items().borrow().is_empty());
    }

    #[test]
    fn a_processor_that_does_not_respond_for_the_mode_is_rejected() {
        let mut oracle = ZkEENonDeterminismSource::new(RunMode::NativeRunSavingForRiscV);
        oracle.add_external_processor(RepresentationProcessor {
            follows_mode: false,
        });
        oracle.add_external_processor(MemoryProcessor);
        let mut source = ReadWitnessSource::new(oracle);
        let a = bytes32(1);
        assert!(RepresentationQuery::get(&mut source, &a).is_err());
        // the failed query is over
        assert_eq!(SwapQuery::get(&mut source, (&a, &a)).unwrap(), (a, a));
        assert_eq!(
            *source.get_read_items().borrow(),
            [words(a.as_u8_array_ref()), words(a.as_u8_array_ref())].concat()
        );
    }

    #[test]
    fn a_querier_in_this_process_is_not_served_in_a_risc_v_run() {
        let mut oracle = ZkEENonDeterminismSource::new(RunMode::RiscVRun);
        oracle.add_external_processor(REPRESENTATION);
        assert!(RepresentationQuery::get(&mut oracle, &bytes32(1)).is_err());
    }

    #[test]
    #[should_panic(expected = "not the RISC-V guest")]
    fn the_guest_is_not_served_in_a_native_run() {
        let mut oracle = ZkEENonDeterminismSource::new(RunMode::NativeRunSavingForRiscV);
        oracle.add_external_processor(REPRESENTATION);
        let ram = [0u32; 128];
        oracle.write_with_memory_access(&ram, REPRESENTATION_QUERY_ID);
        oracle.write_with_memory_access(&ram, 0x100);
    }

    #[test]
    fn a_disconnected_oracle_records_the_zeroes_read() {
        let mut oracle = ZkEENonDeterminismSource::new(RunMode::NativeRunSavingForRiscV);
        oracle.add_external_processor(REPRESENTATION);
        let mut source = ReadWitnessSource::new(oracle);
        <zk_ee::oracle::basic_queries::DisconnectOracleQuery as OracleQuery>::get(&mut source, ())
            .unwrap();
        assert_eq!(
            RepresentationQuery::get(&mut source, &bytes32(1)).unwrap(),
            Bytes32::ZERO
        );
        assert_eq!(*source.get_read_items().borrow(), vec![0; 8]);
    }

    #[test]
    #[should_panic(expected = "more than one processor")]
    fn a_query_id_is_served_with_one_protocol() {
        struct IteratorSwap;

        impl OracleQueryProcessor for IteratorSwap {
            fn supported_query_ids(&self) -> Vec<u32> {
                vec![SWAP_QUERY_ID]
            }

            fn process_buffered_query(
                &mut self,
                _query_id: u32,
                _query: Vec<usize>,
                _memory: &dyn RamPeek,
            ) -> Box<dyn ExactSizeIterator<Item = usize> + 'static + Send + Sync> {
                unreachable!()
            }
        }

        let mut oracle = ZkEENonDeterminismSource::default();
        oracle.add_external_processor(MemoryProcessor);
        oracle.add_external_processor(IteratorSwap);
    }
}
