#![cfg(test)]

use rig::{basic_system::system_implementation::system::da_commitment_generator::blob_commitment_generator::{ENCODABLE_BYTES_PER_BLOB, blob_versioned_hash_with_advisor, commitment_and_proof_advice::BlobCommitmentAndProofAdvisor}, callable_oracles::blob_kzg_commitment::blob_kzg_commitment_and_proof, utils::{encode_pubdata_for_4844_blobs, get_alloy_4844_blob_versioned_hash}};

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

    // Test with maximum possible data size (accounting for 31-byte length prefix)
    let data = [1; ENCODABLE_BYTES_PER_BLOB - 31];
    let encoded_data = encode_pubdata_for_4844_blobs(&data);
    assert_eq!(encoded_data.len(), ENCODABLE_BYTES_PER_BLOB);

    let versioned_hash = blob_versioned_hash_with_advisor(&encoded_data, &mut advisor);
    let versioned_hash_expected = get_alloy_4844_blob_versioned_hash(&data);

    // Ensure our implementation matches Alloy's reference implementation
    assert_eq!(versioned_hash, versioned_hash_expected)
}

#[test]
fn test_blob_with_data() {
    let mut advisor = BlobCommitmentAndProofAdvisorImplementation;

    // Test with a moderate amount of data (1KB)
    let data = [1; 1024];

    let versioned_hash =
        blob_versioned_hash_with_advisor(&encode_pubdata_for_4844_blobs(&data), &mut advisor);
    let versioned_hash_expected = get_alloy_4844_blob_versioned_hash(&data);

    // Verify consistency with reference implementation
    assert_eq!(versioned_hash, versioned_hash_expected)
}

#[test]
fn test_empty_blob() {
    let mut advisor = BlobCommitmentAndProofAdvisorImplementation;

    // Test edge case: empty data should still produce valid commitment
    let data = [];

    let versioned_hash =
        blob_versioned_hash_with_advisor(&encode_pubdata_for_4844_blobs(&data), &mut advisor);
    let versioned_hash_expected = get_alloy_4844_blob_versioned_hash(&data);

    // Empty blobs should still have consistent hash generation
    assert_eq!(versioned_hash, versioned_hash_expected)
}
