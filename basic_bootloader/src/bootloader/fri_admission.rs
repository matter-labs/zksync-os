//! Host-side FRI proof admission helpers.
//!
//! These helpers exist so callers outside the bootloader can verify a single FRI
//! statement against its `statement_versioned_hash` before admitting a
//! `FriProofTx` to the block. This is an admission check that fails fast
//! on bad proofs so they never reach the prover.

use crate::bootloader::constants::FRI_STATEMENT_HASH_VERSION;
use crypto::{sha3::Keccak256, MiniDigest};
use zk_ee::utils::Bytes32;

/// Errors returned by [`run_host_verifier`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FriHostVerifyError {
    /// The verifier thread could not be spawned (resource exhaustion).
    VerifierThreadSpawn,
    /// The verifier panicked or rejected the input. Treat as "proof
    /// is not valid" — do not retry, do not surface internals.
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
}

/// Derive the versioned statement hash from the 16 `u32` words the
/// FRI unified verifier returns.
pub fn statement_versioned_hash_from_verifier_output(output: &[u32; 16]) -> Bytes32 {
    let mut hasher = Keccak256::new();
    hasher.update([FRI_STATEMENT_HASH_VERSION]);
    for word in output.iter() {
        hasher.update(word.to_le_bytes());
    }
    Bytes32::from_array(hasher.finalize())
}

/// Run the host airbender unified verifier on a pre-flattened
/// verifier word stream and return the 16 final-register output.
pub fn run_host_verifier(verifier_words: &[u32]) -> Result<[u32; 16], FriHostVerifyError> {
    use full_statement_verifier::definitions::OP_VERIFY_UNIFIED_RECURSION_LAYER_IN_UNIFIED_CIRCUIT;
    use full_statement_verifier::verifier_common::prover::nd_source_std;

    // Pin the op type to unified recursion.
    let Some((&op_type, rest)) = verifier_words.split_first() else {
        return Err(FriHostVerifyError::UnsupportedOpType);
    };
    if op_type != OP_VERIFY_UNIFIED_RECURSION_LAYER_IN_UNIFIED_CIRCUIT {
        return Err(FriHostVerifyError::UnsupportedOpType);
    }

    let words: Vec<u32> = rest.to_vec();

    let join = std::thread::Builder::new()
        .name("fri-admission-verifier".to_string())
        .stack_size(1 << 27)
        .spawn(move || {
            nd_source_std::set_iterator(words.into_iter());
            let output = full_statement_verifier::unified_circuit_statement::verify_unified_circuit_recursion_layer(
                full_statement_verifier::verifier_common::SecurityModel::Security80,
            );
            let trailing = nd_source_std::try_read_word().is_some();
            (output, trailing)
        })
        .map_err(|_| FriHostVerifyError::VerifierThreadSpawn)?;

    // The verifier panics on malformed input; `join()` surfaces the
    // panic as `Err`. We do not propagate the payload — the admission
    // API treats any panic as "proof is not valid".
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
    fn statement_hash_depends_on_all_verifier_words_and_version() {
        let mut output = [0u32; 16];
        for (idx, word) in output.iter_mut().enumerate() {
            *word = idx as u32 + 1;
        }

        let baseline = statement_versioned_hash_from_verifier_output(&output);

        output[8] ^= 0xdead_beef;
        let changed = statement_versioned_hash_from_verifier_output(&output);
        assert_ne!(baseline, changed);

        // Same registers without the version byte must differ.
        let mut hasher = Keccak256::new();
        for word in output.iter() {
            hasher.update(word.to_le_bytes());
        }
        let without_version = Bytes32::from_array(hasher.finalize());
        assert_ne!(changed, without_version);
    }

    #[test]
    fn run_host_verifier_rejects_empty_stream() {
        // Empty stream -> no op-type word -> immediate rejection
        // before the verifier thread is spawned.
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
        // Correct op type but garbage payload: verifier panics
        // inside the worker thread; the join boundary surfaces it
        // as VerifierRejected.
        use full_statement_verifier::definitions::OP_VERIFY_UNIFIED_RECURSION_LAYER_IN_UNIFIED_CIRCUIT;
        let mut words = vec![OP_VERIFY_UNIFIED_RECURSION_LAYER_IN_UNIFIED_CIRCUIT];
        words.extend(0u32..256);
        let err = run_host_verifier(&words).unwrap_err();
        assert_eq!(err, FriHostVerifyError::VerifierRejected);
    }
}
