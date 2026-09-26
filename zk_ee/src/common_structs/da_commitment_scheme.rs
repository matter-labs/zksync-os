use crate::{
    internal_error,
    oracle::{
        memory_io::{MemoryOracle, OracleQuery, ShortDeserializable, ShortSerializable},
        query_ids::DA_COMMITMENT_SCHEME_QUERY_ID,
    },
    system::errors::internal::InternalError,
};

///
/// Rust representation of `L2DACommitmentScheme` from l1 contracts.
///
/// This is the DA commitment *mechanism* — how the committed pubdata is
/// published and hashed. Which *part* of the pubdata is committed (full vs
/// logs-only) is a separate, orthogonal axis, [`PubdataContent`], carried in the chain
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

/// The scheme is received as a short word: its ID, range-checked.
impl ShortSerializable for DACommitmentScheme {
    #[inline(always)]
    fn to_short_word(self) -> u32 {
        u32::from(self as u8)
    }
}

impl ShortDeserializable for DACommitmentScheme {
    #[inline(always)]
    fn from_short_word(word: u32) -> Result<Self, InternalError> {
        u8::try_from(word)
            .ok()
            .and_then(|id| DACommitmentScheme::try_from(id).ok())
            .ok_or_else(|| internal_error!("Invalid DA commitment scheme ID"))
    }
}

pub struct DACommitmentSchemeQuery;

impl OracleQuery for DACommitmentSchemeQuery {
    const QUERY_ID: u32 = DA_COMMITMENT_SCHEME_QUERY_ID;
    type Input = ();
    type Output = DACommitmentScheme;
}

impl DACommitmentScheme {
    pub fn try_from_oracle<O: MemoryOracle>(oracle: &mut O) -> Result<Self, InternalError> {
        DACommitmentSchemeQuery::get(oracle, ())
    }
}

///
/// Pubdata content: which part of the pubdata the batch commits to.
///
/// This is orthogonal to [`DACommitmentScheme`] (which selects the *mechanism* —
/// calldata keccak vs EIP-4844 blobs). The content selects the *scope*:
/// - `FullPubdata` commits the full pubdata (state diffs + logs + message payloads).
/// - `LogsOnly` commits only the mandatory L2->L1 log section (log records,
///   including the interop commitment (IMT) leaves), leaving state diffs and
///   message payloads to the operator.
///
/// It is a chain-level rule carried in [`ChainConfig`](crate::system::metadata::chain_config)
/// and thereby committed into the batch public input via the chain config hash,
/// so the settlement layer can enforce the chain's configured content.
///
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum PubdataContent {
    /// The whole pubdata is committed and must be published.
    FullPubdata,
    /// Only the mandatory L2->L1 log section is committed; state diffs and
    /// message payloads are published at the operator's discretion.
    LogsOnly,
}

impl TryFrom<u8> for PubdataContent {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(PubdataContent::FullPubdata),
            1 => Ok(PubdataContent::LogsOnly),
            _ => Err(()),
        }
    }
}

impl PubdataContent {
    /// Whether the batch commits the full pubdata (`FullPubdata`) or only the
    /// mandatory logs section (`LogsOnly`).
    pub fn commits_full_pubdata(&self) -> bool {
        matches!(self, PubdataContent::FullPubdata)
    }
}
