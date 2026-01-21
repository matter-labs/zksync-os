use super::*;
use system_hooks::add_precompiles;
use zk_ee::system::errors::internal::InternalError;

impl<S: EthereumLikeTypes> PostSystemInitOp<S> for EthereumPostInitOp
where
    S::IO: IOSubsystemExt,
{
    fn post_init_op<Config: BasicBootloaderExecutionConfig>(
        _system: &mut System<S>,
        system_functions: &mut HooksStorage<S, <S as SystemTypes>::Allocator>,
    ) -> Result<(), InternalError> {
        add_precompiles(system_functions)?;

        hooks::eip_2537::initialize_eip_2537(system_functions)?;
        hooks::eip_152::initialize_eip_152(system_functions)?;

        Ok(())
    }
}
