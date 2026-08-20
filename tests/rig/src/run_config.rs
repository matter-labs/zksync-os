//! Preset [`RunConfig`] constructors for common testing scenarios.

use crate::chain::RunConfig;
use std::path::PathBuf;

/// Forward-only run with no RISC-V simulation.
///
/// This is an escape hatch and should only be used for tests that cannot run
/// under the RISC-V path at all. Tests should normally rely on the default rig
/// behavior so RISC-V checks still run when the environment enables them.
pub fn forward_only() -> RunConfig {
    RunConfig::without_riscv_run()
}

/// Run using the current test binary setup with RISC-V simulation enabled.
pub fn with_riscv_simulation() -> RunConfig {
    RunConfig::with_riscv_run()
}

/// RISC-V simulation run that also writes a flamegraph SVG to `path`.
pub fn with_profiler(path: impl Into<PathBuf>) -> RunConfig {
    let mut config = with_riscv_simulation();
    config.flamegraph = Some(crate::FlamegraphOptions::new(path.into()));
    config
}

/// RISC-V simulation run that saves the witness to a file.
pub fn with_witness_dump(path: impl Into<PathBuf>) -> RunConfig {
    let mut config = with_riscv_simulation();
    config.witness_output_file = Some(path.into());
    config
}
