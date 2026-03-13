use crate::utils::evaluate::read_memory_as_u8;
use crate::utils::usize_slice_iterator::UsizeSliceIteratorOwned;
use basic_bootloader::bootloader::block_flow::zk::da_commitment_generator::blob_commitment_generator::ENCODABLE_BYTES_PER_BLOB;
use basic_bootloader::bootloader::block_flow::zk::da_commitment_generator::KZGCommitmentAndProof;
use basic_bootloader::bootloader::block_flow::zk::da_commitment_generator::BLOB_COMMITMENT_AND_PROOF_QUERY_ID;
use basic_system::system_functions::point_evaluation::versioned_hash_for_kzg;
use crypto::MiniDigest;
use oracle_provider::OracleQueryProcessor;
use oracle_provider::RamPeek;
use zk_ee::oracle::word_serialization::WordSerializable;

use crate::read_u8_words;

///
/// Query processor, which returns blob kzg commitment and proof for a given data.
///
/// Proof is basically kzg proof in a point derived from data and kzg commitment,
/// so it allows to verify kzg commitment correctness by validating this proof and value of the polynomial in this point.
///
#[derive(Default)]
pub struct BlobCommitmentAndProofQuery;

impl OracleQueryProcessor for BlobCommitmentAndProofQuery {
    fn supported_query_ids(&self) -> Vec<u32> {
        vec![BLOB_COMMITMENT_AND_PROOF_QUERY_ID]
    }

    fn process_buffered_query(
        &mut self,
        query_id: u32,
        query: Vec<usize>,
        memory: &dyn RamPeek,
    ) -> Box<dyn ExactSizeIterator<Item = usize> + 'static + Send + Sync> {
        debug_assert!(self.supports_query_id(query_id));

        // this query processor supposed to work only on "host" architecture, which is always 64 bit
        const { assert!(8 == core::mem::size_of::<usize>()) };
        let mut it = query.into_iter();

        // Even though on riscv32 pointer and length are 32 bits, they are encoded as u64 and take a whole 64-bit word here
        let data_ptr = it.next().unwrap() as u32;
        let data_len = it.next().unwrap() as u32;
        assert!(
            it.next().is_none(),
            "RISC-V ptr and len should've been passed."
        );

        assert!(data_ptr.is_multiple_of(4));

        let data = read_memory_as_u8(memory, data_ptr, data_len).unwrap();
        let result = blob_kzg_commitment_and_proof(&data);

        let r = result.to_word_vec();
        let r = Vec::into_boxed_slice(r);
        let n = UsizeSliceIteratorOwned::new(r);
        Box::new(n)
    }
}

/// Query processor to be used for prover input native run
/// Works in a similar way as the NativeBlobCommitmentAndProof, but with
/// 64 bit pointers. Importantly, the query response is the
/// same.
///
/// This processor explicitly reads the process memory
/// using a raw pointer to get the input.
#[derive(Default)]
pub struct NativeBlobCommitmentAndProofQuery;

impl OracleQueryProcessor for NativeBlobCommitmentAndProofQuery {
    fn supported_query_ids(&self) -> Vec<u32> {
        vec![BLOB_COMMITMENT_AND_PROOF_QUERY_ID]
    }

    fn process_buffered_query(
        &mut self,
        query_id: u32,
        query: Vec<usize>,
        _memory: &dyn RamPeek,
    ) -> Box<dyn ExactSizeIterator<Item = usize> + 'static + Send + Sync> {
        debug_assert!(self.supports_query_id(query_id));

        // this query processor supposed to work only on "host" architecture, which is always 64 bit
        const { assert!(8 == core::mem::size_of::<usize>()) };
        let mut it = query.into_iter();
        let data_ptr = it.next().expect("A u64 should've been passed in.");
        let data_len = it.next().expect("A u64 should've been passed in.");
        assert!(
            it.next().is_none(),
            "Only a pointer and the length are expected."
        );
        assert!(data_len <= ENCODABLE_BYTES_PER_BLOB);
        let data = read_u8_words(data_ptr as u64, data_len as u64);
        let result = blob_kzg_commitment_and_proof(&data);

        let r = result.to_word_vec();
        let r = Vec::into_boxed_slice(r);
        let n = UsizeSliceIteratorOwned::new(r);
        Box::new(n)
    }
}

