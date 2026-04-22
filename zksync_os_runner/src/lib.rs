use airbender_host::{FlamegraphConfig, Program, Runner as _};
use std::path::PathBuf;

/// Default upper bound on RISC-V cycles used when the caller doesn't override it.
/// This is large enough for any real block; individual tests can lower it.
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
    sym_path: Option<PathBuf>,
}

impl Runner {
    pub fn new(dist_dir: PathBuf) -> Self {
        Self {
            dist_dir,
            cycles: DEFAULT_CYCLE_LIMIT,
            flamegraph: None,
            sym_path: None,
        }
    }

    pub fn with_cycles(mut self, cycles: usize) -> Self {
        self.cycles = cycles;
        self
    }

    /// Enable flamegraph profiling. `sym_path` is the path to the ELF symbols
    /// file used to resolve stack frames; if `None`, frames are raw addresses.
    pub fn with_flamegraph(
        mut self,
        options: FlamegraphOptions,
        sym_path: Option<PathBuf>,
    ) -> Self {
        self.flamegraph = Some(options);
        self.sym_path = sym_path;
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

        let mut builder = program.transpiler_runner().with_cycles(self.cycles);

        if let Some(fg_options) = self.flamegraph {
            let flamegraph_config = FlamegraphConfig {
                output: fg_options.output_path,
                sampling_rate: fg_options.frequency_recip,
                inverse: false,
                elf_path: self.sym_path,
            };
            builder = builder.with_flamegraph(flamegraph_config);
        }

        let runner = builder
            .build()
            .unwrap_or_else(|err| panic!("failed to build transpiler runner: {err}"));

        let result = runner
            .run(input_words)
            .unwrap_or_else(|err| panic!("transpiler runner execution failed: {err}"));

        #[allow(unused_mut, unused_assignments)]
        let mut block_effective = None;

        #[cfg(feature = "cycle_marker")]
        {
            if let Some(cm) = result.cycle_markers {
                let results = cycle_marker::print_cycle_markers(cm);
                block_effective = results.block_effective;
            }
        }

        RunResult {
            output: result.receipt.output,
            block_effective,
        }
    }
}
