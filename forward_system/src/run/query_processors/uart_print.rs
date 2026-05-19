use super::*;
use oracle_provider::OracleQueryProcessor;
use zk_ee::oracle::query_ids::UART_QUERY_ID;

/// This processor handles debug print requests from the RISC-V execution
/// environment. It receives formatted string data and outputs it to stdout,
/// providing a mechanism for debugging and logging from within the ZK
/// execution environment.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct UARTPrintResponder;

impl UARTPrintResponder {
    const SUPPORTED_QUERY_IDS: &[u32] = &[UART_QUERY_ID];
}

impl OracleQueryProcessor for UARTPrintResponder {
    fn supported_query_ids(&self) -> Vec<u32> {
        Self::SUPPORTED_QUERY_IDS.to_vec()
    }

    fn supports_query_id(&self, query_id: u32) -> bool {
        Self::SUPPORTED_QUERY_IDS.contains(&query_id)
    }

    fn process(
        &mut self,
        query_id: u32,
        input: &[u8],
        _memory: &dyn oracle_provider::RamPeek,
    ) -> Result<Vec<u8>, InternalError> {
        assert!(Self::SUPPORTED_QUERY_IDS.contains(&query_id));

        // The input is the UART message encoded via wincode.
        // The guest sends the message as bytes (Vec<u8>).
        let string_bytes: Vec<u8> = wincode::deserialize(input)
            .map_err(|_| internal_error!("decode UART message failed"))?;
        print!("{}", String::from_utf8_lossy(&string_bytes));

        // Return empty response
        wincode::serialize(&()).map_err(|_| internal_error!("encode UART response failed"))
    }
}
