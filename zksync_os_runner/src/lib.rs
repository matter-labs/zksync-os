#![feature(allocator_api)]
#![allow(incomplete_features)]

use common_constants::rom::ROM_SECOND_WORD_BITS;
use common_constants::{INITIAL_TIMESTAMP, TIMESTAMP_STEP};
use riscv_transpiler::ir::{preprocess_bytecode, FullUnsignedMachineDecoderConfig};
use riscv_transpiler::vm::{
    DelegationsCounters, NonDeterminismCSRSource, RamWithRomRegion, SimpleTape, State, VM,
};
use std::io::Read;
use std::path::{Path, PathBuf};

/// Total RAM size (1 GiB address space).
const RAM_SIZE: usize = 1 << 30;

///
/// Runs zkOS on RISC-V (proof running) with given params:
/// `img_path` - path to ZKsync OS binary file (for example "zksync_os/for_tests.bin")
/// `cycles` - limit for number of cycles.
/// `non_determinism_source` - non-determinism source used to read values from outside
///  (inside risc-v can be accessed via special system register read). In practice used to get all the block data - txs, metadata, storage values, etc.
///
/// Returns 256 bit program output. In real env this output will be exposed as proof public input.
///
pub fn run(
    img_path: PathBuf,
    cycles: usize,
    non_determinism_source: impl NonDeterminismCSRSource,
) -> [u32; 8] {
    run_and_get_effective_cycles(img_path, cycles, non_determinism_source).0
}

pub fn run_and_get_effective_cycles_from_bytes(
    img_bytes: &[u8],
    text_bytes: &[u8],
    cycles: usize,
    non_determinism_source: impl NonDeterminismCSRSource,
) -> ([u32; 8], Option<u64>) {
    let bin_words = bytes_to_u32_words(img_bytes);
    let text_words = bytes_to_u32_words(text_bytes);
    run_inner(&bin_words, &text_words, cycles, non_determinism_source)
}

pub fn run_and_get_effective_cycles(
    img_path: PathBuf,
    cycles: usize,
    non_determinism_source: impl NonDeterminismCSRSource,
) -> ([u32; 8], Option<u64>) {
    let text_path = img_path.with_extension("text");
    let (bin_words, text_words) = load_bin_and_text(&img_path, &text_path);
    run_inner(&bin_words, &text_words, cycles, non_determinism_source)
}

fn run_inner(
    bin_words: &[u32],
    text_words: &[u32],
    cycles: usize,
    mut non_determinism_source: impl NonDeterminismCSRSource,
) -> ([u32; 8], Option<u64>) {
    println!("ZK RISC-V transpiler is starting");

    let instructions = preprocess_bytecode::<FullUnsignedMachineDecoderConfig>(text_words);
    let tape = SimpleTape::new(&instructions);
    let mut ram =
        RamWithRomRegion::<{ ROM_SECOND_WORD_BITS }>::from_rom_content(bin_words, RAM_SIZE);
    let mut state = State::initial_with_counters(DelegationsCounters::default());

    #[allow(unused_mut, unused_assignments)]
    let mut block_effective = None;

    #[cfg(feature = "cycle_marker")]
    {
        use cycle_marker::CycleMarkerHooks;

        let (_reached_end, cycle_markers) = CycleMarkerHooks::with(|| {
            VM::<DelegationsCounters, CycleMarkerHooks>::run_basic_unrolled::<_, _, _>(
                &mut state,
                &mut ram,
                &mut (),
                &tape,
                cycles,
                &mut non_determinism_source,
            )
        });
        let results = cycle_marker::print_cycle_markers(cycle_markers);
        block_effective = results.block_effective;
    }

    #[cfg(not(feature = "cycle_marker"))]
    {
        let _reached_end = VM::<DelegationsCounters>::run_basic_unrolled::<_, _, _>(
            &mut state,
            &mut ram,
            &mut (),
            &tape,
            cycles,
            &mut non_determinism_source,
        );
    }

    let cycles_executed = (state.timestamp - INITIAL_TIMESTAMP) / TIMESTAMP_STEP;

    // our convention is to return 32 bytes placed into registers x10-x17
    let output: [u32; 8] = std::array::from_fn(|i| state.registers[10 + i].value);

    (output, block_effective.or(Some(cycles_executed)))
}

