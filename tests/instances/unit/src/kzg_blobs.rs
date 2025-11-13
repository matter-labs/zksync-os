#![cfg(test)]

use rig::{alloy::consensus::{SidecarBuilder, SimpleCoder}, basic_system::system_implementation::system::da_commitment_generator::blob_commitment_generator::{ENCODABLE_BYTES_PER_BLOB, blob_versioned_hash_with_advisor, commitment_and_proof_advice::BlobCommitmentAndProofAdvisor}, callable_oracles::blob_kzg_commitment::blob_kzg_commitment_and_proof};

struct BlobCommitmentAndProofAdvisorImplementation;

impl BlobCommitmentAndProofAdvisor for BlobCommitmentAndProofAdvisorImplementation {
    fn get_blob_commitment_and_proof_advice(
        &mut self,
        data: &[u8],
    ) -> rig::basic_system::system_implementation::system::da_commitment_generator::KZGCommitmentAndProof{
        blob_kzg_commitment_and_proof(data)
    }
}

fn get_alloy_versioned_hash(data: &[u8]) -> [u8; 32] {
    let blob_sidecar = SidecarBuilder::<SimpleCoder>::from_slice(&data)
        .build()
        .unwrap();

    let mut alloy_hashes_iter = blob_sidecar.versioned_hashes();
    let versioned_hash_alloy = alloy_hashes_iter.next().expect("Should exist");
    //assert!(alloy_hashes_iter.next().is_none());

    versioned_hash_alloy.0
}

fn encode_pubdata(data: &[u8]) -> Vec<u8> {
    // we allocate 31 byte to encode length as a separate field element for convenience
    let mut vec = Vec::from([0u8; 31]);
    vec.extend_from_slice(&data);
    let length = vec.len() - 31;
    vec[0..8].copy_from_slice(&(length as u64).to_be_bytes());

    vec
}

#[test]
fn test_blob_with_max_size() {
    let mut advisor = BlobCommitmentAndProofAdvisorImplementation;

    let data = [1; ENCODABLE_BYTES_PER_BLOB];
    let versioned_hash = blob_versioned_hash_with_advisor(&data, &mut advisor);

    let versioned_hash_expected = get_alloy_versioned_hash(&data);

    assert_eq!(versioned_hash, versioned_hash_expected)
}

#[test]
fn test_blob_with_data() {
    let mut advisor = BlobCommitmentAndProofAdvisorImplementation;

    let data = encode_pubdata(&[1; 1024]);

    let versioned_hash = blob_versioned_hash_with_advisor(&data, &mut advisor);
    let versioned_hash_expected = get_alloy_versioned_hash(&data);

    assert_eq!(versioned_hash, versioned_hash_expected)
}

#[test]
fn test_empty_blob() {
    let mut advisor = BlobCommitmentAndProofAdvisorImplementation;

    let data = [];
    let versioned_hash = blob_versioned_hash_with_advisor(&data, &mut advisor);

    let versioned_hash_expected = get_alloy_versioned_hash(&data);

    assert_eq!(versioned_hash, versioned_hash_expected)
}
