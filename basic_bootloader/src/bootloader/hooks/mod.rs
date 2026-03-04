//! Shared system hooks used by both ZK and Ethereum STFs.

#[cfg(feature = "eip-2537")]
pub mod eip_2537;

#[cfg(feature = "eip-152")]
pub mod eip_152;
