use crate::{
    internal_error,
    oracle::{
        query_ids::DA_COMMITMENT_SCHEME_QUERY_ID,
        usize_serialization::{UsizeDeserializable, UsizeSerializable},
        IOOracle,
    },
    system::errors::internal::InternalError,
};

///
/// Rust representation of `L2DACommitmentScheme` from l1 contracts.
///
/// This is the DA commitment *mechanism* — how the committed pubdata is
/// published and hashed. Which *part* of the pubdata is committed (full vs
/// logs-only) is a separate, orthogonal axis, [`DAMode`], carried in the chain
/// config.
///
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum DACommitmentScheme {
    /// Invalid option.
    None,
    /// Commitment equal to 0, used for validiums.
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

///
/// Data availability mode: which part of the pubdata the batch commits to.
///
/// This is orthogonal to [`DACommitmentScheme`] (which selects the *mechanism* —
/// calldata keccak vs EIP-4844 blobs). The mode selects the *scope*:
/// - `Rollup` commits the full pubdata (state diffs + logs + message payloads).
/// - `Validium` commits only the mandatory L2->L1 log section (log records,
///   including the interop commitment (IMT) leaves), leaving state diffs and
///   message payloads to the operator.
///
/// It is a chain-level rule carried in [`ChainConfig`](crate::system::metadata::chain_config)
/// and thereby committed into the batch public input via the chain config hash,
/// so the settlement layer can enforce the chain's configured mode.
///
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum DAMode {
    /// The whole pubdata is committed and must be published.
    Rollup,
    /// Only the mandatory L2->L1 log section is committed; state diffs and
    /// message payloads are published at the operator's discretion.
    Validium,
}

impl TryFrom<u8> for DAMode {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(DAMode::Rollup),
            1 => Ok(DAMode::Validium),
            _ => Err(()),
        }
    }
}

impl DAMode {
    /// Whether the batch commits the full pubdata (`Rollup`) or only the
    /// mandatory logs section (`Validium`).
    pub fn commits_full_pubdata(&self) -> bool {
        matches!(self, DAMode::Rollup)
    }
}

// Serialized as a single `u64`-width word (mirrors `bool`), so it composes with
// the other `ChainConfig` fields and works on the 32-bit proving target.
impl UsizeSerializable for DAMode {
    const USIZE_LEN: usize = <u64 as UsizeSerializable>::USIZE_LEN;

    fn iter(&self) -> impl ExactSizeIterator<Item = usize> {
        cfg_if::cfg_if!(
            if #[cfg(target_endian = "big")] {
                compile_error!("unsupported architecture: big endian arch is not supported")
            } else if #[cfg(target_pointer_width = "32")] {
                return [*self as usize, 0].into_iter();
            } else if #[cfg(target_pointer_width = "64")] {
                return core::iter::once(*self as usize);
            } else {
                compile_error!("unsupported architecture")
            }
        );
    }
}

impl UsizeDeserializable for DAMode {
    const USIZE_LEN: usize = <Self as UsizeSerializable>::USIZE_LEN;

    fn from_iter(src: &mut impl ExactSizeIterator<Item = usize>) -> Result<Self, InternalError> {
        let word = <u64 as UsizeDeserializable>::from_iter(src)?;
        let raw = u8::try_from(word).map_err(|_| internal_error!("Invalid DA mode"))?;
        DAMode::try_from(raw).map_err(|_| internal_error!("Invalid DA mode"))
    }
}
