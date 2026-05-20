use basic_bootloader::bootloader::block_flow::zk::da_commitment_generator::blob_commitment_generator::ENCODABLE_BYTES_PER_BLOB;
use basic_bootloader::bootloader::block_flow::zk::da_commitment_generator::KZGCommitmentAndProof;
use basic_bootloader::bootloader::block_flow::zk::da_commitment_generator::BLOB_COMMITMENT_AND_PROOF_QUERY_ID;
use basic_system::system_functions::point_evaluation::versioned_hash_for_kzg;
use crypto::MiniDigest;
use oracle_provider::OracleQueryProcessor;
use oracle_provider::RamPeek;
use zk_ee::oracle::word_layout::WordLayout;
use zk_ee::system::errors::internal::InternalError;

///
/// Query processor, which returns blob kzg commitment and proof for a given data.
///
/// The input is a [u32; 2] WordLayout-encoded as [ptr, len], where ptr is a host
/// process memory pointer and len is the byte count.
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
        input: &[u32],
        _memory: &dyn RamPeek,
    ) -> Result<Vec<u32>, InternalError> {
        debug_assert!(self.supports_query_id(query_id));

        // Input is (u64, u64) WordLayout-encoded: (ptr, len) = 4 u32 words
        let (data_ptr, data_len): (u64, u64) = {
            let mut cursor = 0;
            <(u64, u64)>::read_words(&mut || {
                let w = input[cursor];
                cursor += 1;
                w
            })
        };
        let data_len = data_len as usize;
        assert!(data_len <= ENCODABLE_BYTES_PER_BLOB);

        // Read from host process memory
        assert!(data_ptr != 0);
        let data = unsafe { core::slice::from_raw_parts(data_ptr as *const u8, data_len) };
        let result = blob_kzg_commitment_and_proof(data);

        let mut output = Vec::new();
        result.write_words(&mut |w| output.push(w));
        Ok(output)
    }
}

pub use BlobCommitmentAndProofQuery as NativeBlobCommitmentAndProofQuery;

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
    use oracle_provider::DummyMemorySource;

    #[test]
    fn native_blob_query_processes_valid_query() {
        let data = [1u8, 2, 3, 4, 5];
        let input = (data.as_ptr() as u64, data.len() as u64);
        let mut input_words = Vec::new();
        input.write_words(&mut |w| input_words.push(w));
        let output = BlobCommitmentAndProofQuery
            .process(
                BLOB_COMMITMENT_AND_PROOF_QUERY_ID,
                &input_words,
                &DummyMemorySource,
            )
            .unwrap();

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
    #[should_panic]
    fn native_blob_query_rejects_null_pointer() {
        let _ = BlobCommitmentAndProofQuery.process(
            BLOB_COMMITMENT_AND_PROOF_QUERY_ID,
            &[0, 1],
            &DummyMemorySource,
        );
    }
}
