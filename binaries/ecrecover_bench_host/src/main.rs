//! The host of the `ecrecover_bench` guest: runs a dist of it on the RISC-V
//! transpiler VM (the interpreter, which keeps the cycle markers and the
//! delegation counters), serves the guest's hint requests over the CSR
//! channel, checks the guest's output and reports the cycles and the
//! delegation calls of every recovery (the guest puts a cycle marker before
//! and after each).
//!
//! usage: ecrecover_bench_host <dist dir>... [--repeats N] [--hints on|off|both|witness|all] [--machine unsigned|full]
//!        [--flamegraph <svg> [--sampling N]] [--micro]
//!
//! `--micro` runs the guest's field-operation microbenchmark instead (the
//! cycles and delegation calls of every field operation, `MICRO_OPS` of each
//! between two markers).
//!
//! The hints are the ZKsync OS oracle protocol of the crate's CSR hooks
//! (`crypto::secp256k1::oracle`): a query is the query id and the address of
//! a request in the guest's memory, which the host reads with the operand
//! (`oracle::host::serve`), in the representation of the dist (the Montgomery
//! words of the delegated elements for a `bigint` variant, canonical words
//! otherwise, from the features of its manifest). `--hints witness` answers
//! the witness requests, `--hints on` declines them (the key is computed).
//! `--flamegraph` profiles the run (one stack sample every `--sampling`
//! cycles, 100 by default) against the dist's `app.elf`. With `BENCH_TRACE`
//! set in the environment, every query (its id, the address of the request and
//! the first words there) is printed to stderr.
//!
//! `--machine unsigned` (the default) decodes for the full unsigned machine,
//! the production profile; `full` also executes the signed multiplications and
//! divisions (`mulh`, `div`...), which the compiler emits in the software
//! modular inversion of the no-hint path. The cycle count of an instruction
//! is the same under both.

use anyhow::Context;
use crypto::secp256k1::hooks::DefaultSecp256k1Hooks;
use crypto::secp256k1::oracle::host::{answer, read_request, QuerierMemory, Representation};
use crypto::secp256k1::oracle::{FieldHintOp, FIELD_OPS_ADVISE_QUERY_ID};
use crypto::secp256k1::recover_from_bytes_with_hooks;
use crypto::sha3::Keccak256;
use crypto::MiniDigest;
use field::baby_bear::base::BabyBearField;
use riscv_transpiler::abstractions::non_determinism::QuasiUARTSourceState;
use riscv_transpiler::common_constants::{
    rom::ROM_SECOND_WORD_BITS, INITIAL_TIMESTAMP, TIMESTAMP_STEP,
};
use riscv_transpiler::cycle::{CycleMarkerHooks, Mark};
use riscv_transpiler::ir::simple_instruction_set::preprocess_bytecode;
use riscv_transpiler::ir::{
    DecodingOptions, FullMachineDecoderConfig, FullUnsignedMachineDecoderConfig,
};
use riscv_transpiler::vm::{
    DelegationsCounters, FlamegraphConfig, NonDeterminismCSRSource, RamPeek, RamWithRomRegion,
    SimpleTape, State, VmFlamegraphProfiler, VM,
};
use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};

/// The guest's vector (geth's `ecrecover` test): the digest, `r`, `s` of a
/// signature with `v = 27`, and the address it recovers
const DIGEST: [u8; 32] = [
    0x38, 0xd1, 0x8a, 0xcb, 0x67, 0xd2, 0x5c, 0x8b, 0xb9, 0x94, 0x27, 0x64, 0xb6, 0x2f, 0x18, 0xe1,
    0x70, 0x54, 0xf6, 0x6a, 0x81, 0x7b, 0xd4, 0x29, 0x54, 0x23, 0xad, 0xf9, 0xed, 0x98, 0x87, 0x3e,
];
const R: [u8; 32] = DIGEST;
const S: [u8; 32] = [
    0x78, 0x9d, 0x1d, 0xd4, 0x23, 0xd2, 0x5f, 0x07, 0x72, 0xd2, 0x74, 0x8d, 0x60, 0xf7, 0xe4, 0xb8,
    0x1b, 0xb1, 0x4d, 0x08, 0x6e, 0xba, 0x8e, 0x8e, 0x8e, 0xfb, 0x6d, 0xcf, 0xf8, 0xa4, 0xae, 0x02,
];
const Y_IS_ODD: bool = false;
const ADDRESS: [u8; 32] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xce, 0xac, 0xca, 0xc6,
    0x40, 0xad, 0xf5, 0x5b, 0x20, 0x28, 0x46, 0x9b, 0xd3, 0x6b, 0xa5, 0x01, 0xf2, 0x8b, 0x69, 0x9d,
];

