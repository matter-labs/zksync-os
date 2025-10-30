use zk_ee::utils::write_bytes::WriteBytes;
use zk_ee::utils::Bytes32;

#[cfg(feature = "aggregation")]
mod blake2s_commitment_generator;
mod keccak256_commitment_generator;

#[cfg(feature = "aggregation")]
pub use blake2s_commitment_generator::Blake2sCommitmentGenerator;
pub use keccak256_commitment_generator::Keccak256CommitmentGenerator;

pub trait DACommitmentGenerator: WriteBytes {
    ///
    /// Generate DA commitment from the consumed data.
    ///
    /// Please note, that structure shouldn't be used after this call.
    /// It accepts `&mut self` to make the trait dyn compatible.
    ///
    fn finalize(&mut self) -> Bytes32;
}

pub struct NopCommitmentGenerator;

impl WriteBytes for NopCommitmentGenerator {
    fn write(&mut self, _buf: &[u8]) {}
}

impl DACommitmentGenerator for NopCommitmentGenerator {
    fn finalize(&mut self) -> Bytes32 {
        Bytes32::zero()
    }
}
