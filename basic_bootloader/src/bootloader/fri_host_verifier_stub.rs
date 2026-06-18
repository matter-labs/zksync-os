use zk_ee::utils::Bytes32;

/// Errors returned by host FRI verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FriHostVerifyError {
    /// The verifier thread could not be spawned (resource exhaustion).
    VerifierThreadSpawn,
    /// The verifier panicked or rejected the input. Treat as "proof
    /// is not valid": do not retry, do not surface internals.
    VerifierRejected,
    /// The verifier returned a result, but the non-determinism source
    /// still had words left. This means the input stream was longer
    /// than the verifier needed, which we treat as adversarial.
    TrailingWords,
    /// Stream is empty or the op-type prefix is not the unified
    /// recursion op. We only accept unified-circuit proofs; calling
    /// the umbrella dispatch would let the verifier branch on
    /// adversary-chosen op words.
    UnsupportedOpType,
    /// The verifier output does not bind to the claimed statement hash.
    StatementHashMismatch,
}

/// Verify a pre-flattened FRI statement word stream against the claimed
/// `statement_versioned_hash`.
pub fn verify_host_fri_statement(
    verifier_words: &[u32],
    statement_versioned_hash: Bytes32,
) -> Result<(), FriHostVerifyError> {
    let _ = (verifier_words, statement_versioned_hash);
    Err(FriHostVerifyError::VerifierRejected)
}
