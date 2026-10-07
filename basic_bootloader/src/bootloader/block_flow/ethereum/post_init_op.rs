use super::*;
use crate::bootloader::supported_ees::EthereumLikeSystem;
use system_hooks::add_precompiles;
use zk_ee::system::errors::internal::InternalError;

impl<S: EthereumLikeSystem> PostSystemInitOp<S> for EthereumPostInitOp
where
    S::IO: IOSubsystemExt,
{
    fn post_init_op<Config: BasicBootloaderExecutionConfig>(
        _system: &mut System<S>,
        system_functions: &mut HooksStorage<S, <S as SystemTypes>::Allocator>,
    ) -> Result<(), InternalError> {
        add_precompiles(system_functions)?;

        Ok(())
    }
}
