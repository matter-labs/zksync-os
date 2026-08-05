use super::*;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::system::SystemTypes;

/// Trait for operations performed after system initialization but before transaction processing.
///
/// This phase typically sets up system contracts, precompiles, and other infrastructure
/// needed for transaction execution. Called once per block after system metadata is loaded.
pub trait PostSystemInitOp<S: SystemTypes> {
    /// Performs post-initialization setup
    fn post_init_op<Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
    ) -> Result<(), InternalError>;
}
