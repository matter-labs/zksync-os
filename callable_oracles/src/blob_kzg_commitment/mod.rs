use crate::utils::evaluate::read_memory_as_u8;
use basic_bootloader::bootloader::block_flow::zk::da_commitment_generator::blob_commitment_generator::ENCODABLE_BYTES_PER_BLOB;
use basic_bootloader::bootloader::block_flow::zk::da_commitment_generator::KZGCommitmentAndProof;
use basic_bootloader::bootloader::block_flow::zk::da_commitment_generator::BLOB_COMMITMENT_AND_PROOF_QUERY_ID;
use basic_system::system_functions::point_evaluation::versioned_hash_for_kzg;
use crypto::MiniDigest;
use oracle_provider::OracleQueryProcessor;
use oracle_provider::RamPeek;
use zk_ee::internal_error;
use zk_ee::system::errors::internal::InternalError;

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

    fn process(
        &mut self,
        query_id: u32,
        input: &[u8],
        memory: &dyn RamPeek,
    ) -> Result<Vec<u8>, InternalError> {
        debug_assert!(self.supports_query_id(query_id));

        // this query processor supposed to work only on "host" architecture, which is always 64 bit
        const { assert!(8 == core::mem::size_of::<usize>()) };

        // Decode (data_ptr, data_len) from the input - RISC-V sends these as two u32 values
        let (data_ptr, data_len): (u32, u32) = wincode::deserialize(input)
            .map_err(|_| internal_error!("decode blob ptr/len failed"))?;

        assert!(data_ptr.is_multiple_of(4));

        let data = read_memory_as_u8(memory, data_ptr, data_len).unwrap();
        let result = blob_kzg_commitment_and_proof(&data);

        wincode::serialize(&result).map_err(|_| internal_error!("encode blob commitment failed"))
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

    fn process(
        &mut self,
        query_id: u32,
        input: &[u8],
        _memory: &dyn RamPeek,
    ) -> Result<Vec<u8>, InternalError> {
        debug_assert!(self.supports_query_id(query_id));

        // this query processor supposed to work only on "host" architecture, which is always 64 bit
        const { assert!(8 == core::mem::size_of::<usize>()) };

        // Decode (data_ptr, data_len) from the input - native sends these as two u64 values
        let (data_ptr, data_len): (u64, u64) = wincode::deserialize(input)
            .map_err(|_| internal_error!("decode blob ptr/len failed"))?;

        assert!(data_len <= ENCODABLE_BYTES_PER_BLOB as u64);
        let data = read_u8_words(data_ptr, data_len);
        let result = blob_kzg_commitment_and_proof(&data);

        wincode::serialize(&result).map_err(|_| internal_error!("encode blob commitment failed"))
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

    #[test]
    fn native_blob_query_processes_valid_query() {
        let data = [1u8, 2, 3, 4, 5];
        let input = wincode::serialize(&(data.as_ptr().addr() as u64, data.len() as u64)).unwrap();
        let result_bytes = NativeBlobCommitmentAndProofQuery
            .process(
                BLOB_COMMITMENT_AND_PROOF_QUERY_ID,
                &input,
                &DummyMemorySource,
            )
            .unwrap();
        let result: KZGCommitmentAndProof = wincode::deserialize(&result_bytes).unwrap();

        assert_eq!(result.commitment.len(), 48);
        assert_eq!(result.proof.len(), 48);
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

        let input = wincode::serialize(&(data_addr, data.len() as u32)).unwrap();
        let result_bytes = BlobCommitmentAndProofQuery
            .process(BLOB_COMMITMENT_AND_PROOF_QUERY_ID, &input, &memory)
            .unwrap();
        let result: KZGCommitmentAndProof = wincode::deserialize(&result_bytes).unwrap();

        let expected = blob_kzg_commitment_and_proof(data);
        assert_eq!(result.commitment, expected.commitment);
        assert_eq!(result.proof, expected.proof);
    }

    #[test]
    #[should_panic]
    fn native_blob_query_rejects_null_pointer() {
        let input = wincode::serialize(&(0u64, 1u64)).unwrap();
        let _ = NativeBlobCommitmentAndProofQuery.process(
            BLOB_COMMITMENT_AND_PROOF_QUERY_ID,
            &input,
            &DummyMemorySource,
        );
    }

    #[test]
    #[should_panic]
    fn blob_kzg_oracle_panics_on_misaligned_pointer() {
        let memory = TestMemorySource::default();
        let input = wincode::serialize(&(0x101u32, 10u32)).unwrap();
        let _ = BlobCommitmentAndProofQuery.process(
            BLOB_COMMITMENT_AND_PROOF_QUERY_ID,
            &input,
            &memory,
        );
    }
}
