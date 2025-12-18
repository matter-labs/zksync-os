use super::*;
use zk_ee::system::errors::internal::InternalError;

impl<S: EthereumLikeTypes> PostSystemInitOp<S> for ZKHeaderPostInitOp
where
    S::IO: IOSubsystemExt,
{
    fn post_init_op<Config: BasicBootloaderExecutionConfig>(
        _system: &mut System<S>,
        system_functions: &mut HooksStorage<S, <S as SystemTypes>::Allocator>,
    ) -> Result<(), InternalError> {
        system_hooks::add_precompiles(system_functions)?;

        #[cfg(not(feature = "disable_system_contracts"))]
        {
            system_hooks::add_l1_messenger(system_functions)?;
            system_hooks::add_l2_base_token(system_functions)?;
            system_hooks::add_contract_deployer(system_functions)?;
            system_hooks::add_interop_root_reporter(system_functions)?;
        }

        Ok(())
    }
}
