use super::{snapshottable_io::SnapshottableIo, *};

///
/// Cache for preimages of hashes.
/// Used for bytecode hashes and account hashes.
///
pub trait PreimageCacheModel: Sized + SnapshottableIo {
    type Resources: Resources;
    type PreimageRequest;

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
        preimage: &[&[u8]],
    ) -> Result<&'static [u8], SystemError>;
}
