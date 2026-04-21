use zk_ee::utils::Bytes32;

/// Source of raw FRI proof bytes keyed by `statement_versioned_hash`.
///
/// The sidecar is a dumb byte store: it holds the `UnrolledProgramProof`
/// bytes that were received alongside each `FriProofTx` at admission
/// time and hands them back when the bootloader issues a
/// `FRI_PROOF_QUERY_ID` oracle query.
///
/// All decoding and flattening of the proof into the verifier's
/// oracle word stream happens inside `FriProofResponder`, which owns
/// the setup and compiled circuit layouts required for that work.
pub trait FriProofSidecarSource: 'static {
    /// Returns the raw (bincode-serialized) `UnrolledProgramProof`
    /// bytes stored under this `statement_versioned_hash`.
    ///
    /// Returns `None` if the sidecar has no entry for this hash. In
    /// production that should never happen: the admission path
    /// pre-validates and populates the sidecar before the tx reaches
    /// execution. Returning `None` here causes the FRI tx handler to
    /// fail the binding check and reject the tx.
    fn get_proof_bytes(&mut self, statement_versioned_hash: Bytes32) -> Option<Vec<u8>>;
}

/// A no-op sidecar source used when FRI proof verification is not
/// wired up (non-Gateway chains, or code paths that don't receive
/// `FRI_PROOF_TX_TYPE` txs). Any attempt to resolve a statement hash
/// returns `None`, which causes the FRI tx handler to reject the tx
/// with a missing-sidecar error.
///
/// This is the placeholder used by `RunBlockForward`'s trait impl
/// because the upstream `zksync_os_interface::traits::RunBlock` trait
/// does not carry an `FriProofSidecarSource` parameter. Callers that
/// actually process FRI proofs should instead call the free-function
/// `run_block` directly with their own sidecar.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoFriProofSidecar;

impl FriProofSidecarSource for NoFriProofSidecar {
    fn get_proof_bytes(&mut self, _statement_versioned_hash: Bytes32) -> Option<Vec<u8>> {
        None
    }
}
