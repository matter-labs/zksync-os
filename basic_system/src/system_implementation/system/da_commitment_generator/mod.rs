use alloc::alloc::Allocator;
use alloc::boxed::Box;
use zk_ee::utils::write_bytes::WriteBytes;
use zk_ee::utils::Bytes32;

#[cfg(feature = "aggregation")]
mod blake2s_commitment_generator;
pub mod blob_commitment_generator;
mod keccak256_commitment_generator;

#[cfg(feature = "aggregation")]
pub use blake2s_commitment_generator::Blake2sCommitmentGenerator;
pub use blob_commitment_generator::commitment_and_proof_advice::KZGCommitmentAndProof;
pub use blob_commitment_generator::commitment_and_proof_advice::BLOB_COMMITMENT_AND_PROOF_QUERY_ID;
pub use blob_commitment_generator::BlobCommitmentGenerator;
pub use keccak256_commitment_generator::Keccak256CommitmentGenerator;
use zk_ee::common_structs::da_commitment_scheme::DACommitmentScheme;
use zk_ee::internal_error;
use zk_ee::oracle::IOOracle;
use zk_ee::system::errors::internal::InternalError;

pub trait DACommitmentGenerator<O: IOOracle>: WriteBytes {
    ///
    /// Generate DA commitment from the consumed data.
    ///
    /// Please note, that structure shouldn't be used after this call.
    /// It accepts `&mut self` to make the trait dyn compatible.
    ///
    fn finalize(&mut self, oracle: &mut O) -> Bytes32;
}

pub struct NopCommitmentGenerator;

impl WriteBytes for NopCommitmentGenerator {
    fn write(&mut self, _buf: &[u8]) {}
}

impl<O: IOOracle> DACommitmentGenerator<O> for NopCommitmentGenerator {
    fn finalize(&mut self, _oracle: &mut O) -> Bytes32 {
        Bytes32::zero()
    }
}

pub fn da_commitment_generator_from_scheme<A: Allocator, O: IOOracle>(
    da_commitment_scheme: DACommitmentScheme,
    alloc: A,
) -> Result<Box<dyn DACommitmentGenerator<O>, A>, InternalError> {
    match da_commitment_scheme {
        DACommitmentScheme::BlobsAndPubdataKeccak256 => Ok(alloc::boxed::Box::new_in(
            Keccak256CommitmentGenerator::new(),
            alloc,
        )),
        DACommitmentScheme::EmptyNoDA => {
            Ok(alloc::boxed::Box::new_in(NopCommitmentGenerator, alloc))
        }
        DACommitmentScheme::BlobsZKsyncOS => Ok(alloc::boxed::Box::new_in(
            BlobCommitmentGenerator::new(),
            alloc,
        )),
        _ => Err(internal_error!("Unsupported DA commitment scheme")),
    }
}
