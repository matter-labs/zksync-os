//! System hooks used by ethereum STF, re-exported from shared hooks.

#[cfg(feature = "eip-152")]
pub(crate) use crate::bootloader::hooks::eip_152;
#[cfg(feature = "eip-2537")]
pub(crate) use crate::bootloader::hooks::eip_2537;
