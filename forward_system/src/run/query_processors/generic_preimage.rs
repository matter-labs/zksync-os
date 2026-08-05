use super::*;
use crate::run::PreimageSource;
use basic_system::system_implementation::flat_storage_model::FLAT_STORAGE_GENERIC_PREIMAGE_QUERY_ID;
use zk_ee::oracle::usize_serialization::dyn_usize_iterator::DynUsizeIterator;
use zk_ee::utils::usize_rw::ReadIterWrapper;
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
    const SUPPORTED_QUERY_IDS: &[u32] = &[FLAT_STORAGE_GENERIC_PREIMAGE_QUERY_ID];
}

impl<PS: PreimageSource, M: MemorySource> OracleQueryProcessor<M> for GenericPreimageResponder<PS> {
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

        let hash = Bytes32::from_iter(&mut query.into_iter()).expect("must deserialize hash value");

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

        DynUsizeIterator::from_constructor(preimage, |inner_ref| {
            ReadIterWrapper::from(inner_ref.iter().copied())
        })
    }
}
