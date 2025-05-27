use super::*;

///
/// Cache for preimages of hashes.
/// Used for bytecode hashes and account hashes.
///
pub trait PreimageCacheModel: Sized {
    type Resources: Resources;
    type TxStats;
    type PreimageRequest;
    type StateSnapshot;

    fn begin_new_tx(&mut self);
    fn tx_stats(&self) -> Self::TxStats;

    fn start_frame(&mut self) -> Self::StateSnapshot;
    fn finish_frame(&mut self, rollback_handle: Option<&Self::StateSnapshot>);

    fn get_preimage<const PROOF_ENV: bool>(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        preimage_type: &Self::PreimageRequest,
        resources: &mut Self::Resources,
        oracle: &mut impl IOOracle,
    ) -> Result<&'static [u8], SystemError>;

    fn record_preimage<const PROOF_ENV: bool>(
        &mut self,
        ee_type: ExecutionEnvironmentType,
        preimage_type: &Self::PreimageRequest,
        resources: &mut Self::Resources,
        preimage: &[u8],
    ) -> Result<&'static [u8], SystemError>;
}
