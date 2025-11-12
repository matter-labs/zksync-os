#![no_main]

use basic_system::system_implementation::system::da_commitment_generator::blob_commitment_generator::{ENCODABLE_BYTES_PER_BLOB, blob_versioned_hash_with_advisor, commitment_and_proof_advice::BlobCommitmentAndProofAdvisor};
use callable_oracles::blob_kzg_commitment::blob_kzg_commitment_and_proof;
use libfuzzer_sys::fuzz_target;
use arbitrary::Arbitrary;

struct BlobCommitmentAndProofAdvisorImplementation;

impl BlobCommitmentAndProofAdvisor for BlobCommitmentAndProofAdvisorImplementation {
    fn get_blob_commitment_and_proof_advice(
        &mut self,
        data: &[u8],
    ) -> rig::basic_system::system_implementation::system::da_commitment_generator::KZGCommitmentAndProof{
        blob_kzg_commitment_and_proof(data)
    }
}

#[derive(Debug, Arbitrary)]
struct Input {
    data: [u8; ENCODABLE_BYTES_PER_BLOB],
}

fuzz_target!(|data: &[u8]| {
    if data.len() > ENCODABLE_BYTES_PER_BLOB {
        return;
    }

    let mut advisor = BlobCommitmentAndProofAdvisorImplementation;

    let _ = blob_versioned_hash_with_advisor(data, &mut advisor);
});