///
/// Calculate kzg commitment and proof at the point `blake2s(versioned_hash & data)` for blob created from passed data.
///
/// For encoding, we chunk `data` by 31 bytes and interpret each chunk as BE blob element.
///
pub fn blob_kzg_commitment_and_proof(data: &[u8]) -> KZGCommitmentAndProof {
    let mut blob = [0u8; 4096 * 32];
    for (i, chunk) in data.chunks(31).enumerate() {
        let fe = &mut blob[i * 32..(i + 1) * 32];
        fe[1..1 + chunk.len()].copy_from_slice(chunk);
    }
    let blob = c_kzg::Blob::new(blob);

    let kzg_settings = c_kzg::ethereum_kzg_settings(8);

    let commitment = kzg_settings.blob_to_kzg_commitment(&blob).unwrap();

    let mut hasher = crypto::blake2s::Blake2s256::new();
    hasher.update(versioned_hash_for_kzg(commitment.as_slice()).as_slice());
    hasher.update(data);
    let mut challenge_point = hasher.finalize();
    // truncate hash to 128 bits
    // NOTE: it is safe to draw a random scalar at max 128 bits because of the schwartz zippel lemma
    for byte in challenge_point[0..16].iter_mut() {
        *byte = 0;
    }
    let p = kzg_settings
        .compute_kzg_proof(&blob, &c_kzg::Bytes32::new(challenge_point))
        .unwrap();
    let proof = p.0;

    KZGCommitmentAndProof {
        commitment: commitment.to_bytes().into_inner(),
        proof: proof.to_bytes().into_inner(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::TestMemorySource;
    use oracle_provider::DummyMemorySource;
    use zk_ee::oracle::usize_serialization::UsizeSerializable;

    #[test]
    fn native_blob_query_processes_valid_query() {
        let data = [1u8, 2, 3, 4, 5];
        let output: Vec<usize> = NativeBlobCommitmentAndProofQuery
            .process_buffered_query(
                BLOB_COMMITMENT_AND_PROOF_QUERY_ID,
                vec![data.as_ptr().addr(), data.len()],
                &DummyMemorySource,
            )
            .collect();

        assert!(!output.is_empty());
    }

    #[test]
    fn blob_kzg_commitment_deterministic() {
        let data = b"hello blob commitment test data";
        let result1 = blob_kzg_commitment_and_proof(data);
        let result2 = blob_kzg_commitment_and_proof(data);
        assert_eq!(result1.commitment, result2.commitment);
        assert_eq!(result1.proof, result2.proof);
    }

    #[test]
    fn blob_kzg_commitment_different_data() {
        let result1 = blob_kzg_commitment_and_proof(b"data1");
        let result2 = blob_kzg_commitment_and_proof(b"data2");
        assert_ne!(
            result1.commitment, result2.commitment,
            "different data should produce different commitments"
        );
    }

    #[test]
    fn blob_kzg_commitment_empty_data() {
        let result = blob_kzg_commitment_and_proof(b"");
        assert_eq!(result.commitment.len(), 48);
        assert_eq!(result.proof.len(), 48);
    }

    #[test]
    fn riscv_blob_kzg_oracle_via_memory() {
        let data = b"test blob data for oracle query";
        let data_addr: u32 = 0x100;
        let mut memory = TestMemorySource::default();
        memory.write_bytes(data_addr, data);

        let result: Vec<usize> = BlobCommitmentAndProofQuery
            .process_buffered_query(
                BLOB_COMMITMENT_AND_PROOF_QUERY_ID,
                vec![data_addr as usize, data.len() as usize],
                &memory,
            )
            .collect();

        assert!(!result.is_empty(), "Oracle should return non-empty result");

        let expected = blob_kzg_commitment_and_proof(data);
        let expected_serialized: Vec<usize> = expected.iter().collect();
        assert_eq!(result, expected_serialized);
    }

    #[test]
    #[should_panic]
    fn native_blob_query_rejects_null_pointer() {
        let _ = NativeBlobCommitmentAndProofQuery.process_buffered_query(
            BLOB_COMMITMENT_AND_PROOF_QUERY_ID,
            vec![0, 1],
            &DummyMemorySource,
        );
    }

    #[test]
    #[should_panic(expected = "RISC-V ptr and len should've been passed")]
    fn blob_kzg_oracle_panics_on_extra_args() {
        let memory = TestMemorySource::default();
        let _ = BlobCommitmentAndProofQuery.process_buffered_query(
            BLOB_COMMITMENT_AND_PROOF_QUERY_ID,
            vec![0x100, 10, 42],
            &memory,
        );
    }

    #[test]
    #[should_panic]
    fn blob_kzg_oracle_panics_on_misaligned_pointer() {
        let memory = TestMemorySource::default();
        let _ = BlobCommitmentAndProofQuery.process_buffered_query(
            BLOB_COMMITMENT_AND_PROOF_QUERY_ID,
            vec![0x101, 10],
            &memory,
        );
    }
}
