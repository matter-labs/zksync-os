//! Shared bincode-decode + flatten helper for FRI proof bytes.

use crate::run::query_processors::FriVerifierArtifacts;
use execution_utils::unified_circuit::flatten_proof_into_responses_for_unified_recursion;
use execution_utils::unrolled::UnrolledProgramProof;

/// Errors returned by [`decode_and_flatten_proof`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeAndFlattenError {
    /// The bytes failed to decode as a bincode-serialized
    /// `UnrolledProgramProof`. Likely cause: corrupted sidecar
    /// storage, encoder version drift, or adversarial garbage.
    BincodeDecode,
}

/// Decode bincode `proof_bytes` into an `UnrolledProgramProof` and
/// flatten it into the verifier word stream the airbender unified
/// verifier reads.
pub fn decode_and_flatten_proof(
    proof_bytes: &[u8],
    artifacts: &FriVerifierArtifacts,
) -> Result<Vec<u32>, DecodeAndFlattenError> {
    let bincode_config = bincode_v2::config::standard();
    let (proof, _) = bincode_v2::serde::decode_from_slice::<UnrolledProgramProof, _>(
        proof_bytes,
        bincode_config,
    )
    .map_err(|_| DecodeAndFlattenError::BincodeDecode)?;

    Ok(flatten_proof_into_responses_for_unified_recursion(
        &proof,
        &artifacts.setup,
        &artifacts.compiled_layouts,
        false,
    ))
}
