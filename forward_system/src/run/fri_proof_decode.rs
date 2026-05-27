//! Shared bincode-decode + flatten helper for FRI proof bytes.

use crate::run::query_processors::FriVerifierArtifacts;
use execution_utils::unified_circuit::flatten_proof_into_responses_for_unified_recursion;
use execution_utils::unrolled::UnrolledProgramProof;

/// Decode bincode `proof_bytes` into an `UnrolledProgramProof` and
/// flatten it into the verifier word stream the airbender unified
/// verifier reads. Returns `None` if `proof_bytes` is not a valid
/// bincode-serialized `UnrolledProgramProof`.
pub fn decode_and_flatten_proof(
    proof_bytes: &[u8],
    artifacts: &FriVerifierArtifacts,
) -> Option<Vec<u32>> {
    let bincode_config = bincode_v2::config::standard();
    let (proof, _) = bincode_v2::serde::decode_from_slice::<UnrolledProgramProof, _>(
        proof_bytes,
        bincode_config,
    )
    .ok()?;

    Some(flatten_proof_into_responses_for_unified_recursion(
        &proof,
        &artifacts.setup,
        &artifacts.compiled_layouts,
        false,
    ))
}
