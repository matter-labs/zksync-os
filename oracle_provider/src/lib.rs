#![allow(clippy::bool_comparison)]
#![allow(clippy::precedence)]
#![allow(clippy::len_zero)]

// Hook zk_ee IOOracle to be NonDeterminismCSRSource

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use zk_ee::kv_markers::UsizeDeserializable;
use zk_ee::system_io_oracle::*;
use zk_ee::types_config::*;

use risc_v_simulator::abstractions::memory::MemorySource;
use risc_v_simulator::abstractions::non_determinism::NonDeterminismCSRSource;

pub struct ZkEENonDeterminismSource<M: MemorySource> {
    query_buffer: Option<QueryBuffer>,
    current_query_id: Option<u32>,
    current_iterator: Option<Box<dyn ExactSizeIterator<Item = usize> + 'static>>,
    iterator_len_to_indicate: Option<u32>,
    high_half: Option<u32>,
    is_connected_to_external_oracle: bool,
    /// Vector of different processors that are responsible for handling queries.
    processors: Vec<Box<dyn OracleQueryProcessor<M> + 'static>>,
    /// Mapping from query_id to processor that is handling it (represented as index in processors vector above).
    ranges: BTreeMap<u32, usize>,
}

impl<M: MemorySource> Default for ZkEENonDeterminismSource<M> {
    fn default() -> Self {
        Self {
            query_buffer: None,
            current_query_id: None,
            current_iterator: None,
            iterator_len_to_indicate: None,
            high_half: None,
            is_connected_to_external_oracle: false,
            processors: Vec::new(),
            ranges: BTreeMap::new(),
        }
    }
}

impl<M: MemorySource> ZkEENonDeterminismSource<M> {
    pub fn add_external_processor<P: OracleQueryProcessor<M> + 'static>(&mut self, processor: P) {
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

    fn process_buffered_query(&mut self, memory: &M) {
        assert!(self.current_iterator.is_none());
        assert!(self.current_query_id.is_none());

        let buffer = self.query_buffer.take().expect("must exist");
        let query_id = buffer.query_type;
        // println!("Processing a query with ID = 0x{:08x}", query_id);
        if query_id == DisconnectOracleFormalIterator::ID {
            self.is_connected_to_external_oracle = false;
        } else {
            let buffer = buffer.buffer;
            let Some(processor_id) = self.ranges.get(&query_id).copied() else {
                panic!("Can not process query with ID = 0x{query_id:08x}");
            };
            let processor = &mut self.processors[processor_id];
            let new_iterator = processor.process_buffered_query(query_id, buffer, memory);

            if let Some(new_iterator) = new_iterator {
                let result_len = new_iterator.len() * 2; // NOTE for mismatch of 32/64-bit archs
                self.iterator_len_to_indicate = Some(result_len as u32);
                if result_len > 0 {
                    self.current_query_id = Some(query_id);
                    self.current_iterator = Some(new_iterator);
                }
            } else {
                self.iterator_len_to_indicate = Some(0);
            }
        }
    }

