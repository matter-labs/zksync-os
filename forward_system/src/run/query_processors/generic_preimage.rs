use super::*;
use crate::run::PreimageSource;
use basic_system::system_implementation::ethereum_storage_model::{
    ETHEREUM_BYTECODE_LENGTH_FROM_PREIMAGE_QUERY_ID, ETHEREUM_BYTECODE_PREIMAGE_QUERY_ID,
    ETHEREUM_MPT_PREIMAGE_BYTE_LEN_QUERY_ID, ETHEREUM_MPT_PREIMAGE_WORDS_QUERY_ID,
};
use basic_system::system_implementation::flat_storage_model::FLAT_STORAGE_GENERIC_PREIMAGE_QUERY_ID;
use zk_ee::oracle::memory_io::host::{
    write_dynamic_bytes, QuerierMemory, ReadQueryInput, WriteQueryOutput,
};
use zk_ee::utils::Bytes32;

/// This processor handles requests to resolve hash preimages - given a hash,
/// it returns the original data that was hashed. This is essential for
/// operations that need to reconstruct the original data from its hash,
/// such as Merkle tree operations and storage proofs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericPreimageResponder<PS: PreimageSource> {
    pub preimage_source: PS,
}

impl<PS: PreimageSource> GenericPreimageResponder<PS> {
    const SUPPORTED_QUERY_IDS: &[u32] = &[
        FLAT_STORAGE_GENERIC_PREIMAGE_QUERY_ID,
        ETHEREUM_BYTECODE_LENGTH_FROM_PREIMAGE_QUERY_ID,
        ETHEREUM_BYTECODE_PREIMAGE_QUERY_ID,
        ETHEREUM_MPT_PREIMAGE_BYTE_LEN_QUERY_ID,
        ETHEREUM_MPT_PREIMAGE_WORDS_QUERY_ID,
    ];
}

impl<PS: PreimageSource> OracleQueryProcessor for GenericPreimageResponder<PS> {
    fn supported_memory_query_ids(&self) -> Vec<u32> {
        Self::SUPPORTED_QUERY_IDS.to_vec()
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
        assert!(Self::SUPPORTED_QUERY_IDS.contains(&query_id));

        let hash = Bytes32::read_input(memory, input_word).expect("must read the hash");

        let preimage = if hash.is_zero() {
            vec![]
        } else {
            self.preimage_source.get_preimage(hash).unwrap_or_else(|| {
                panic!(
                    "must know a preimage for hash {} for query ID 0x{:016x}",
                    hex::encode(hash.as_u8_array_ref()),
                    query_id
                )
            })
        };
        let mut response = Vec::new();
        if query_id == ETHEREUM_BYTECODE_LENGTH_FROM_PREIMAGE_QUERY_ID
            || query_id == ETHEREUM_MPT_PREIMAGE_BYTE_LEN_QUERY_ID
        {
            u32::try_from(preimage.len())
                .expect("preimage length must fit into u32")
                .write_output(&mut response);
        } else {
            write_dynamic_bytes(&preimage, &mut response);
        }
        respond_to_every_target(mode, response, native_run_responses, guest_run_responses);
    }
}
