//! Preset [`RunConfig`] constructors for common testing scenarios.
//!
//! Instead of constructing `RunConfig { app: Some("for_tests".into()), ... }` by hand in every
//! test, use one of the named presets below.
//!
//! # Examples
//!
//! ```rust,ignore
//! use rig::run_config;
//!
//! // Forward-only run — fast, no proof verification.
//! let output = chain.run_block(txs, None, None, Some(run_config::forward_only()));
//!
//! // Full proof run — slower, verifies storage-diff hashes.
//! let output = chain.run_block(txs, None, None, Some(run_config::full_proof()));
//! ```

use crate::chain::RunConfig;
use std::path::PathBuf;

/// Forward-only run — fastest option, no RISC-V simulation or proof verification.
///
/// Use this when you need quick iteration and don't care about proof correctness.
pub fn forward_only() -> RunConfig {
    RunConfig {
        only_forward: true,
        ..Default::default()
    }
}

/// Full proof run using the `for_tests` binary.
///
/// Runs both the forward pass and the RISC-V proof simulation and verifies that storage-diff
/// hashes match between the two runs. This is the most thorough option and the right choice for
/// correctness tests.
pub fn full_proof() -> RunConfig {
    RunConfig {
        app: Some("for_tests".to_string()),
        only_forward: false,
        check_storage_diff_hashes: true,
        ..Default::default()
    }
}

/// Full proof run that also writes a flamegraph SVG to `path`.
///
/// Use when profiling execution to find hot spots.
pub fn with_profiler(path: impl Into<PathBuf>) -> RunConfig {
    use crate::ProfilerConfig;
    let mut pc = ProfilerConfig::new(path.into());
    pc.frequency_recip = 1;
    RunConfig {
        app: Some("for_tests".to_string()),
        only_forward: false,
        check_storage_diff_hashes: true,
        profiler_config: Some(pc),
        ..Default::default()
    }
}

/// Run that saves the full witness to a file for later inspection or replay.
pub fn with_witness_dump(path: impl Into<PathBuf>) -> RunConfig {
    RunConfig {
        app: Some("for_tests".to_string()),
        only_forward: false,
        witness_output_file: Some(path.into()),
        ..Default::default()
    }
}
