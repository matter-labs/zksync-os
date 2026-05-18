use super::*;
use zk_ee::system::errors::internal::InternalError;
use zk_ee::system::metadata::basic_metadata::BasicBlockMetadata;

impl<S: EthereumLikeTypes> PostSystemInitOp<S> for ZKHeaderPostInitOp
where
    S::IO: IOSubsystemExt,
{
    fn post_init_op<Config: BasicBootloaderExecutionConfig>(
        #[cfg_attr(feature = "disable_system_contracts", allow(unused_variables))]
        system: &mut System<S>,
        system_functions: &mut HooksStorage<S, <S as SystemTypes>::Allocator>,
    ) -> Result<(), InternalError> {
        system_hooks::add_precompiles(system_functions)?;

        // TODO: maybe rename
        #[cfg(not(feature = "disable_system_contracts"))]
        {
            system_hooks::add_l1_messenger(system_functions)?;
            system_hooks::add_set_bytecode_on_address_hook(system_functions)?;
            system_hooks::add_contract_deployer(system_functions)?;
            system_hooks::add_interop_root_reporter(system_functions)?;
            system_hooks::add_system_context_reporter(system_functions)?;

            // TODO(EVM-1191): temporary solution, should be removed before the release
            system_hooks::add_base_token_mint(system_functions)?;

            // Gateway-only system hook
            if system.metadata.is_gateway() {
                system_hooks::add_fri_proof_verification_hook(system_functions)?;
            }
        }

        Ok(())
    }
}