    /// Reads the next 32bits.
    /// Our iterators and queues hold usize elements (u64), so we have to do some splitting and caching.
    fn read_impl(&mut self) -> u32 {
        // if let Some(query_id) = self.current_query_id {
        //     println!(
        //         "Reading from oracle as a part of query ID = 0x{:08x}",
        //         query_id
        //     );
        // }

        // We mocked reads, so it's filtered out before
        if self.is_connected_to_external_oracle == false {
            return 0;
        }

        if let Some(iterator_len_to_indicate) = self.iterator_len_to_indicate.take() {
            return iterator_len_to_indicate;
        }

        // This is the 32 bit remaining from the previous item - return them now.
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

    fn write_impl(&mut self, memory: &M, value: u32) {
        if self.current_query_id.is_some() {
            println!("Current query ID = 0x{:08x} iterator is not consumed in full, but received value 0x{:08x}", self.current_query_id.unwrap(), value);
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

        if let Some(query_buffer) = self.query_buffer.as_mut() {
            let complete = query_buffer.write(value);
            if complete {
                self.process_buffered_query(memory);
            }
        } else {
            if self.is_connected_to_external_oracle == false && value != UARTAccessMarker::ID {
                // we are not interested in general to start another query
                return;
            }

            let new_buffer = QueryBuffer::empty_for_query_type(value);
            self.query_buffer = Some(new_buffer);
        }
    }
}

pub trait OracleQueryProcessor<M: MemorySource> {
    /// List of different query ids that are supported (for example NextTxSize or BlockLevelMetadataIterator).
    fn supported_query_ids(&self) -> Vec<u32>;
    fn supports_query_id(&self, query_id: u32) -> bool {
        self.supported_query_ids().contains(&query_id)
    }

    fn process_buffered_query(
        &mut self,
        query_id: u32,
        query: Vec<usize>,
        memory: &M,
    ) -> Option<Box<dyn ExactSizeIterator<Item = usize> + 'static>>;
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

#[derive(Clone)]
#[allow(dead_code)]
pub struct BasicZkEEOracleWrapper<IOTypes: SystemIOTypesConfig, O: IOOracle>
where
    for<'a> O::MarkerTiedIterator<'a>: 'static,
{
    oracle: O,
    is_connected_to_external_oracle: bool,
    partial_uart_string: Vec<u8>,
    _marker: std::marker::PhantomData<IOTypes>,
}

impl<IOTypes: SystemIOTypesConfig, O: IOOracle> BasicZkEEOracleWrapper<IOTypes, O>
where
    for<'a> O::MarkerTiedIterator<'a>: 'static,
{
    pub fn new(oracle: O) -> Self {
        Self {
            oracle,
            is_connected_to_external_oracle: true,
            partial_uart_string: vec![],
            _marker: std::marker::PhantomData,
        }
    }
}

impl<IOTypes: SystemIOTypesConfig, O: IOOracle, M: MemorySource> OracleQueryProcessor<M>
    for BasicZkEEOracleWrapper<IOTypes, O>
where
    for<'a> O::MarkerTiedIterator<'a>: 'static,
{
    fn supported_query_ids(&self) -> Vec<u32> {
        let supported = &[
            NextTxSize::ID,
            NewTxContentIterator::ID,
            ProofDataIterator::ID,
            BlockLevelMetadataIterator::ID,
            InitialStorageSlotDataIterator::<EthereumIOTypesConfig>::ID,
            PreimageContentWordsIterator::ID,
            DisconnectOracleFormalIterator::ID,
            ProofForIndexIterator::ID,
            PrevIndexIterator::ID,
            ExactIndexIterator::ID,
            InteropRootsIterator::ID,
            UARTAccessMarker::ID,
        ];

        debug_assert!(supported.is_sorted());

        supported.to_vec()
    }

    fn process_buffered_query(
        &mut self,
        query_id: u32,
        query: Vec<usize>,
        _memory: &M,
    ) -> Option<Box<dyn ExactSizeIterator<Item = usize> + 'static>> {
        let new_iterator: O::MarkerTiedIterator<'_> = match query_id {
            NextTxSize::ID => {
                let mut src_it = query.into_iter();
                let params : () = <<NextTxSize as OracleIteratorTypeMarker>::Params as UsizeDeserializable>::from_iter(&mut src_it).expect("must deserialize query params");
                assert!(src_it.len() == 0);
                let it = self
                    .oracle
                    .create_oracle_access_iterator::<NextTxSize>(params)
                    .expect("must make an iterator");

                it
            }
            NewTxContentIterator::ID => {
                let mut src_it = query.into_iter();
                let params : () = <<NewTxContentIterator as OracleIteratorTypeMarker>::Params as UsizeDeserializable>::from_iter(&mut src_it).expect("must deserialize query params");
                // assert!(src_it.len() == 0);
                let it = self
                    .oracle
                    .create_oracle_access_iterator::<NewTxContentIterator>(params)
                    .expect("must make an iterator");
                it
            }
            ProofDataIterator::ID => {
                let mut src_it = query.into_iter();
                let params : () = <<ProofDataIterator as OracleIteratorTypeMarker>::Params as UsizeDeserializable>::from_iter(&mut src_it).expect("must deserialize query params");
                assert!(src_it.len() == 0);
                let it = self
                    .oracle
                    .create_oracle_access_iterator::<ProofDataIterator>(params)
                    .expect("must make an iterator");
                it
            }
            InitialStorageSlotDataIterator::<EthereumIOTypesConfig>::ID => {
                let mut src_it = query.into_iter();
                let params = <<InitialStorageSlotDataIterator::<EthereumIOTypesConfig> as OracleIteratorTypeMarker>::Params as UsizeDeserializable>::from_iter(&mut src_it).expect("must deserialize query params");
                assert!(src_it.len() == 0);
                let it = self
                    .oracle
                    .create_oracle_access_iterator::<InitialStorageSlotDataIterator<EthereumIOTypesConfig>>(params)
                    .expect("must make an iterator");
                it
            }
            PreimageContentWordsIterator::ID => {
                let mut src_it = query.into_iter();
                let params = <<PreimageContentWordsIterator as OracleIteratorTypeMarker>::Params as UsizeDeserializable>::from_iter(&mut src_it).expect("must deserialize query params");
                assert!(src_it.len() == 0);
                let it = self
                    .oracle
                    .create_oracle_access_iterator::<PreimageContentWordsIterator>(params)
                    .expect("must make an iterator");
                it
            }
            DisconnectOracleFormalIterator::ID => {
                self.is_connected_to_external_oracle = false;
                return None;
            }
            ProofForIndexIterator::ID => {
                let mut src_it = query.into_iter();
                let params = <<ProofForIndexIterator as OracleIteratorTypeMarker>::Params as UsizeDeserializable>::from_iter(&mut src_it).expect("must deserialize query params");
                assert!(src_it.len() == 0);
                // there is nothing to do here
                let it = self
                    .oracle
                    .create_oracle_access_iterator::<ProofForIndexIterator>(params)
                    .expect("must make an iterator");
                it
            }
            PrevIndexIterator::ID => {
                let mut src_it = query.into_iter();
                let params = <<PrevIndexIterator as OracleIteratorTypeMarker>::Params as UsizeDeserializable>::from_iter(&mut src_it).expect("must deserialize query params");
                assert!(src_it.len() == 0);
                // there is nothing to do here
                let it = self
                    .oracle
                    .create_oracle_access_iterator::<PrevIndexIterator>(params)
                    .expect("must make an iterator");
                it
            }
            ExactIndexIterator::ID => {
                let mut src_it = query.into_iter();
                let params = <<ExactIndexIterator as OracleIteratorTypeMarker>::Params as UsizeDeserializable>::from_iter(&mut src_it).expect("must deserialize query params");
                assert!(src_it.len() == 0);
                // there is nothing to do here
                let it = self
                    .oracle
                    .create_oracle_access_iterator::<ExactIndexIterator>(params)
                    .expect("must make an iterator");
                it
            }
            BlockLevelMetadataIterator::ID => {
                let mut src_it = query.into_iter();
                let params  : ()= <<BlockLevelMetadataIterator as OracleIteratorTypeMarker>::Params as UsizeDeserializable>::from_iter(&mut src_it).expect("must deserialize query params");
                assert!(src_it.len() == 0);
                // there is nothing to do here
                let it = self
                    .oracle
                    .create_oracle_access_iterator::<BlockLevelMetadataIterator>(params)
                    .expect("must make an iterator");
                it
            }
            InteropRootsIterator::ID => {
                let mut src_it = query.into_iter();
                let params  : ()= <<InteropRootsIterator as OracleIteratorTypeMarker>::Params as UsizeDeserializable>::from_iter(&mut src_it).expect("must deserialize query params");
                assert!(src_it.len() == 0);
                // there is nothing to do here
                let it = self
                    .oracle
                    .create_oracle_access_iterator::<InteropRootsIterator>(params)
                    .expect("must make an iterator");
                it
            }
            UARTAccessMarker::ID => {
                // just our old plain uart
                let u32_vec: Vec<u32> = query
                    .into_iter()
                    .flat_map(|el| [el as u32, (el >> 32) as u32])
                    .collect();
                assert!(!u32_vec.is_empty());
                let message_len_in_bytes = u32_vec[0] as usize;
                let mut string_bytes: Vec<u8> = u32_vec[1..]
                    .iter()
                    .flat_map(|el| el.to_le_bytes())
                    .collect();
                assert!(string_bytes.len() >= message_len_in_bytes);
                string_bytes.truncate(message_len_in_bytes);
                println!("UART: {}", String::from_utf8_lossy(&string_bytes));

                // // TODO: understand what is broken in format machinery and my we get non-utf8 sometimes

                // let recovered_string = if self.partial_uart_string.len() > 0 {
                //     let mut existing = std::mem::replace(&mut self.partial_uart_string, vec![]);
                //     existing.extend(string_bytes);

                //     existing
                // } else {
                //     string_bytes
                // };
                // // dbg!(String::from_utf8_lossy(&recovered_string));
                // if let Ok(string) = String::from_utf8(recovered_string.clone()) {
                //     println!("UART: {}", string);
                // } else {
                //     // we could receive a partial one
                //     self.partial_uart_string = recovered_string;
                // }

                return None;
            }
            a => {
                panic!("Can not proceed query with ID = 0x{a:08x}");
            }
        };

        if new_iterator.len() > 0 {
            Some(Box::new(new_iterator))
        } else {
            None
        }
    }
}

// now we hook an access
impl<M: MemorySource> NonDeterminismCSRSource<M> for ZkEENonDeterminismSource<M> {
    #[allow(clippy::let_and_return)]
    fn read(&mut self) -> u32 {
        let value = self.read_impl();
        // println!("`NonDeterminismCSRSource` returned 0x{:08x}", value);
        value
    }

    fn write_with_memory_access(&mut self, memory: &M, value: u32) {
        // println!("`NonDeterminismCSRSource` received 0x{:08x}", value);
        self.write_impl(memory, value);
    }
}

/// Wraps the original source and remembers all the read accesses.
pub struct ReadWitnessSource<M: MemorySource> {
    original_source: ZkEENonDeterminismSource<M>,
    read_items: Rc<RefCell<Vec<u32>>>,
}

impl<M: MemorySource> ReadWitnessSource<M> {
    pub fn new(original_source: ZkEENonDeterminismSource<M>) -> Self {
        Self {
            original_source,
            read_items: Rc::new(RefCell::new(vec![])),
        }
    }

    pub fn get_read_items(&self) -> Rc<RefCell<Vec<u32>>> {
        self.read_items.clone()
    }
}

impl<M: MemorySource> NonDeterminismCSRSource<M> for ReadWitnessSource<M> {
    fn read(&mut self) -> u32 {
        let item = self.original_source.read();
        // on read - remember the items.
        self.read_items.borrow_mut().push(item);
        item
    }

    fn write_with_memory_access(&mut self, memory: &M, value: u32) {
        self.original_source.write_with_memory_access(memory, value);
    }
}