#[cfg(feature = "flamegraph")]
pub fn run_with_flamegraph(
    img_path: PathBuf,
    sym_path: PathBuf,
    cycles: usize,
    mut non_determinism_source: impl NonDeterminismCSRSource,
    output_path: PathBuf,
) -> ([u32; 8], Option<u64>) {
    use riscv_transpiler::vm::{FlamegraphConfig, VmFlamegraphProfiler};

    let text_path = img_path.with_extension("text");
    let (bin_words, text_words) = load_bin_and_text(&img_path, &text_path);

    let instructions = preprocess_bytecode::<FullUnsignedMachineDecoderConfig>(&text_words);
    let tape = SimpleTape::new(&instructions);
    let mut ram =
        RamWithRomRegion::<{ ROM_SECOND_WORD_BITS }>::from_rom_content(&bin_words, RAM_SIZE);
    let mut state = State::initial_with_counters(DelegationsCounters::default());

    let mut config = FlamegraphConfig::new(sym_path, output_path);
    config.frequency_recip = 1;
    config.reverse_graph = false;
    let mut profiler =
        VmFlamegraphProfiler::new(config).expect("failed to initialize flamegraph profiler");

    let _reached_end = VM::<DelegationsCounters>::run_basic_unrolled_with_flamegraph::<_, _, _>(
        &mut state,
        &mut ram,
        &mut (),
        &tape,
        cycles,
        &mut non_determinism_source,
        &mut profiler,
    )
    .expect("flamegraph execution failed");

    let cycles_executed = (state.timestamp - INITIAL_TIMESTAMP) / TIMESTAMP_STEP;
    let output: [u32; 8] = std::array::from_fn(|i| state.registers[10 + i].value);
    (output, Some(cycles_executed))
}

pub fn simulate_witness_tracing(
    img_path: PathBuf,
    mut non_determinism_source: impl NonDeterminismCSRSource,
) {
    println!("ZK RISC-V transpiler witness tracing is starting");

    let text_path = img_path.with_extension("text");
    let (bin_words, text_words) = load_bin_and_text(&img_path, &text_path);

    let instructions = preprocess_bytecode::<FullUnsignedMachineDecoderConfig>(&text_words);
    let tape = SimpleTape::new(&instructions);
    let mut ram =
        RamWithRomRegion::<{ ROM_SECOND_WORD_BITS }>::from_rom_content(&bin_words, RAM_SIZE);
    let mut state = State::initial_with_counters(DelegationsCounters::default());

    // Run with a snapshotter to trace witness generation, measuring throughput.
    let cycles_upper_bound = 1 << 24;
    let mut snapshotter = riscv_transpiler::vm::SimpleSnapshotter::<
        DelegationsCounters,
        { ROM_SECOND_WORD_BITS },
    >::new_with_cycle_limit(cycles_upper_bound, state);

    let now = std::time::Instant::now();
    let _reached_end = VM::<DelegationsCounters>::run_basic_unrolled::<_, _, _>(
        &mut state,
        &mut ram,
        &mut snapshotter,
        &tape,
        cycles_upper_bound,
        &mut non_determinism_source,
    );
    let elapsed = now.elapsed();

    let cycles_executed = ((state.timestamp - INITIAL_TIMESTAMP) / TIMESTAMP_STEP) as usize;
    let speed = (cycles_executed as f64) / elapsed.as_secs_f64() / 1_000_000f64;
    let num_snapshots = snapshotter.snapshots.len();
    println!(
        "Witness gen speed is roughly {speed:.1} MHz: ran {cycles_executed} cycles ({num_snapshots} snapshots) over {elapsed:?}"
    );
}

// -- Helpers --

fn load_bin_and_text(bin_path: &Path, text_path: &Path) -> (Vec<u32>, Vec<u32>) {
    let bin_words = read_file_as_u32_words(bin_path);
    let text_words = read_file_as_u32_words(text_path);
    (bin_words, text_words)
}

fn read_file_as_u32_words(path: &Path) -> Vec<u32> {
    let mut file =
        std::fs::File::open(path).unwrap_or_else(|_| panic!("file missing: {}", path.display()));
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .unwrap_or_else(|_| panic!("failed to read: {}", path.display()));
    bytes_to_u32_words(&bytes)
}

fn bytes_to_u32_words(bytes: &[u8]) -> Vec<u32> {
    assert!(
        bytes.len().is_multiple_of(4),
        "binary length {} is not a multiple of 4",
        bytes.len()
    );
    bytes
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))
        .collect()
}

#[cfg(test)]
mod test {
    use super::*;
    use riscv_transpiler::abstractions::non_determinism::QuasiUARTSource;
    use std::str::FromStr;

    #[test]
    /// Quick test that uses the .bin file that computes the n-th fibonacci number.
    fn quick_runner() {
        let bin_path = PathBuf::from_str("generated/dynamic_fibonacci.bin").unwrap();
        let text_path = bin_path.with_extension("text");
        if !bin_path.exists() || !text_path.exists() {
            eprintln!("skipping quick_runner: generated binary/text not found");
            return;
        }
        let mut non_determinism_source = QuasiUARTSource::default();
        // Get 11th fibonacci number.
        non_determinism_source.oracle.push_back(11);
        let output = run(bin_path, 1 << 25, non_determinism_source);
        assert_eq!(output, [233u32, 11, 0, 0, 0, 0, 0, 0])
    }
}
