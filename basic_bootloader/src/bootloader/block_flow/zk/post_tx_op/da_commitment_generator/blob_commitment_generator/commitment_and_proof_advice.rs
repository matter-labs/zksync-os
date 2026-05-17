use serde_big_array::BigArray;

pub const BLOB_COMMITMENT_AND_PROOF_QUERY_ID: u32 =
    zk_ee::oracle::query_ids::ADVICE_SUBSPACE_MASK | 0x20;

#[derive(serde::Serialize, serde::Deserialize)]
#[repr(C, align(8))]
pub struct KZGCommitmentAndProof {
    #[serde(with = "BigArray")]
    pub commitment: [u8; 48],
    #[serde(with = "BigArray")]
    pub proof: [u8; 48],
}

pub trait BlobCommitmentAndProofAdvisor {
    fn get_blob_commitment_and_proof_advice(&mut self, data: &[u8]) -> KZGCommitmentAndProof;
}

pub struct OracleBasedBlobCommitmentAndProofAdvisor<'a, O: zk_ee::oracle::IOOracle> {
    pub oracle: &'a mut O,
}

impl<'a, O: zk_ee::oracle::IOOracle> BlobCommitmentAndProofAdvisor
    for OracleBasedBlobCommitmentAndProofAdvisor<'a, O>
{
    fn get_blob_commitment_and_proof_advice(&mut self, data: &[u8]) -> KZGCommitmentAndProof {
        self.oracle
            .query(
                BLOB_COMMITMENT_AND_PROOF_QUERY_ID,
                &(data.as_ptr() as usize as u64, data.len() as u64),
            )
            .expect("must deserialize commitment and proof")
    }
}