/// The guest's memory layout (`ram-1gb`)
const RAM_BYTES: usize = 1 << 30;
const CYCLE_BOUND: usize = 1 << 36;

/// The run modes: the guest's modes 0 (no hints), 1 (hints) and 2 (the field
/// microbenchmark), and 3: the hints with the witnessed key (the guest's mode
/// 1, the host answering the witness requests)
const MODE_NAMES: [&str; 4] = ["hints off", "hints on ", "micro", "witnessed"];
const MODE_HINTS: u32 = 1;
const MODE_MICRO: u32 = 2;
const MODE_WITNESSED: u32 = 3;

/// The guest's RAM as the memory of the querier
struct GuestRam<'a, M: RamPeek + ?Sized>(&'a M);

impl<M: RamPeek + ?Sized> QuerierMemory for GuestRam<'_, M> {
    fn word_size(&self) -> usize {
        4
    }

    fn read_u32(&self, address: usize) -> u32 {
        let address = u32::try_from(address).expect("a guest address");
        assert!(address.is_multiple_of(4), "a word address");
        self.0.peek_word(address)
    }
}

/// The CSR source of a run: the settings first (the mode, the repeats), then
/// the answers of the hint queries the guest sends
struct BenchSource {
    outbound: VecDeque<u32>,
    /// The id of the query whose second word, the address of the request, is next
    pending_query: Option<u32>,
    /// The representation of the elements of the dist
    representation: Representation,
    /// Whether the witness requests are answered
    witness: bool,
    hints_served: usize,
    /// The hint requests by op
    hints_by_op: BTreeMap<u32, usize>,
    /// The longest witnessed coefficient so far, in bits
    max_coefficient_bits: u32,
    /// The guest's prints (a panic message, for one), as the reference source decodes them
    uart: QuasiUARTSourceState,
}

impl BenchSource {
    fn new(mode: u32, repeats: u32, representation: Representation) -> Self {
        let guest_mode = if mode == MODE_WITNESSED { MODE_HINTS } else { mode };
        Self {
            outbound: VecDeque::from([guest_mode, repeats]),
            pending_query: None,
            representation,
            witness: mode == MODE_WITNESSED,
            hints_served: 0,
            hints_by_op: BTreeMap::new(),
            max_coefficient_bits: 0,
            uart: QuasiUARTSourceState::Ready,
        }
    }

