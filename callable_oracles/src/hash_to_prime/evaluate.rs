use super::*;

use crate::utils::evaluate::read_memory_as_u8;
use crate::MemoryRegionDescriptionParams;
use evaluate::compute::compute_from_entropy;
use oracle_provider::OracleQueryProcessor;
use risc_v_simulator::abstractions::memory::MemorySource;
use zk_ee::oracle::usize_serialization::UsizeDeserializable;

pub struct HashToPrimeSource<M: MemorySource> {
    marker: std::marker::PhantomData<M>,
}

impl<M: MemorySource> OracleQueryProcessor<M> for HashToPrimeSource<M> {
    fn supported_query_ids(&self) -> Vec<u32> {
        vec![HASH_TO_PRIME_ORACLE_ID]
    }

    fn process_buffered_query(
        &mut self,
        query_id: u32,
        query: Vec<usize>,
        memory: &M,
    ) -> Box<dyn ExactSizeIterator<Item = usize> + 'static> {
        debug_assert!(self.supports_query_id(query_id));
        let mut it = query.into_iter();
        let memory_region_for_request: MemoryRegionDescriptionParams =
            UsizeDeserializable::from_iter(&mut it).expect("must deserialize");
        let entropy_source = read_memory_as_u8(
            memory,
            memory_region_for_request.offset,
            memory_region_for_request.len,
        )
        .expect("must read memory");
        use crypto::blake2s::Blake2s256;
        use crypto::MiniDigest;
        assert!(MAX_ENTROPY_BYTES <= 64);
        let mut entropy = [0u8; 64];
        for (idx, dst) in entropy.as_chunks_mut::<32>().0.iter_mut().enumerate() {
            let mut hasher = Blake2s256::new();
            hasher.update(&(idx as u32).to_le_bytes());
            hasher.update(&entropy_source);
            dst.copy_from_slice(hasher.finalize().as_slice());
        }

        let _certificate = compute_from_entropy(&entropy);

        // TODO: serialize
        todo!();
    }
}
