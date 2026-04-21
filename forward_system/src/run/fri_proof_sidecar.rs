use zk_ee::utils::Bytes32;

/// Source of FRI proof oracle streams keyed by `statement_versioned_hash`.
///
/// The sidecar source is responsible for:
/// 1. Holding the raw proof bytes received alongside each
///    `FriProofTx` at admission time.
/// 2. Decoding them (bincode-serialized `UnrolledProgramProof`).
/// 3. Flattening them together with the verifier's setup / layout
///    artifacts into the exact `u32` word sequence the Airbender
///    unified verifier will read via `DefaultNonDeterminismSource::read_word()`.
///
/// The forward-system `FriProofResponder` calls this trait whenever the
/// bootloader issues a `FRI_PROOF_QUERY_ID` query, and relays the
/// resulting `Vec<u32>` as the query response iterator.
///
/// Keeping the flattening on the source side (rather than in the
/// responder itself) keeps the responder stateless and means the
/// server / test rig can choose whatever backing store and flattening
/// pipeline fits its environment.
pub trait FriProofSidecarSource: 'static {
    /// Returns the flattened `u32` oracle stream for the proof at this
    /// `statement_versioned_hash`, ready to be consumed by the
    /// `verify_unrolled_or_unified_circuit_recursion_layer` verifier.
    ///
    /// Returns `None` if the sidecar has no entry for this hash. In
    /// production that should never happen: the admission path
    /// pre-validates and populates the sidecar before the tx reaches
    /// execution. Returning `None` here causes the FRI tx handler to
    /// fail the binding check and reject the tx.
    fn get_proof_oracle_stream(&mut self, statement_versioned_hash: Bytes32) -> Option<Vec<u32>>;
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
    fn get_proof_oracle_stream(&mut self, _statement_versioned_hash: Bytes32) -> Option<Vec<u32>> {
        None
    }
}