    /// A word the guest writes: the UART protocol while a print is on, else the query id
    /// or the address of the request of a query, which is answered
    fn write(&mut self, ram: &dyn RamPeek, value: u32) {
        const UART_HELLO: u32 = 0xffff_ffff;
        if !matches!(self.uart, QuasiUARTSourceState::Ready)
            || (self.pending_query.is_none() && value == UART_HELLO)
        {
            self.uart.process_write(value);
            return;
        }
        let Some(query_id) = self.pending_query.take() else {
            if std::env::var_os("BENCH_TRACE").is_some() {
                eprintln!("query id 0x{value:08x}");
            }
            self.pending_query = Some(value);
            return;
        };
        if std::env::var_os("BENCH_TRACE").is_some() {
            let words: Vec<String> = (0..6)
                .map(|i| format!("0x{:08x}", ram.peek_word(value + 4 * i)))
                .collect();
            eprintln!("request at 0x{value:08x}: {}", words.join(" "));
        }
        assert_eq!(query_id, FIELD_OPS_ADVISE_QUERY_ID, "a field hint query");
        assert!(
            self.outbound.is_empty(),
            "the guest reads the answer in full before the next query"
        );
        let (op, operand) = read_request(&GuestRam(ram), value as usize);
        let response = if op == FieldHintOp::Secp256k1RecoveryWitness && !self.witness {
            vec![0]
        } else {
            answer(op, &operand, self.representation)
        };
        if op == FieldHintOp::Secp256k1RecoveryWitness && response[0] != 0 {
            for words in response[17..].chunks_exact(4) {
                let mut value = 0u128;
                for (i, word) in words.iter().enumerate() {
                    value |= u128::from(*word) << (32 * i);
                }
                let bits = 128 - (value as i128).unsigned_abs().leading_zeros();
                self.max_coefficient_bits = self.max_coefficient_bits.max(bits);
            }
        }
        self.outbound.extend(response);
        self.hints_served += 1;
        *self.hints_by_op.entry(op as u32).or_default() += 1;
    }
}

impl NonDeterminismCSRSource for BenchSource {
    fn read(&mut self) -> u32 {
        self.outbound
            .pop_front()
            .expect("the guest reads only what the host sent")
    }

    fn write_with_memory_access<M: RamPeek + ?Sized>(&mut self, ram: &M, value: u32) {
        self.write(&PeekRef(ram), value);
    }

    fn write_with_memory_access_raw(&mut self, ram: &[u32], value: u32) {
        self.write(&PeekRef(ram), value);
    }

    fn write_with_memory_access_dyn(&mut self, ram: &dyn RamPeek, value: u32) {
        self.write(ram, value);
    }
}

/// A sized peeker over an unsized one, for `&dyn RamPeek`
struct PeekRef<'a, M: RamPeek + ?Sized>(&'a M);

impl<M: RamPeek + ?Sized> RamPeek for PeekRef<'_, M> {
    fn peek_word(&self, address: u32) -> u32 {
        self.0.peek_word(address)
    }
}

/// The guest's output: the fold of the address over the repeats
fn expected_output(repeats: u32) -> [u32; 8] {
    let point =
        recover_from_bytes_with_hooks(&DIGEST, &R, &S, Y_IS_ODD, &mut DefaultSecp256k1Hooks)
            .expect("the vector recovers a key");
    let mut key = [0u8; 64];
    let (x, y) = key.split_at_mut(32);
    point.write_coordinates(
        x.try_into().expect("32 bytes"),
        y.try_into().expect("32 bytes"),
    );
    let mut hasher = Keccak256::new();
    hasher.update(&key);
    let mut address = [0u8; 32];
    hasher.finalize_reset_with_closure(|digest| address = *digest.as_array());
    address[..12].fill(0);
    assert_eq!(address, ADDRESS, "the host recovers the vector's address");
    let mut folded = [0u32; 8];
    for i in 0..repeats {
        for (word, chunk) in folded.iter_mut().zip(address.as_chunks::<4>().0) {
            *word ^= u32::from_le_bytes(*chunk).rotate_left(i % 32);
        }
    }
    folded
}

/// A dist directory's `app.bin` and `app.text` as words, and the representation
/// of its elements (from the features in its manifest)
struct Dist {
    rom: Vec<u32>,
    text: Vec<u32>,
    representation: Representation,
}

