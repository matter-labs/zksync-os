use system_hooks::add_precompile;
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

pub const BLAKE_HOOK_ADDRESS_LOW: u16 = 0x0009;

pub fn initialize_eip_152<S: EthereumLikeTypes>(
    hooks_storage: &mut HooksStorage<S, S::Allocator>,
) -> Result<(), InternalError>
where
    S::IO: IOSubsystemExt,
{
    add_precompile::<S, S::Allocator, Blake2FPrecompile, Blake2FPrecompileErrors>(
        hooks_storage,
        BLAKE_HOOK_ADDRESS_LOW,
    )
}
