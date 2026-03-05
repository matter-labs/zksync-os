use crate::add_precompile;
use evm_interpreter::precompile_addresses::BLAKE2F_HOOK_ADDRESS_LOW;
use zk_ee::{
    common_structs::system_hooks::HooksStorage,
    interface_error,
    system::{errors::internal::InternalError, EthereumLikeTypes, IOSubsystemExt},
};

define_subsystem!(Blake2FPrecompile,
  interface Blake2FPrecompileInterfaceError
  {
      InvalidInputSize,
      InvalidBooleanFlag,
  }
);

use evm_interpreter::ERGS_PER_GAS;

use zk_ee::define_subsystem;

mod impls;
mod mixing_function;
pub use self::impls::Blake2FPrecompile;

pub fn initialize_blake2f<S: EthereumLikeTypes, A: core::alloc::Allocator + Clone>(
    hooks_storage: &mut HooksStorage<S, A>,
) -> Result<(), InternalError>
where
    S::IO: IOSubsystemExt,
{
    add_precompile::<S, A, Blake2FPrecompile, Blake2FPrecompileErrors>(
        hooks_storage,
        BLAKE2F_HOOK_ADDRESS_LOW,
    )
}