impl Dist {
    fn load(dir: &Path) -> anyhow::Result<Self> {
        let words = |name: &str| -> anyhow::Result<Vec<u32>> {
            let path = dir.join(name);
            let bytes = std::fs::read(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            anyhow::ensure!(
                bytes.len() % 4 == 0,
                "{} is not whole words",
                path.display()
            );
            Ok(bytes
                .as_chunks::<4>()
                .0
                .iter()
                .map(|chunk| u32::from_le_bytes(*chunk))
                .collect())
        };
        let manifest = dir.join("manifest.toml");
        let manifest = std::fs::read_to_string(&manifest)
            .with_context(|| format!("failed to read {}", manifest.display()))?;
        let features = manifest
            .lines()
            .find_map(|line| line.strip_prefix("features = "))
            .context("the manifest names the features of the build")?
            .trim_matches('"');
        let representation = if features.split(',').any(|feature| feature.trim() == "bigint") {
            Representation::Montgomery
        } else {
            Representation::Canonical
        };
        Ok(Self {
            rom: words("app.bin")?,
            text: words("app.text")?,
            representation,
        })
    }
}

/// What one run measured
struct Measurement {
    mode: u32,
    repeats: u32,
    total_cycles: u64,
    /// The hint requests the host answered over the run
    hints_served: usize,
    hints_by_op: BTreeMap<u32, usize>,
    max_coefficient_bits: u32,
    /// The cycles of every recovery
    cycles: Vec<u64>,
    /// The delegation calls of every recovery, by delegation id
    delegations: Vec<BTreeMap<u32, u64>>,
}

impl Measurement {
    fn report(&self) -> String {
        let mut sorted = self.cycles.clone();
        sorted.sort_unstable();
        let median = sorted[sorted.len() / 2];
        let mean = self.cycles.iter().sum::<u64>() / self.cycles.len() as u64;
        let mut by_id: BTreeMap<u32, u64> = BTreeMap::new();
        for call in &self.delegations {
            for (id, count) in call {
                *by_id.entry(*id).or_default() += count;
            }
        }
        let delegations: Vec<String> = by_id
            .iter()
            .map(|(id, total)| format!("0x{id:x}: {:.1}", *total as f64 / self.repeats as f64))
            .collect();
        format!(
            "{}: {} recoveries; per recovery {mean} cycles (min {}, median {median}, max {}); delegation calls per recovery [{}]; hints per recovery {:.1} [{}]{}; run total {} cycles",
            MODE_NAMES[self.mode as usize],
            self.repeats,
            sorted[0],
            sorted[sorted.len() - 1],
            if delegations.is_empty() { "none".to_string() } else { delegations.join(", ") },
            self.hints_served as f64 / self.repeats as f64,
            self.hints_by_op
                .iter()
                .map(|(op, n)| {
                    let name = match FieldHintOp::parse_u32(*op) {
                        Some(FieldHintOp::Secp256k1BaseFieldSqrt) => "sqrt",
                        Some(FieldHintOp::Secp256k1BaseFieldInverse) => "inverse",
                        Some(FieldHintOp::Secp256k1ScalarFieldInverse) => "scalar inverse",
                        Some(FieldHintOp::Secp256k1BaseFieldDivision) => "division",
                        Some(FieldHintOp::Secp256k1RecoveryWitness) => "recovery witness",
                        None => "?",
                    };
                    format!("{name} {:.1}", *n as f64 / self.repeats as f64)
                })
                .collect::<Vec<_>>()
                .join(", "),
            if self.max_coefficient_bits > 0 {
                format!("; longest coefficient {} bits", self.max_coefficient_bits)
            } else {
                String::new()
            },
            self.total_cycles,
        )
    }
}

/// Runs the guest once, as `airbender-host`'s transpiler runner does without
/// the JIT (the full unsigned machine, decoding protected against jumps into a
/// delegation sequence), and reads what its markers say
fn measure<D: DecodingOptions>(
    dist: &Dist,
    mode: u32,
    repeats: u32,
    flamegraph: Option<&FlamegraphConfig>,
) -> anyhow::Result<Measurement> {
    anyhow::ensure!(repeats > 0, "at least one recovery");
    let instructions = preprocess_bytecode::<D, true>(&dist.text);
    let tape = SimpleTape::new(&instructions);
    let mut ram =
        RamWithRomRegion::<{ ROM_SECOND_WORD_BITS }>::from_rom_content(&dist.rom, RAM_BYTES);
    let mut state = State::initial_with_counters(DelegationsCounters::default());
    let mut source = BenchSource::new(mode, repeats, dist.representation);
    let (reached_end, markers) = CycleMarkerHooks::with(|| match flamegraph {
        None => Ok(
            VM::<DelegationsCounters, CycleMarkerHooks>::run_basic_unrolled::<_, _, _, BabyBearField>(
                &mut state,
                &mut ram,
                &mut (),
                &tape,
                CYCLE_BOUND,
                &mut source,
            ),
        ),
        Some(config) => {
            let mut profiler = VmFlamegraphProfiler::new(config.clone())?;
            VM::<DelegationsCounters, CycleMarkerHooks>::run_basic_unrolled_with_flamegraph::<
                _,
                _,
                _,
                BabyBearField,
            >(
                &mut state,
                &mut ram,
                &mut (),
                &tape,
                CYCLE_BOUND,
                &mut source,
                &mut profiler,
            )
        }
    });
    let reached_end = reached_end.context("the profiler failed")?;
    anyhow::ensure!(reached_end, "the guest did not reach the end");
    let output: [u32; 8] = core::array::from_fn(|i| state.registers[10 + i].value);
    anyhow::ensure!(
        output == expected_output(repeats),
        "the guest's output is not the fold of the address (pc 0x{:x})",
        state.pc
    );
    anyhow::ensure!(
        markers.markers.len() == 2 * repeats as usize,
        "{} markers for {repeats} recoveries",
        markers.markers.len()
    );
    let mut cycles = Vec::with_capacity(repeats as usize);
    let mut delegations = Vec::with_capacity(repeats as usize);
    for pair in markers.markers.chunks_exact(2) {
        let Mark {
            cycles: call_cycles,
            delegations: call_delegations,
        } = pair[1].diff(&pair[0]);
        cycles.push(call_cycles);
        delegations.push(call_delegations.into_iter().collect());
    }
    Ok(Measurement {
        mode,
        repeats,
        total_cycles: (state.timestamp - INITIAL_TIMESTAMP) / TIMESTAMP_STEP,
        hints_served: source.hints_served,
        hints_by_op: source.hints_by_op,
        max_coefficient_bits: source.max_coefficient_bits,
        cycles,
        delegations,
    })
}

/// The field operations of the guest's microbenchmark, in its order
const MICRO_OPERATIONS: [&str; 10] = [
    "mul",
    "square",
    "add",
    "sub (normalizing)",
    "normalize",
    "is_zero",
    "to_bytes",
    "from_bytes",
    "sub with magnitude",
    "normalize weak",
];
const MICRO_OPS: u64 = 256;

/// Runs the guest's field microbenchmark: the cycles and delegation calls of one
/// operation of each kind
fn measure_micro<D: DecodingOptions>(dist: &Dist) -> anyhow::Result<Vec<String>> {
    let instructions = preprocess_bytecode::<D, true>(&dist.text);
    let tape = SimpleTape::new(&instructions);
    let mut ram = RamWithRomRegion::<{ ROM_SECOND_WORD_BITS }>::from_rom_content(&dist.rom, RAM_BYTES);
    let mut state = State::initial_with_counters(DelegationsCounters::default());
    let mut source = BenchSource::new(MODE_MICRO, 0, dist.representation);
    let (reached_end, markers) = CycleMarkerHooks::with(|| {
        VM::<DelegationsCounters, CycleMarkerHooks>::run_basic_unrolled::<_, _, _, BabyBearField>(
            &mut state,
            &mut ram,
            &mut (),
            &tape,
            CYCLE_BOUND,
            &mut source,
        )
    });
    anyhow::ensure!(reached_end, "the guest did not reach the end");
    anyhow::ensure!(
        markers.markers.len() == 2 * MICRO_OPERATIONS.len(),
        "{} markers for {} operations",
        markers.markers.len(),
        MICRO_OPERATIONS.len()
    );
    let mut lines = Vec::new();
    for (name, pair) in MICRO_OPERATIONS.iter().zip(markers.markers.chunks_exact(2)) {
        let Mark {
            cycles,
            delegations,
        } = pair[1].diff(&pair[0]);
        if cycles == 0 {
            continue;
        }
        let mut delegations: Vec<(u32, u64)> = delegations.into_iter().collect();
        delegations.sort_unstable();
        let delegations: Vec<String> = delegations
            .iter()
            .map(|(id, n)| format!("0x{id:x}: {:.2}", *n as f64 / MICRO_OPS as f64))
            .collect();
        lines.push(format!(
            "{name:<20} {:>7.1} cycles{}",
            cycles as f64 / MICRO_OPS as f64,
            if delegations.is_empty() {
                String::new()
            } else {
                format!(", delegation calls [{}]", delegations.join(", "))
            }
        ));
    }
    Ok(lines)
}

fn main() -> anyhow::Result<()> {
    let mut dists: Vec<PathBuf> = Vec::new();
    let mut repeats = 8u32;
    let mut runs: Vec<u32> = vec![MODE_HINTS, 0];
    let mut full_machine = false;
    let mut flamegraph: Option<PathBuf> = None;
    let mut sampling = 100usize;
    let mut micro = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--repeats" => {
                repeats = args
                    .next()
                    .context("--repeats takes a number")?
                    .parse()
                    .context("--repeats takes a number")?;
            }
            "--hints" => {
                runs = match args.next().as_deref() {
                    Some("on") => vec![MODE_HINTS],
                    Some("off") => vec![0],
                    Some("both") => vec![MODE_HINTS, 0],
                    Some("witness") => vec![MODE_WITNESSED],
                    Some("all") => vec![MODE_WITNESSED, MODE_HINTS, 0],
                    _ => anyhow::bail!("--hints takes on, off, both, witness or all"),
                };
            }
            "--flamegraph" => {
                flamegraph = Some(PathBuf::from(
                    args.next().context("--flamegraph takes an output path")?,
                ));
            }
            "--sampling" => {
                sampling = args
                    .next()
                    .context("--sampling takes a number of cycles")?
                    .parse()
                    .context("--sampling takes a number of cycles")?;
            }
            "--micro" => micro = true,
            "--machine" => {
                full_machine = match args.next().as_deref() {
                    Some("unsigned") => false,
                    Some("full") => true,
                    _ => anyhow::bail!("--machine takes unsigned or full"),
                };
            }
            _ => dists.push(PathBuf::from(arg)),
        }
    }
    anyhow::ensure!(
        !dists.is_empty(),
        "usage: ecrecover_bench_host <dist dir>... [--repeats N] [--hints on|off|both|witness|all] [--machine unsigned|full] [--flamegraph <svg> [--sampling N]] [--micro]"
    );
    anyhow::ensure!(
        flamegraph.is_none() || (dists.len() == 1 && runs.len() == 1),
        "a flamegraph profiles one dist in one hint mode"
    );
    for dir in &dists {
        let dist = Dist::load(dir)?;
        println!("{} ({:?} words on the channel)", dir.display(), dist.representation);
        if micro {
            let lines = if full_machine {
                measure_micro::<FullMachineDecoderConfig>(&dist)
            } else {
                measure_micro::<FullUnsignedMachineDecoderConfig>(&dist)
            }
            .with_context(|| format!("{}, field microbenchmark", dir.display()))?;
            for line in lines {
                println!("  {line}");
            }
            continue;
        }
        for mode in &runs {
            let config = flamegraph.as_ref().map(|output| {
                let mut config = FlamegraphConfig::new(dir.join("app.elf"), output.clone());
                config.frequency_recip = sampling;
                config
            });
            let measurement = if full_machine {
                measure::<FullMachineDecoderConfig>(&dist, *mode, repeats, config.as_ref())
            } else {
                measure::<FullUnsignedMachineDecoderConfig>(&dist, *mode, repeats, config.as_ref())
            }
            .with_context(|| format!("{}, {}", dir.display(), MODE_NAMES[*mode as usize]))?;
            println!("  {}", measurement.report());
        }
    }
    Ok(())
}
