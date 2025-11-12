use crate::{
    internal_error,
    oracle::{query_ids::DA_COMMITMENT_SCHEME_QUERY_ID, IOOracle},
    system::errors::internal::InternalError,
};

///
/// Rust representation of `L2DACommitmentScheme` from l1 contracts.
/// It's used to define DA commitment zksync os outputs.
///
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum DACommitmentScheme {
    /// Invalid option.
    None,
    /// Commitment will be equals to 0, used for validiums.
    EmptyNoDA,
    /// Keccak of stateDiffHash and keccak(pubdata). Can be used by custom DA solutions.
    /// Currently not supported.
    PubdataKeccak256,
    /// This commitment includes EIP-4844 blobs data. Used by default RollupL1DAValidator.
    /// With ZKsync OS it always outputs 1 0-hash blob, as separate commitment used for blobs.
    BlobsAndPubdataKeccak256,
    /// Keccak of blob versioned hashes filled with pubdata.
    BlobsZKsyncOS,
}

impl TryFrom<u8> for DACommitmentScheme {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(DACommitmentScheme::None),
            1 => Ok(DACommitmentScheme::EmptyNoDA),
            2 => Ok(DACommitmentScheme::PubdataKeccak256),
            3 => Ok(DACommitmentScheme::BlobsAndPubdataKeccak256),
            4 => Ok(DACommitmentScheme::BlobsZKsyncOS),
            _ => Err(()),
        }
    }
}

impl DACommitmentScheme {
    pub fn try_from_oracle<O: IOOracle>(oracle: &mut O) -> Result<Self, InternalError> {
        let da_commitment_scheme_id_raw: u8 =
            oracle.query_with_empty_input(DA_COMMITMENT_SCHEME_QUERY_ID)?;
        DACommitmentScheme::try_from(da_commitment_scheme_id_raw)
            .map_err(|_| internal_error!("Invalid DA commitment scheme ID"))
    }
}
