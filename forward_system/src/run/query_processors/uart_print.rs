use super::*;
use oracle_provider::OracleQueryProcessor;
use zk_ee::{
    oracle::query_ids::UART_QUERY_ID,
    oracle::usize_serialization::dyn_usize_iterator::DynUsizeIterator,
};

/// This processor handles debug print requests from the RISC-V execution
/// environment. It receives formatted string data and outputs it to stdout,
/// providing a mechanism for debugging and logging from within the ZK
/// execution environment.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct UARTPrintResponder;

impl UARTPrintResponder {
    const SUPPORTED_QUERY_IDS: &[u32] = &[UART_QUERY_ID];
}

impl<M: MemorySource> OracleQueryProcessor<M> for UARTPrintResponder {
    fn supported_query_ids(&self) -> Vec<u32> {
        Self::SUPPORTED_QUERY_IDS.to_vec()
    }

    fn supports_query_id(&self, query_id: u32) -> bool {
        Self::SUPPORTED_QUERY_IDS.contains(&query_id)
    }

    fn process_buffered_query(
        &mut self,
        query_id: u32,
        query: Vec<usize>,
        _memory: &M,
    ) -> Box<dyn ExactSizeIterator<Item = usize> + 'static> {
        assert!(Self::SUPPORTED_QUERY_IDS.contains(&query_id));

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
        print!("{}", String::from_utf8_lossy(&string_bytes));
        // println!("UART: {}", String::from_utf8_lossy(&string_bytes));

        DynUsizeIterator::from_constructor((), UsizeSerializable::iter)
    }
}
