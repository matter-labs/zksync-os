//! Standalone FRI proof admission API.

use crate::run::fri_proof_decode::{decode_and_flatten_proof, DecodeAndFlattenError};
use crate::run::query_processors::FriVerifierArtifacts;
use basic_bootloader::bootloader::fri_admission::{
    run_host_verifier, statement_versioned_hash_from_verifier_output, FriHostVerifyError,
};
use zk_ee::utils::Bytes32;

/// Errors returned by [`validate_fri_statement`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FriAdmissionError {
    /// `proof_bytes` could not be decoded as a bincode
    /// `UnrolledProgramProof`. Caller is expected to reject the
    /// transaction.
    BincodeDecode,
    /// The verifier rejected the proof. Either the airbender
    /// verifier panicked on malformed input, or the input stream
    /// carried trailing words the verifier did not consume.
    VerifierRejected,
    /// The verifier returned a result, but the statement hash
    /// re-derived from its 16-register output did not match the
    /// caller-supplied `statement_versioned_hash`. The proof is
    /// internally valid but does not prove the claimed statement —
    /// reject.
    StatementHashMismatch,
    /// Could not spawn the dedicated verifier worker thread (system
    /// resource exhaustion).
    VerifierThreadSpawn,
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
        match err {
            FriHostVerifyError::VerifierThreadSpawn => Self::VerifierThreadSpawn,
            // Trailing words, wrong op type, and verifier-panic all
            // indicate the proof is not valid input.
            FriHostVerifyError::VerifierRejected
            | FriHostVerifyError::TrailingWords
            | FriHostVerifyError::UnsupportedOpType => Self::VerifierRejected,
        }
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
    let output = run_host_verifier(&verifier_words)?;
    let computed = statement_versioned_hash_from_verifier_output(&output);
    if computed != statement_versioned_hash {
        return Err(FriAdmissionError::StatementHashMismatch);
    }
    Ok(())
}
