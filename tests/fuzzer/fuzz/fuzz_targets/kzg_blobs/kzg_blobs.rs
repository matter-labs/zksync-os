#![no_main]

use basic_system::system_implementation::system::da_commitment_generator::blob_commitment_generator::{ENCODABLE_BYTES_PER_BLOB, blob_versioned_hash_with_advisor, commitment_and_proof_advice::BlobCommitmentAndProofAdvisor};
use callable_oracles::blob_kzg_commitment::blob_kzg_commitment_and_proof;
use libfuzzer_sys::fuzz_target;
use arbitrary::Arbitrary;
// Utility functions for testing blob commitments against Alloy reference implementation
use rig::utils::get_alloy_4844_blob_versioned_hash;
use rig::utils::encode_pubdata_for_4844_blobs;

struct BlobCommitmentAndProofAdvisorImplementation;

impl BlobCommitmentAndProofAdvisor for BlobCommitmentAndProofAdvisorImplementation {
    fn get_blob_commitment_and_proof_advice(
        &mut self,
        data: &[u8],
    ) -> rig::basic_system::system_implementation::system::da_commitment_generator::KZGCommitmentAndProof{
        blob_kzg_commitment_and_proof(data)
    }
}

fuzz_target!(|data: &[u8]| {
    // Limit input size to account for 31-byte length prefix in encoding
    // This ensures the final encoded data fits within ENCODABLE_BYTES_PER_BLOB
    if data.len() > ENCODABLE_BYTES_PER_BLOB - 31 {
        return;
    }

    // Encode the raw data with length prefix for 4844 blob format
    let encoded_data = encode_pubdata_for_4844_blobs(&data);

    let mut advisor = BlobCommitmentAndProofAdvisorImplementation;

    // Generate versioned hash using our implementation
    let versioned_hash = blob_versioned_hash_with_advisor(&encoded_data, &mut advisor);
    // Generate expected versioned hash using Alloy's reference implementation
    let versioned_hash_expected = get_alloy_4844_blob_versioned_hash(&data);

    // Verify our implementation matches the reference
    assert_eq!(versioned_hash, versioned_hash_expected);
});
