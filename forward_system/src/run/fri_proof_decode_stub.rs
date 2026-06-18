use crate::run::query_processors::FriVerifierArtifacts;

#[allow(dead_code)]
pub fn decode_and_flatten_proof(
    proof_bytes: &[u8],
    artifacts: &FriVerifierArtifacts,
) -> Option<Vec<u32>> {
    let _ = (proof_bytes, artifacts);
    None
}
