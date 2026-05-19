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

unsafe impl<C: wincode::config::ConfigCore> wincode::SchemaWrite<C> for KZGCommitmentAndProof {
    type Src = Self;

    fn size_of(_src: &Self) -> wincode::WriteResult<usize> {
        Ok(48 + 48)
    }

    fn write(mut writer: impl wincode::io::Writer, src: &Self) -> wincode::WriteResult<()> {
        <[u8; 48] as wincode::SchemaWrite<C>>::write(writer.by_ref(), &src.commitment)?;
        <[u8; 48] as wincode::SchemaWrite<C>>::write(writer.by_ref(), &src.proof)?;
        Ok(())
    }
}

unsafe impl<'de, C: wincode::config::ConfigCore> wincode::SchemaRead<'de, C>
    for KZGCommitmentAndProof
{
    type Dst = Self;

    fn read(
        mut reader: impl wincode::io::Reader<'de>,
        dst: &mut core::mem::MaybeUninit<Self>,
    ) -> wincode::ReadResult<()> {
        let mut commitment = core::mem::MaybeUninit::<[u8; 48]>::uninit();
        <[u8; 48] as wincode::SchemaRead<'de, C>>::read(reader.by_ref(), &mut commitment)?;
        let mut proof = core::mem::MaybeUninit::<[u8; 48]>::uninit();
        <[u8; 48] as wincode::SchemaRead<'de, C>>::read(reader.by_ref(), &mut proof)?;
        dst.write(Self {
            commitment: unsafe { commitment.assume_init() },
            proof: unsafe { proof.assume_init() },
        });
        Ok(())
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
                &(data.as_ptr() as usize as u64, data.len() as u64),
            )
            .expect("must deserialize commitment and proof")
    }
}
