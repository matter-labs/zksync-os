use super::*;
use zk_ee::oracle::IOOracle;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::system::SystemTypes;

/// Trait for initializing block metadata at the start of block execution.
///
/// This operation should query the oracle for block-level configuration (gas limits,
/// timestamps, etc.) and validate the metadata before block processing begins.
pub trait MetadataInitOp<S: SystemTypes> {
    /// Initializes metadata for a new block.
    fn metadata_op<Config: BasicBootloaderExecutionConfig>(
        oracle: &mut impl IOOracle,
        allocator: S::Allocator,
    ) -> Result<S::Metadata, InternalError>;
}
