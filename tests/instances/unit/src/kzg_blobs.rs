#![cfg(test)]

use rig::{basic_system::system_implementation::system::da_commitment_generator::blob_commitment_generator::{ENCODABLE_BYTES_PER_BLOB, blob_versioned_hash_with_advisor, commitment_and_proof_advice::BlobCommitmentAndProofAdvisor}, callable_oracles::blob_kzg_commitment::blob_kzg_commitment_and_proof};

struct BlobCommitmentAndProofAdvisorImplementation;

impl BlobCommitmentAndProofAdvisor for BlobCommitmentAndProofAdvisorImplementation {
    fn get_blob_commitment_and_proof_advice(
        &mut self,
        data: &[u8],
    ) -> rig::basic_system::system_implementation::system::da_commitment_generator::KZGCommitmentAndProof{
        blob_kzg_commitment_and_proof(data)
    }
}

#[test]
fn test_blob_with_max_size() {
    let mut advisor = BlobCommitmentAndProofAdvisorImplementation;

    let data = [1; ENCODABLE_BYTES_PER_BLOB];
    let _ = blob_versioned_hash_with_advisor(&data, &mut advisor);
}

#[test]
fn test_empty_blob() {
    let mut advisor = BlobCommitmentAndProofAdvisorImplementation;

    let data = [];
    let _ = blob_versioned_hash_with_advisor(&data, &mut advisor);
}
