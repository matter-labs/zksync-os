use crate::system_implementation::system::da_commitment_generator::DACommitmentGenerator;
use crypto::sha3::Keccak256;
use crypto::MiniDigest;
use zk_ee::utils::write_bytes::WriteBytes;
use zk_ee::utils::Bytes32;

pub struct Keccak256CommitmentGenerator {
    pubdata_hasher: Keccak256,
}

impl Keccak256CommitmentGenerator {
    pub fn new() -> Self {
        Self {
            pubdata_hasher: Keccak256::new(),
        }
    }
}

impl WriteBytes for Keccak256CommitmentGenerator {
    fn write(&mut self, buf: &[u8]) {
        self.pubdata_hasher.update(buf);
    }
}

impl DACommitmentGenerator for Keccak256CommitmentGenerator {
    fn finalize(&mut self) -> Bytes32 {
        let mut da_commitment_hasher = crypto::sha3::Keccak256::new();
        da_commitment_hasher.update([0u8; 32]); // we don't have to validate state diffs hash
        da_commitment_hasher.update(self.pubdata_hasher.finalize_reset()); // full pubdata keccak
        da_commitment_hasher.update([1u8]); // with calldata we should provide 1 blob
        da_commitment_hasher.update([0u8; 32]); // its hash will be ignored on the settlement layer
        da_commitment_hasher.finalize().into()
    }
}
