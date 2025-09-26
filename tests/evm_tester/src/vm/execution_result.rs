//!
//! The VM execution result.
//!

use super::output::ExecutionOutput;
use alloy::primitives::*;

///
/// The VM execution result.
///
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// The VM snapshot execution result.
    pub output: ExecutionOutput,
    /// The number of executed cycles.
    pub cycles: usize,
    /// The number of EraVM ergs used.
    pub ergs: u64,
    /// The number of gas used.
    pub gas: U256,
}

impl ExecutionResult {
    ///
    /// A shortcut constructor.
    ///
    pub fn new(output: ExecutionOutput, cycles: usize, ergs: u64, gas: U256) -> Self {
        Self {
            output,
            cycles,
            ergs,
            gas,
        }
    }
}
