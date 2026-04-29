//! ZKsync OS RISC-V runner.
//!
//! TODO(airbender): once `TranspilerRunner` accepts a generic decoder
//! config (or exposes a MOP-aware variant), collapse this file to a
//! thin wrapper that just picks the right decoder and delegates.
use airbender_host::Program;
use common_constants::rom::ROM_SECOND_WORD_BITS;
use common_constants::{INITIAL_TIMESTAMP, TIMESTAMP_STEP};
use riscv_transpiler::abstractions::non_determinism::QuasiUARTSource;
use riscv_transpiler::cycle::CycleMarkerHooks;
use riscv_transpiler::ir::{
    preprocess_bytecode, DecodingOptions, FullUnsignedMachineDecoderConfig,
};
use riscv_transpiler::vm::{DelegationsCounters, RamWithRomRegion, SimpleTape, State, VM};
use std::fs;
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

/// Total RAM size (1 GiB address space).
const RAM_SIZE: usize = 1 << 30;

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

        // Use airbender-host to parse the manifest and verify sha256 of the
        // distributed artifacts, then run the VM ourselves with a MOP-aware
        // decoder (airbender-host's TranspilerRunner does not support MOP).
        let program = Program::load(&self.dist_dir).unwrap_or_else(|err| {
            panic!(
                "failed to load program from {}: {err}",
                self.dist_dir.display()
            )
        });

        let bin_words = read_u32_words(program.app_bin());
        let text_words = read_u32_words(program.app_text());

        let instructions = preprocess_bytecode::<FullUnsignedMachineWithMopDecoderConfig>(&text_words);
        let tape = SimpleTape::new(&instructions);
        let mut ram =
            RamWithRomRegion::<{ ROM_SECOND_WORD_BITS }>::from_rom_content(&bin_words, RAM_SIZE);
        let mut state = State::initial_with_counters(DelegationsCounters::default());
        let mut non_determinism_source = QuasiUARTSource::new_with_reads(input_words.to_vec());

        let cycle_markers = if let Some(fg_options) = self.flamegraph {
            use riscv_transpiler::vm::{FlamegraphConfig, VmFlamegraphProfiler};

            let fg_config = FlamegraphConfig {
                symbols_path: self.dist_dir.join("app.elf"),
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
                    self.cycles,
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
                    self.cycles,
                    &mut non_determinism_source,
                )
            });
            cm
        };

        #[allow(unused_mut, unused_assignments)]
        let mut block_effective = None;

        #[cfg(feature = "cycle_marker")]
        {
            let results = cycle_marker::print_cycle_markers(cycle_markers);
            block_effective = results.block_effective;
        }

        let cycles_executed = (state.timestamp - INITIAL_TIMESTAMP) / TIMESTAMP_STEP;

        // Our convention is to return 32 bytes placed into registers x10-x17.
        let output: [u32; 8] = core::array::from_fn(|i| state.registers[10 + i].value);

        RunResult {
            output,
            block_effective: block_effective.or(Some(cycles_executed)),
        }
    }
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
