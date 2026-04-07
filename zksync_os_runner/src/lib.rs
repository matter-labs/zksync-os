use airbender_host::{FlamegraphConfig, Program, Runner};
use std::path::PathBuf;

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

/// Run a ZKsync OS RISC-V program and return the 256-bit output.
///
/// `dist_dir` - path to the program distribution directory (containing manifest.toml and artifacts).
/// `cycles` - limit for number of cycles.
/// `input_words` - pre-recorded non-determinism input words.
///
/// Returns 256 bit program output as `[u32; 8]`.
pub fn run(dist_dir: PathBuf, cycles: usize, input_words: &[u32]) -> [u32; 8] {
    run_and_get_effective_cycles(dist_dir, cycles, input_words).0
}

/// Run a ZKsync OS RISC-V program and return both the output and optional effective cycle count.
pub fn run_and_get_effective_cycles(
    dist_dir: PathBuf,
    cycles: usize,
    input_words: &[u32],
) -> ([u32; 8], Option<u64>) {
    run_inner(dist_dir, cycles, input_words, None, None)
}

pub fn run_with_flamegraph(
    dist_dir: PathBuf,
    sym_path: PathBuf,
    cycles: usize,
    input_words: &[u32],
    options: FlamegraphOptions,
) -> ([u32; 8], Option<u64>) {
    run_inner(dist_dir, cycles, input_words, Some(sym_path), Some(options))
}

fn run_inner(
    dist_dir: PathBuf,
    cycles: usize,
    input_words: &[u32],
    sym_path: Option<PathBuf>,
    flamegraph: Option<FlamegraphOptions>,
) -> ([u32; 8], Option<u64>) {
    eprintln!("ZK RISC-V transpiler runner is starting");

    let program = Program::load(&dist_dir)
        .unwrap_or_else(|err| panic!("failed to load program from {}: {err}", dist_dir.display()));

    let mut builder = program.transpiler_runner().with_cycles(cycles);

    if let Some(fg_options) = flamegraph {
        let flamegraph_config = FlamegraphConfig {
            output: fg_options.output_path,
            sampling_rate: fg_options.frequency_recip,
            inverse: false,
            elf_path: sym_path,
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

    (result.receipt.output, block_effective)
}
