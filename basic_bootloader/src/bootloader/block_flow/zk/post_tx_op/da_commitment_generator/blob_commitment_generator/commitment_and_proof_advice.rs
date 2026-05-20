pub const BLOB_COMMITMENT_AND_PROOF_QUERY_ID: u32 =
    zk_ee::oracle::query_ids::ADVICE_SUBSPACE_MASK | 0x20;

#[repr(C, align(8))]
pub struct KZGCommitmentAndProof {
    pub commitment: [u8; 48],
    pub proof: [u8; 48],
}

impl zk_ee::oracle::word_layout::WordLayout for KZGCommitmentAndProof {
    // 48 bytes each = 12 u32 words each = 24 total
    const WORD_COUNT: Option<usize> = Some(24);

    fn write_words(&self, w: &mut impl FnMut(u32)) {
        <[u8; 48] as zk_ee::oracle::word_layout::WordLayout>::write_words(&self.commitment, w);
        <[u8; 48] as zk_ee::oracle::word_layout::WordLayout>::write_words(&self.proof, w);
    }

    fn read_words(r: &mut impl FnMut() -> u32) -> Self {
        Self {
            commitment: <[u8; 48] as zk_ee::oracle::word_layout::WordLayout>::read_words(r),
            proof: <[u8; 48] as zk_ee::oracle::word_layout::WordLayout>::read_words(r),
        }
    }
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
                &(data.as_ptr() as u64, data.len() as u64),
            )
            .expect("must deserialize commitment and proof")
    }
}
