use super::*;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::system::SystemTypes;

pub trait PostSystemInitOp<S: SystemTypes> {
    fn post_init_op<Config: BasicBootloaderExecutionConfig>(
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, S::Allocator>,
    ) -> Result<(), InternalError>;
}
