#![allow(clippy::bool_comparison)]
#![allow(clippy::precedence)]
#![allow(clippy::len_zero)]

pub mod legacy_adapter;
pub mod witness_recording;

#[cfg(all(
    not(target_arch = "riscv32"),
    not(all(target_pointer_width = "64", target_endian = "little"))
))]
compile_error!("oracle_provider requires a 64-bit little-endian host target");

use std::collections::BTreeMap;

use airbender_codec::{AirbenderCodec, AirbenderCodecV0};
use serde::de::DeserializeOwned;
use serde::Serialize;
use zk_ee::oracle::query_ids::DISCONNECT_ORACLE_QUERY_ID;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::{internal_error, oracle::IOOracle};

pub use riscv_transpiler::vm::RamPeek;

pub struct DummyMemorySource;

impl RamPeek for DummyMemorySource {
    fn peek_word(&self, _address: u32) -> u32 {
        unreachable!("DummyMemorySource should not be read from")
    }
}

///
/// Structure that dispatches queries to various responders.
/// When constructed it checks that responders do not try to serve the same query ID.
#[derive(Default)]
pub struct ZkEENonDeterminismSource {
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
}

impl IOOracle for ZkEENonDeterminismSource {
    fn query<I: Serialize, O: DeserializeOwned + Serialize>(
        &mut self,
        query_type: u32,
        input: &I,
    ) -> Result<O, InternalError> {
        if self.is_connected_to_external_oracle == false {
            return Err(internal_error!("oracle disconnected"));
        }
        if query_type == DISCONNECT_ORACLE_QUERY_ID {
            self.is_connected_to_external_oracle = false;
            // Encode a dummy response of the expected output type.
            // DisconnectOracleQuery expects `()` as output.
            let encoded = AirbenderCodecV0::encode(&())
                .map_err(|_| internal_error!("encode disconnect response failed"))?;
            return AirbenderCodecV0::decode(&encoded)
                .map_err(|_| internal_error!("decode disconnect response failed"));
        }
        let input_bytes =
            AirbenderCodecV0::encode(input).map_err(|_| internal_error!("encode input failed"))?;
        let Some(processor_id) = self.ranges.get(&query_type).copied() else {
            return Err(internal_error!(
                "Can not process query with ID = 0x{query_type:08x}"
            ));
        };
        let processor = &mut self.processors[processor_id];
        let response_bytes = processor.process(query_type, &input_bytes, &DummyMemorySource)?;
        AirbenderCodecV0::decode(&response_bytes)
            .map_err(|_| internal_error!("decode response failed"))
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
        input: &[u8],
        memory: &dyn RamPeek,
    ) -> Result<Vec<u8>, InternalError>;
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
            input: &[u8],
            _memory: &dyn RamPeek,
        ) -> Result<Vec<u8>, InternalError> {
            assert_eq!(query_id, TEST_QUERY_ID);
            let decoded: u64 =
                AirbenderCodecV0::decode(input).map_err(|_| internal_error!("decode failed"))?;
            assert_eq!(decoded, 7u64);
            let response: Vec<u32> = vec![0x55667788, 0x11223344, 0xDDEEFF00, 0x99AABBCC];
            AirbenderCodecV0::encode(&response).map_err(|_| internal_error!("encode failed"))
        }
    }

    #[test]
    fn serde_oracle_roundtrip() {
        let mut oracle = ZkEENonDeterminismSource::default();
        oracle.add_external_processor(FixedResponseProcessor);

        let response: Vec<u32> = oracle.query(TEST_QUERY_ID, &7u64).unwrap();
        assert_eq!(
            response,
            vec![0x55667788, 0x11223344, 0xDDEEFF00, 0x99AABBCC]
        );
    }

    #[test]
    fn oracle_disconnects_on_disconnect_query() {
        let mut oracle = ZkEENonDeterminismSource::default();
        oracle.add_external_processor(FixedResponseProcessor);

        // The disconnect query itself succeeds
        let result: Result<(), _> = oracle.query(DISCONNECT_ORACLE_QUERY_ID, &());
        assert!(result.is_ok());

        // After disconnect, further queries should fail
        let result: Result<Vec<u32>, _> = oracle.query(TEST_QUERY_ID, &7u64);
        assert!(result.is_err());
    }
}
