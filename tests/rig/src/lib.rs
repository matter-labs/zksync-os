#![allow(incomplete_features)]
#![feature(generic_const_exprs)]
#![feature(allocator_api)]
#![feature(array_chunks)]
//! # ZKsync OS Test Rig
//!
//! This crate provides all infrastructure needed to write ZKsync OS integration tests.
//!
//! ## Core components
//!
//! | Module | What it contains |
//! |--------|-----------------|
//! | [`chain`] | [`Chain`] — in-memory chain state; [`BlockContext`]; [`RunConfig`] |
//! | [`constants`] | Named constants: chain ID, gas limits, fee params, system addresses |
//! | [`run_config`] | `RunConfig` preset constructors: [`run_config::forward_only`], [`run_config::full_proof`] |
//! | [`builder`] | [`builder::ChainBuilder`] and [`builder::TxBuilder`] — fluent APIs for setup & signing |
//! | [`assertions`] | Assertion macros: `assert_tx_success!`, `assert_tx_reverted!`, `assert_all_success!`, … |
//! | [`utils`] | Low-level helpers: `sign_and_encode_alloy_tx`, `load_sol_bytecode`, … |
//! | [`testing_utils`] | `call_address_and_measure_gas_cost` and call-tracer helpers |
//!
//! ## Quick-start example
//!
//! ```rust,ignore
//! use rig::{Chain, builder::ChainBuilder, builder::TxBuilder, run_config, constants::*};
//! use rig::utils::sign_and_encode_alloy_tx;
//!
//! #[test]
//! fn my_test() {
//!     let signer = PrivateKeySigner::random();
//!     let sender = B160::from_be_bytes(signer.address().into_array());
//!
//!     let mut chain = ChainBuilder::new()
//!         .with_balance(sender, U256::from(DEFAULT_BALANCE))
//!         .build();
//!
//!     let tx = TxBuilder::new()
//!         .from(signer)
//!         .to(some_address)
//!         .gas_limit(TRANSFER_GAS_LIMIT)
//!         .build();
//!
//!     let output = chain.run_block(vec![tx], None, None, Some(run_config::full_proof()));
//!     assert_tx_success!(output, 0);
//! }
//! ```
//!
//! See [`tests/TESTING.md`] for a full reference guide.

use std::sync::Once;
pub mod assertions;
pub mod builder;
pub mod chain;
pub mod constants;
pub mod run_config;
pub mod testing_utils;
pub mod utils;

pub use alloy;
pub use alloy_rlp;
pub use alloy_sol_types;
pub use basic_system;
pub use callable_oracles;
pub use chain::BlockContext;
pub use chain::Chain;
pub use ethers;
pub use forward_system;
pub use log;
pub use oracle_provider;
pub use risc_v_simulator;
pub use risc_v_simulator::sim::ProfilerConfig;
pub use ruint;
pub use zk_ee;
pub use zksync_os_api;
pub use zksync_os_interface;
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
