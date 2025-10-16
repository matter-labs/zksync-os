#![feature(allocator_api)]
#![allow(incomplete_features)]
#![feature(generic_const_exprs)]

use prover_examples::prover::VectorMemoryImplWithRom;
use risc_v_simulator::sim::BinarySource;
use risc_v_simulator::{
    abstractions::{memory::VectorMemoryImpl, non_determinism::NonDeterminismCSRSource},
    cycle::IMStandardIsaConfig,
    sim::{DiagnosticsConfig, ProfilerConfig, SimulatorConfig},
};
use std::{alloc::Global, io::Read, path::PathBuf, str::FromStr};

/// Runs the zksync_os binary on a simulator with a given non_determinism source for that many cycles.
/// If you enable diagnostics, it will print the flamegraph - but the run will be a lot slower.
pub fn run_default(
    cycles: usize,
    non_determinism_source: impl NonDeterminismCSRSource<VectorMemoryImpl>,
    enable_diagnostics: bool,
) -> [u32; 8] {
    run_default_with_flamegraph_path(
        cycles,
        non_determinism_source,
        if enable_diagnostics {
            Some(std::env::current_dir().unwrap().join("flamegraph.svg"))
        } else {
            None
        },
    )
}

pub fn run_default_with_flamegraph_path(
    cycles: usize,
    non_determinism_source: impl NonDeterminismCSRSource<VectorMemoryImpl>,
    diagnostics_path: Option<PathBuf>,
) -> [u32; 8] {
    let zksync_os_path =
        std::env::var("ZKSYNC_OS_DIR").unwrap_or_else(|_| String::from("../zksync_os"));
    let diag_config = diagnostics_path.map(|path| {
        let sym_path = PathBuf::from_str(&zksync_os_path).unwrap().join("app.elf");

        let mut d = DiagnosticsConfig::new(sym_path);

        d.profiler_config = {
            let mut p = ProfilerConfig::new(path);

            p.frequency_recip = 1;
            p.reverse_graph = false;

            Some(p)
        };

        d
    });
    run(
        PathBuf::from_str(&zksync_os_path).unwrap().join("app.bin"),
        diag_config,
        cycles,
        non_determinism_source,
    )
}

///
/// Runs zkOS on RISC-V (proof running) with given params:
/// `img_path` - path to ZKsync OS binary file (for now always in "zksync_os/app.bin")
/// `diagnostics` - optional diagnostics config, can be used to enable profiler.
/// `cycles` - limit for number of cycles.
/// `non_determinism_source` - non-determinism source used to read values from outside
///  (inside risc-v can be accessed via special system register read). In practice used to get all the block data - txs, metadata, storage values, etc.
///
/// Returns 256 bit program output. In real env this output will be exposed as proof public input.
///
pub fn run(
    img_path: PathBuf,
    diagnostics: Option<DiagnosticsConfig>,
    cycles: usize,
    non_determinism_source: impl NonDeterminismCSRSource<VectorMemoryImpl>,
) -> [u32; 8] {
    run_and_get_effective_cycles(img_path, diagnostics, cycles, non_determinism_source).0
}

pub fn run_and_get_effective_cycles(
    img_path: PathBuf,
    diagnostics: Option<DiagnosticsConfig>,
    cycles: usize,
    non_determinism_source: impl NonDeterminismCSRSource<VectorMemoryImpl>,
) -> ([u32; 8], Option<u64>) {
    println!("ZK RISC-V simulator is starting");

    // Check that the bin file is present and readable.
    let mut file = std::fs::File::open(img_path.clone())
        .unwrap_or_else(|_| panic!("ZKsync OS bin file missing: {img_path:?}"));
    let mut buffer = vec![];
    file.read_to_end(&mut buffer).expect("must read the file");

    let config = SimulatorConfig {
        bin: BinarySource::Path(img_path),
        cycles,
        entry_point: 0,
        diagnostics,
    };

    let run_result =
        risc_v_simulator::runner::run_simple_with_entry_point_and_non_determimism_source(
            config,
            non_determinism_source,
        );

    risc_v_simulator::cycle::state::output_opcode_stats();

    #[allow(unused_mut, unused_assignments)]
    let mut block_effective = None;

    #[cfg(feature = "cycle_marker")]
    {
        block_effective = cycle_marker::print_cycle_markers();
    }

    // our convention is to return 32 bytes placed into registers x10-x17

    // TODO: move to new simulator
    #[allow(deprecated)]
    (
        run_result.state.registers[10..18].try_into().unwrap(),
        block_effective,
    )
}

pub fn simulate_witness_tracing(
    img_path: PathBuf,
    non_determinism_source: impl NonDeterminismCSRSource<VectorMemoryImplWithRom>,
) {
    println!("ZK RISC-V simulator is starting");

    // Check that the bin file is present and readable.
    let mut file = std::fs::File::open(img_path.clone())
        .unwrap_or_else(|_| panic!("ZKsync OS bin file missing: {img_path:?}"));
    let mut buffer = vec![];
    file.read_to_end(&mut buffer).expect("must read the file");

    let num_instances_upper_bound = 1 << 14;
    let binary = execution_utils::get_padded_binary(&buffer);

    let worker = prover_examples::prover::worker::Worker::new();

    let now = std::time::Instant::now();
    let (all_witness_instances, _, _, _) =
        prover_examples::trace_execution_for_gpu::<_, IMStandardIsaConfig, Global>(
            num_instances_upper_bound,
            &binary,
            non_determinism_source,
            1 << 22,
            &worker,
        );
    let elapsed = now.elapsed();
    let cycles_upper_bound =
        all_witness_instances.len() * all_witness_instances[0].num_cycles_chunk_size;
    let speed = (cycles_upper_bound as f64) / elapsed.as_secs_f64() / 1_000_000f64;
    println!(
        "Simulator witness gen speed is roughly {speed} MHz: ran {cycles_upper_bound} cycles over {elapsed:?}"
    );
}

#[cfg(test)]
mod test {
    use super::*;
    use risc_v_simulator::abstractions::non_determinism::QuasiUARTSource;

    #[test]
    /// Quick test that uses the .bin file that computes the n-th fibonacci number.
    fn quick_runner() {
        let mut non_determinism_source = QuasiUARTSource::default();
        // Get 11th fibonacci number.
        non_determinism_source.oracle.push_back(11);
        let output = run(
            PathBuf::from_str("generated/dynamic_fibonacci.bin").unwrap(),
            None,
            1 << 25,
            non_determinism_source,
        );
        assert_eq!(output, [233u32, 11, 0, 0, 0, 0, 0, 0])
    }
}
