//! Host-side FRI verifier runner.

#![cfg(not(target_arch = "riscv32"))]

use crate::bootloader::fri_verifier::statement_versioned_hash_from_verifier_output;
use zk_ee::utils::Bytes32;

const FRI_HOST_VERIFIER_STACK_SIZE: usize = 1 << 27;

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
    let output = run_host_verifier(verifier_words)?;
    if statement_versioned_hash_from_verifier_output(&output) != statement_versioned_hash {
        return Err(FriHostVerifyError::StatementHashMismatch);
    }
    Ok(())
}

/// Run the host airbender unified verifier on a pre-flattened
/// verifier word stream and return the 16 final-register output.
fn run_host_verifier(verifier_words: &[u32]) -> Result<[u32; 16], FriHostVerifyError> {
    use full_statement_verifier::definitions::OP_VERIFY_UNIFIED_RECURSION_LAYER_IN_UNIFIED_CIRCUIT;
    use full_statement_verifier::verifier_common::prover::nd_source_std;

    // Pin the op type to unified recursion.
    let Some((&op_type, rest)) = verifier_words.split_first() else {
        return Err(FriHostVerifyError::UnsupportedOpType);
    };
    if op_type != OP_VERIFY_UNIFIED_RECURSION_LAYER_IN_UNIFIED_CIRCUIT {
        return Err(FriHostVerifyError::UnsupportedOpType);
    }

    // The verifier is stack-heavy. A plain `catch_unwind` would catch
    // verifier panics, but it would still run on the caller's stack and
    // would not make stack overflow a reliable recoverable error.
    //
    // `nd_source_std::set_iterator` stores a boxed `'static` iterator
    // in thread-local state, so the spawned verifier thread must own
    // the verifier words.
    let words = rest.to_vec();

    let join = std::thread::Builder::new()
        .name(std::string::String::from("fri-host-verifier"))
        .stack_size(FRI_HOST_VERIFIER_STACK_SIZE)
        .spawn(move || {
            nd_source_std::set_iterator(words.into_iter());
            let output = full_statement_verifier::unified_circuit_statement::verify_unified_circuit_recursion_layer(
                full_statement_verifier::verifier_common::SecurityModel::Security100,
            );
            let trailing = nd_source_std::try_read_word().is_some();
            (output, trailing)
        })
        .map_err(|_| FriHostVerifyError::VerifierThreadSpawn)?;

    // The verifier panics on malformed input; `join()` surfaces the
    // panic as `Err`. We do not propagate the payload: the caller
    // treats any panic as "proof is not valid".
    let (output, trailing) = join
        .join()
        .map_err(|_| FriHostVerifyError::VerifierRejected)?;
    if trailing {
        return Err(FriHostVerifyError::TrailingWords);
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_host_verifier_rejects_empty_stream() {
        let err = run_host_verifier(&[]).unwrap_err();
        assert_eq!(err, FriHostVerifyError::UnsupportedOpType);
    }

    #[test]
    fn run_host_verifier_rejects_non_unified_op_type() {
        use full_statement_verifier::definitions::OP_VERIFY_UNIFIED_RECURSION_LAYER_IN_UNIFIED_CIRCUIT;

        // Any op word other than the pinned unified op must be
        // rejected without invoking the upstream umbrella, so we
        // never accidentally accept unrolled proofs even with the
        // `verifiers` feature flipped on.
        let bogus_op = OP_VERIFY_UNIFIED_RECURSION_LAYER_IN_UNIFIED_CIRCUIT.wrapping_add(1);
        let err = run_host_verifier(&[bogus_op]).unwrap_err();
        assert_eq!(err, FriHostVerifyError::UnsupportedOpType);
    }

    #[test]
    fn run_host_verifier_rejects_garbage_after_correct_op() {
        use full_statement_verifier::definitions::OP_VERIFY_UNIFIED_RECURSION_LAYER_IN_UNIFIED_CIRCUIT;

        // Correct op type but garbage payload: verifier panics
        // inside the worker thread; the join boundary surfaces it
        // as VerifierRejected.
        let mut words = vec![OP_VERIFY_UNIFIED_RECURSION_LAYER_IN_UNIFIED_CIRCUIT];
        words.extend(0u32..256);
        let err = run_host_verifier(&words).unwrap_err();
        assert_eq!(err, FriHostVerifyError::VerifierRejected);
    }
}
