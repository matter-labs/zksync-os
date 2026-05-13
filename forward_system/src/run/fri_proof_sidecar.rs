use zk_ee::utils::Bytes32;

/// Source of raw FRI proof bytes keyed by `statement_versioned_hash`.
pub trait FriProofSidecarSource: 'static {
    /// Returns the raw (bincode-serialized) `UnrolledProgramProof`
    /// bytes stored under this `statement_versioned_hash`.
    ///
    /// Returns `None` if the sidecar has no entry for this hash.
    fn get_proof_bytes(&mut self, statement_versioned_hash: Bytes32) -> Option<Vec<u8>>;
}

/// A no-op sidecar source used when FRI proof verification is not
/// wired up (non-Gateway chains, or code paths that don't receive
/// `FRI_PROOF_TX_TYPE` txs).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoFriProofSidecar;

impl FriProofSidecarSource for NoFriProofSidecar {
    fn get_proof_bytes(&mut self, _statement_versioned_hash: Bytes32) -> Option<Vec<u8>> {
        None
    }
}

pub struct FromInterfaceSidecar<S>(pub S);

impl<S: zksync_os_interface::traits::FriProofSidecarSource> FriProofSidecarSource
    for FromInterfaceSidecar<S>
{
    fn get_proof_bytes(&mut self, statement_versioned_hash: Bytes32) -> Option<Vec<u8>> {
        let bytes = *statement_versioned_hash.as_u8_array_ref();
        self.0.get_proof_bytes(alloy::primitives::B256::from(bytes))
    }
}
