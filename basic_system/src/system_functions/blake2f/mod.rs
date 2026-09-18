use zk_ee::interface_error;
use zk_ee::system::base_system_functions::{
    Blake2FPrecompileErrors, Blake2FPrecompileInterfaceError,
};

mod impls;
mod mixing_function;
pub use self::impls::Blake2FPrecompile;
