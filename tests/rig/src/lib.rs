#![allow(incomplete_features)]
#![feature(generic_const_exprs)]
#![feature(allocator_api)]
#![feature(array_chunks)]
//!
//! This crate contains infrastructure to write ZKsync OS integration tests.
//! It contains `Chain` - in memory chain state structure with methods to run blocks, change state
//! and few utility methods(in the `utils` module) to encode transactions, load contracts, etc.
//!
use std::sync::Once;
pub mod chain;
pub mod utils;

pub use alloy;
pub use alloy_rlp;
pub use chain::BlockContext;
pub use chain::Chain;
pub use ethers;
pub use forward_system;
pub use log;
pub use risc_v_simulator::sim::ProfilerConfig;
pub use ruint;
pub use zk_ee;
pub use zksync_os_api;
pub use zksync_web3_rs;

static INIT_LOGGER_ONCE: Once = Once::new();
pub fn init_logger() {
    INIT_LOGGER_ONCE.call_once(env_logger::init);
}

#[allow(dead_code)]
mod colors {
    pub const RESET: &str = "\x1b[0m";

    pub const BLACK: &str = "\x1b[30m";
    pub const RED: &str = "\x1b[31m";
    pub const GREEN: &str = "\x1b[32m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const BLUE: &str = "\x1b[34m";
    pub const MAGENTA: &str = "\x1b[35m";
    pub const CYAN: &str = "\x1b[36m";
    pub const WHITE: &str = "\x1b[37m";

    pub const BRIGHT_BLACK: &str = "\x1b[90m";
    pub const BRIGHT_RED: &str = "\x1b[91m";
    pub const BRIGHT_GREEN: &str = "\x1b[92m";
    pub const BRIGHT_YELLOW: &str = "\x1b[93m";
    pub const BRIGHT_BLUE: &str = "\x1b[94m";
    pub const BRIGHT_MAGENTA: &str = "\x1b[95m";
    pub const BRIGHT_CYAN: &str = "\x1b[96m";
    pub const BRIGHT_WHITE: &str = "\x1b[97m";
}
