use airbender_host::{FlamegraphConfig, Program, Runner};
use std::path::PathBuf;

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
    run_default_with_flamegraph_path(dist_dir, PathBuf::new(), cycles, input_words, None)
}

/// Run a ZKsync OS RISC-V program with optional flamegraph profiling.
///
/// `dist_dir` - path to the program distribution directory.
/// `sym_path` - path to the ELF symbols file (used for flamegraph).
/// `cycles` - limit for number of cycles.
/// `input_words` - pre-recorded non-determinism input words.
/// `flamegraph_path` - optional path to write flamegraph output.
pub fn run_default_with_flamegraph_path(
    dist_dir: PathBuf,
    sym_path: PathBuf,
    cycles: usize,
    input_words: &[u32],
    flamegraph_path: Option<PathBuf>,
) -> ([u32; 8], Option<u64>) {
    println!("ZK RISC-V transpiler runner is starting");

    let program = Program::load(&dist_dir)
        .unwrap_or_else(|err| panic!("failed to load program from {}: {err}", dist_dir.display()));

    let mut builder = program.transpiler_runner().with_cycles(cycles);

    if let Some(fg_path) = flamegraph_path {
        let flamegraph_config = FlamegraphConfig {
            output: fg_path,
            sampling_rate: 1,
            inverse: false,
            elf_path: if sym_path.as_os_str().is_empty() {
                None
            } else {
                Some(sym_path)
            },
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
