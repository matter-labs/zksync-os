use crate::run::query_processors::FriVerifierArtifacts;
use basic_bootloader::bootloader::fri_host_verifier::FriHostVerifyError;
use zk_ee::utils::Bytes32;

/// Errors returned by [`validate_fri_statement`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FriAdmissionError {
    /// The `fri_precompile` feature is disabled in this build.
    FeatureDisabled,
    /// `proof_bytes` could not be decoded as a bincode
    /// `UnrolledProgramProof`. Caller is expected to reject the
    /// transaction.
    BincodeDecode,
    /// Host verification failed; see inner variant for the verdict.
    Verify(FriHostVerifyError),
}

pub fn validate_fri_statement(
    statement_versioned_hash: Bytes32,
    proof_bytes: &[u8],
    artifacts: &FriVerifierArtifacts,
) -> Result<(), FriAdmissionError> {
    let _ = (statement_versioned_hash, proof_bytes, artifacts);
    Err(FriAdmissionError::FeatureDisabled)
}
