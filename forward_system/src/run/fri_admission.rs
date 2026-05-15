//! Standalone FRI proof admission API.

use crate::run::fri_proof_decode::{decode_and_flatten_proof, DecodeAndFlattenError};
use crate::run::query_processors::FriVerifierArtifacts;
use basic_bootloader::bootloader::fri_host_verifier::{
    verify_host_fri_statement, FriHostVerifyError,
};
use zk_ee::utils::Bytes32;

/// Errors returned by [`validate_fri_statement`].
///
/// Admission has two sources of failure: decoding the proof bytes,
/// and running the host verifier. `Verify` re-exports the bootloader's
/// `FriHostVerifyError` verbatim so callers can pattern-match on the
/// exact verifier verdict (e.g. distinguish `StatementHashMismatch`
/// from a malformed-proof rejection).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FriAdmissionError {
    /// `proof_bytes` could not be decoded as a bincode
    /// `UnrolledProgramProof`. Caller is expected to reject the
    /// transaction.
    BincodeDecode,
    /// Host verification failed; see inner variant for the verdict.
    Verify(FriHostVerifyError),
}

impl From<DecodeAndFlattenError> for FriAdmissionError {
    fn from(err: DecodeAndFlattenError) -> Self {
        match err {
            DecodeAndFlattenError::BincodeDecode => Self::BincodeDecode,
        }
    }
}

impl From<FriHostVerifyError> for FriAdmissionError {
    fn from(err: FriHostVerifyError) -> Self {
        Self::Verify(err)
    }
}

/// Verify that `proof_bytes` is a valid FRI proof for the claimed
/// `statement_versioned_hash`.
pub fn validate_fri_statement(
    statement_versioned_hash: Bytes32,
    proof_bytes: &[u8],
    artifacts: &FriVerifierArtifacts,
) -> Result<(), FriAdmissionError> {
    let verifier_words = decode_and_flatten_proof(proof_bytes, artifacts)?;
    verify_host_fri_statement(&verifier_words, statement_versioned_hash)?;
    Ok(())
}
