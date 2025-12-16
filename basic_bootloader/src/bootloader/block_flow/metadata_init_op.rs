use super::*;
use zk_ee::oracle::IOOracle;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::system::SystemTypes;

pub trait MetadataInitOp<S: SystemTypes> {
    fn metadata_op<'a, Config: BasicBootloaderExecutionConfig>(
        oracle: &mut impl IOOracle,
        allocator: S::Allocator,
    ) -> Result<S::Metadata, InternalError>;
}
