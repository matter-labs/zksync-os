//! ZKsync OS RISC-V runner.
//!
//! Wraps the `riscv_transpiler` VM with a decoder configured for the
//! ZKsync OS binary's instruction set. In particular the `zksync_os`
//! binary is built with `+zimop`, which means `full_statement_verifier`
//! (compiled with `modular_ops`) emits `mop.rr.*` instructions for
//! modular arithmetic in the FRI verifier. That forces us to enable
//! `SUPPORT_MOP` in the decoder — `airbender_host::TranspilerRunner`
//! hardcodes `FullUnsignedMachineDecoderConfig` (no MOP), so we run the
//! VM directly here instead of going through it. We still use
//! `airbender_host::Program::load()` for manifest parsing and sha256
//! verification of the distributed artifacts.
use airbender_host::Program;
use common_constants::rom::ROM_SECOND_WORD_BITS;
use common_constants::{INITIAL_TIMESTAMP, TIMESTAMP_STEP};
use riscv_transpiler::abstractions::non_determinism::QuasiUARTSource;
use riscv_transpiler::cycle::CycleMarkerHooks;
use riscv_transpiler::ir::{preprocess_bytecode, DecodingOptions};
use riscv_transpiler::vm::{DelegationsCounters, RamWithRomRegion, SimpleTape, State, VM};
use std::fs;
use std::path::PathBuf;

/// Decoder config used by the FRI-aware RISC-V runner.
///
/// Matches `FullUnsignedMachineDecoderConfig` but with `SUPPORT_MOP`
/// enabled so the `mop.rr.*` instructions emitted by
/// `full_statement_verifier` (compiled with `modular_ops`) decode
/// correctly.
struct FullUnsignedMachineWithMopDecoderConfig;

impl DecodingOptions for FullUnsignedMachineWithMopDecoderConfig {
    const SUPPORT_MOP: bool = true;
    const SUPPORT_MUL_DIV: bool = true;
    const SUPPORT_SIGNED_MUL_DIV: bool = false;
    const SUPPORT_SUBWORD_MEM_ACCESS: bool = true;
}

/// Total RAM size (1 GiB address space).
const RAM_SIZE: usize = 1 << 30;

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
    log::info!("ZK RISC-V transpiler runner is starting");

    // Use airbender-host to parse the manifest and verify sha256 of the
    // distributed artifacts, then run the VM ourselves with a MOP-aware
    // decoder (airbender-host's TranspilerRunner does not support MOP).
    let program = Program::load(&dist_dir)
        .unwrap_or_else(|err| panic!("failed to load program from {}: {err}", dist_dir.display()));

    let bin_words = read_u32_words(program.app_bin());
    let text_words = read_u32_words(program.app_text());

    let instructions = preprocess_bytecode::<FullUnsignedMachineWithMopDecoderConfig>(&text_words);
    let tape = SimpleTape::new(&instructions);
    let mut ram =
        RamWithRomRegion::<{ ROM_SECOND_WORD_BITS }>::from_rom_content(&bin_words, RAM_SIZE);
    let mut state = State::initial_with_counters(DelegationsCounters::default());
    let mut non_determinism_source = QuasiUARTSource::new_with_reads(input_words.to_vec());

    let _cycle_markers = if let Some(fg_options) = flamegraph {
        use riscv_transpiler::vm::{FlamegraphConfig, VmFlamegraphProfiler};

        let symbols_path = sym_path.expect("flamegraph requires a symbols (ELF) path");
        let fg_config = FlamegraphConfig {
            symbols_path,
            output_path: fg_options.output_path,
            reverse_graph: false,
            frequency_recip: fg_options.frequency_recip,
        };
        let mut profiler =
            VmFlamegraphProfiler::new(fg_config).expect("failed to initialize flamegraph profiler");
        let (result, cm) = CycleMarkerHooks::with(|| {
            VM::<DelegationsCounters, CycleMarkerHooks>::run_basic_unrolled_with_flamegraph::<_, _, _>(
                &mut state,
                &mut ram,
                &mut (),
                &tape,
                cycles,
                &mut non_determinism_source,
                &mut profiler,
            )
        });
        result.expect("flamegraph execution failed");
        cm
    } else {
        let (_reached_end, cm) = CycleMarkerHooks::with(|| {
            VM::<DelegationsCounters, CycleMarkerHooks>::run_basic_unrolled::<_, _, _>(
                &mut state,
                &mut ram,
                &mut (),
                &tape,
                cycles,
                &mut non_determinism_source,
            )
        });
        cm
    };

    #[allow(unused_mut, unused_assignments)]
    let mut block_effective = None;

    #[cfg(feature = "cycle_marker")]
    {
        let results = cycle_marker::print_cycle_markers(_cycle_markers);
        block_effective = results.block_effective;
    }

    let cycles_executed = (state.timestamp - INITIAL_TIMESTAMP) / TIMESTAMP_STEP;

    // Our convention is to return 32 bytes placed into registers x10-x17.
    let output: [u32; 8] = core::array::from_fn(|i| state.registers[10 + i].value);

    (output, block_effective.or(Some(cycles_executed)))
}

fn read_u32_words(path: &std::path::Path) -> Vec<u32> {
    let bytes =
        fs::read(path).unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
    assert!(
        bytes.len().is_multiple_of(4),
        "{} is not word-aligned: {} bytes",
        path.display(),
        bytes.len()
    );
    bytes
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}
