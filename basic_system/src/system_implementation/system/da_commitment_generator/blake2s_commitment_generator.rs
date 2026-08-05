use crate::system_implementation::system::da_commitment_generator::DACommitmentGenerator;
use crypto::blake2s::Blake2s256;
use crypto::MiniDigest;
use zk_ee::oracle::IOOracle;
use zk_ee::utils::write_bytes::WriteBytes;
use zk_ee::utils::Bytes32;

pub struct Blake2sCommitmentGenerator {
    pubdata_hasher: Blake2s256,
}

impl Blake2sCommitmentGenerator {
    pub fn new() -> Self {
        Self {
            pubdata_hasher: Blake2s256::new(),
        }
    }
}

impl WriteBytes for Blake2sCommitmentGenerator {
    fn write(&mut self, buf: &[u8]) {
        self.pubdata_hasher.update(buf)
    }
}

impl<O: IOOracle> DACommitmentGenerator<O> for Blake2sCommitmentGenerator {
    fn finalize(&mut self, _oracle: &mut O) -> Bytes32 {
        self.pubdata_hasher.finalize_reset().into()
    }
}
