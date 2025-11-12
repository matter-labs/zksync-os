use zk_ee::internal_error;
use zk_ee::oracle::usize_serialization::{UsizeDeserializable, UsizeSerializable};
use zk_ee::system::errors::internal::InternalError;
use zk_ee::utils::exact_size_chain::ExactSizeChain;

pub const BLOB_COMMITMENT_AND_PROOF_QUERY_ID: u32 =
    zk_ee::oracle::query_ids::ADVICE_SUBSPACE_MASK | 0x20;

pub struct KZGCommitmentAndProof {
    pub commitment: [u8; 48],
    pub proof: [u8; 48],
}

impl UsizeSerializable for KZGCommitmentAndProof {
    const USIZE_LEN: usize = 96 / size_of::<usize>();

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        cfg_if::cfg_if!(
            if #[cfg(target_endian = "big")] {
                compile_error!("unsupported architecture: big endian arch is not supported")
            } else {
                #[allow(clippy::needless_return)]
                return ExactSizeChain::new(
                    self.commitment.array_chunks::<{ core::mem::size_of::<usize>() }>().map(|chunk| usize::from_le_bytes(*chunk)),
                    self.proof.array_chunks::<{ core::mem::size_of::<usize>() }>().map(|chunk| usize::from_le_bytes(*chunk)),
                );
            }
        );
    }
}

impl UsizeDeserializable for KZGCommitmentAndProof {
    const USIZE_LEN: usize = <Self as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let mut commitment = [0u8; 48];
        let mut proof = [0u8; 48];
        const FIELD_USIZE_LEN: usize = 48 / core::mem::size_of::<usize>();
        unsafe {
            let commitment_usize_ptr: *mut usize = commitment
                .as_mut_ptr()
                .cast::<[usize; FIELD_USIZE_LEN]>()
                .cast();
            for i in 0..FIELD_USIZE_LEN {
                commitment_usize_ptr
                    .add(i)
                    .write(src.next().ok_or(internal_error!(
                        "KZGCommitmentAndProof deserialization failed"
                    ))?);
            }
            let proof_usize_ptr: *mut usize =
                proof.as_mut_ptr().cast::<[usize; FIELD_USIZE_LEN]>().cast();
            for i in 0..FIELD_USIZE_LEN {
                proof_usize_ptr
                    .add(i)
                    .write(src.next().ok_or(internal_error!(
                        "KZGCommitmentAndProof deserialization failed"
                    ))?);
            }
        }
        Ok(Self { commitment, proof })
    }
}

pub trait BlobCommitmentAndProofAdvisor {
    fn get_blob_commitment_and_proof_advice(&mut self, data: &[u8]) -> KZGCommitmentAndProof;
}

pub struct OracleBasedBlobCommitmentAndProofAdvisor<'a, O: zk_ee::oracle::IOOracle> {
    pub oracle: &'a mut O,
}

// Right now, we run it only on RISC-V, but in theory, it should be possible to make it runnable on native arch as well
impl<'a, O: zk_ee::oracle::IOOracle> BlobCommitmentAndProofAdvisor
    for OracleBasedBlobCommitmentAndProofAdvisor<'a, O>
{
    fn get_blob_commitment_and_proof_advice(&mut self, data: &[u8]) -> KZGCommitmentAndProof {
        self.oracle
            .query_serializable(
                BLOB_COMMITMENT_AND_PROOF_QUERY_ID,
                &(data.as_ptr() as usize as u64, data.len() as u64),
            )
            .expect("must deserialize commitment and proof")
    }
}
