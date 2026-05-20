use super::*;
use crate::run::PreimageSource;
use basic_system::system_implementation::ethereum_storage_model::{
    ETHEREUM_BYTECODE_LENGTH_FROM_PREIMAGE_QUERY_ID, ETHEREUM_BYTECODE_PREIMAGE_QUERY_ID,
    ETHEREUM_MPT_PREIMAGE_BYTE_LEN_QUERY_ID, ETHEREUM_MPT_PREIMAGE_WORDS_QUERY_ID,
};
use basic_system::system_implementation::flat_storage_model::FLAT_STORAGE_GENERIC_PREIMAGE_QUERY_ID;
use zk_ee::utils::Bytes32;

/// This processor handles requests to resolve hash preimages - given a hash,
/// it returns the original data that was hashed. This is essential for
/// operations that need to reconstruct the original data from its hash,
/// such as Merkle tree operations and storage proofs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericPreimageResponder<PS: PreimageSource> {
    pub preimage_source: PS,
}

/// Decode WordLayout-encoded u32 words back into a typed value.
fn decode_input<T: WordLayout>(input: &[u32]) -> T {
    let mut cursor = 0;
    T::read_words(&mut || {
        let w = input.get(cursor).copied().unwrap_or(0);
        cursor += 1;
        w
    })
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
    fn supported_query_ids(&self) -> Vec<u32> {
        Self::SUPPORTED_QUERY_IDS.to_vec()
    }

    fn supports_query_id(&self, query_id: u32) -> bool {
        Self::SUPPORTED_QUERY_IDS.contains(&query_id)
    }

    fn process(
        &mut self,
        query_id: u32,
        input: &[u32],
        _memory: &dyn oracle_provider::RamPeek,
    ) -> Result<Vec<u32>, InternalError> {
        assert!(Self::SUPPORTED_QUERY_IDS.contains(&query_id));

        let hash: Bytes32 = decode_input(input);

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
        let mut result = Vec::new();
        if query_id == ETHEREUM_BYTECODE_LENGTH_FROM_PREIMAGE_QUERY_ID
            || query_id == ETHEREUM_MPT_PREIMAGE_BYTE_LEN_QUERY_ID
        {
            (preimage.len() as u32).write_words(&mut |w| result.push(w));
        } else {
            preimage.write_words(&mut |w| result.push(w));
        }
        Ok(result)
    }
}
