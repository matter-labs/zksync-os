//! Preset [`RunConfig`] constructors for common testing scenarios.

use crate::chain::RunConfig;
use std::path::PathBuf;

/// Forward-only run — fastest option, no RISC-V simulation.
pub fn forward_only() -> RunConfig {
    RunConfig::without_riscv_run()
}

/// Run using the current test binary setup with RISC-V simulation enabled.
pub fn with_riscv_simulation() -> RunConfig {
    RunConfig::with_riscv_run()
}

/// RISC-V simulation run that also writes a flamegraph SVG to `path`.
pub fn with_profiler(path: impl Into<PathBuf>) -> RunConfig {
    use crate::ProfilerConfig;

    let mut config = with_riscv_simulation();
    let mut profiler_config = ProfilerConfig::new(path.into());
    // Keep sampling aligned with existing run_block_generate_witness defaults.
    profiler_config.frequency_recip = 10;
    config.profiler_config = Some(profiler_config);
    config
}

/// RISC-V simulation run that saves the witness to a file.
pub fn with_witness_dump(path: impl Into<PathBuf>) -> RunConfig {
    let mut config = with_riscv_simulation();
    config.witness_output_file = Some(path.into());
    config
}
