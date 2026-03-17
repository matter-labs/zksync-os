#![feature(allocator_api)]

/// Runs the batch, and returns the output (that contains gas usage, transaction status etc.).
pub use forward_system::run::{generate_proof_input, run_block};
pub mod helpers;
