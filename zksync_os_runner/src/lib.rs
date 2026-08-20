//! ZKsync OS RISC-V runner.
//!
//! Wrapper around airbender-host's `TranspilerRunner` that plugs
//! in our MOP-aware decoder.

use airbender_host::{ExecutionResult, FlamegraphConfig, Program, Runner as _};
use riscv_transpiler::ir::{DecodingOptions, FullUnsignedMachineDecoderConfig};
use std::path::PathBuf;

/// Decoder config used by the FRI-aware RISC-V runner.
struct FullUnsignedMachineWithMopDecoderConfig;

impl DecodingOptions for FullUnsignedMachineWithMopDecoderConfig {
    const SUPPORT_MOP: bool = true;
    const SUPPORT_MUL_DIV: bool =
        <FullUnsignedMachineDecoderConfig as DecodingOptions>::SUPPORT_MUL_DIV;
    const SUPPORT_SIGNED_MUL_DIV: bool =
        <FullUnsignedMachineDecoderConfig as DecodingOptions>::SUPPORT_SIGNED_MUL_DIV;
    const SUPPORT_SUBWORD_MEM_ACCESS: bool =
        <FullUnsignedMachineDecoderConfig as DecodingOptions>::SUPPORT_SUBWORD_MEM_ACCESS;
}

/// Default upper bound on RISC-V cycles used when the caller doesn't override it.
pub const DEFAULT_CYCLE_LIMIT: usize = 1 << 36;

/// Flamegraph profiling options passed through to the transpiler VM.
#[derive(Clone)]
pub struct FlamegraphOptions {
    /// Path to write the flamegraph SVG.
    pub output_path: PathBuf,
    /// Collect one sample every `frequency_recip` VM cycles.
    /// Lower values give more detail but add runtime overhead.
    /// Defaults to 1 (sample every cycle).
    pub frequency_recip: usize,
}

impl FlamegraphOptions {
    pub fn new(output_path: PathBuf) -> Self {
        Self {
            output_path,
            frequency_recip: 1,
        }
    }
}

/// Result of running a ZKsync OS RISC-V program.
#[derive(Clone, Debug)]
pub struct RunResult {
    /// 256-bit program output (registers x10-x17 at exit).
    pub output: [u32; 8],
    /// Effective cycle count for the `process_block` marker when the
    /// `cycle_marker` feature is enabled and the program wrote markers;
    /// `None` otherwise.
    pub block_effective: Option<u64>,
}

/// Builder for running a ZKsync OS RISC-V program against an airbender-host
/// `TranspilerRunner`.
pub struct Runner {
    dist_dir: PathBuf,
    cycles: usize,
    flamegraph: Option<FlamegraphOptions>,
}

impl Runner {
    pub fn new(dist_dir: PathBuf) -> Self {
        Self {
            dist_dir,
            cycles: DEFAULT_CYCLE_LIMIT,
            flamegraph: None,
        }
    }

    pub fn with_cycles(mut self, cycles: usize) -> Self {
        self.cycles = cycles;
        self
    }

    /// Enable flamegraph profiling. Stack frames are resolved against
    /// `<dist_dir>/app.elf` (always produced by `cargo airbender build`).
    pub fn with_flamegraph(mut self, options: FlamegraphOptions) -> Self {
        self.flamegraph = Some(options);
        self
    }

    /// Execute the program with the configured options.
    pub fn run(self, input_words: &[u32]) -> RunResult {
        log::info!("ZK RISC-V transpiler runner is starting");

        let program = Program::load(&self.dist_dir).unwrap_or_else(|err| {
            panic!(
                "failed to load program from {}: {err}",
                self.dist_dir.display()
            )
        });

        let mut builder = program
            .transpiler_runner()
            .with_unstable_raw_decoder::<FullUnsignedMachineWithMopDecoderConfig>(
                "zksync-os mop decoder",
            )
            .with_cycles(self.cycles);

        if let Some(fg) = self.flamegraph {
            builder = builder.with_flamegraph(FlamegraphConfig {
                output: fg.output_path,
                sampling_rate: fg.frequency_recip,
                inverse: false,
                elf_path: Some(self.dist_dir.join("app.elf")),
            });
        }

        let runner = builder.build().expect("failed to build transpiler runner");
        let ExecutionResult {
            receipt,
            cycles_executed,
            cycle_markers,
            ..
        } = runner
            .run(input_words)
            .expect("transpiler runner execution failed");

        #[allow(unused_mut, unused_assignments)]
        let mut block_effective = None;

        #[cfg(feature = "cycle_marker")]
        if let Some(cm) = cycle_markers {
            let results = cycle_marker::print_cycle_markers(cm);
            block_effective = results.block_effective;
        }

        #[cfg(not(feature = "cycle_marker"))]
        let _ = cycle_markers;

        RunResult {
            output: receipt.output,
            block_effective: block_effective.or(Some(cycles_executed as u64)),
        }
    }
}
