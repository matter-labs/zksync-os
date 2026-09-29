//! The bytecode lab: a deterministic differential and stress runner that feeds
//! `program_gen` programs to ZKsync OS and compares against REVM.
//!
//! Subcommands:
//!
//! - `run`: generate and run a batch, writing one case file per finding and a
//!   summary, sampling RISC-V in child processes.
//! - `replay`: re-run a single case file, optionally in the proving path.
//! - `export`: turn a case file into an `evm_divergence_validator` scenario.
//!
//! Exit codes: 0 no findings, 1 an S1 finding,
//! 2 only S3 findings, 3 a tool error. The RISC-V "skipped" signal is exit 3
//! from an explicit replay when the app binary is not built. Automatic sampling
//! skips missing binaries and propagates other tool failures.

mod case;
mod classify;
mod export;
mod runner;
mod shrink;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use program_gen::{generate, Fork, Mode};
use serde::Serialize;

use crate::case::{BlockContextRecord, CaseRecord, LAB_FORK};
use crate::classify::{Outcome, OutcomeClass, Severity};
use crate::runner::{run_case_forward_catching, run_case_riscv};

/// A large worker-thread stack: both engines recurse once per EVM call frame, so
/// a deep program would abort the default 8 MiB main-thread stack before a
/// report is produced.
const WORKER_STACK_BYTES: usize = 256 * 1024 * 1024;

/// Default wall-time threshold for the `Slow` outcome.
const DEFAULT_SLOW_MS: u64 = 5_000;

/// Default cap on how many findings of one signature are written to disk, so a
/// long soak reports distinct bugs rather than thousands of copies of one. The
/// run counts still reflect every occurrence.
const DEFAULT_MAX_PER_SIGNATURE: usize = 8;

/// The RISC-V app the proving path loads (`zksync_os/dist/for_tests`).
const RISCV_APP: &str = "for_tests";

fn main() {
    // Run everything on a large-stack worker. An uncaught panic on it (for
    // example a proving-mode assertion in `replay --riscv`) surfaces as a
    // non-zero exit that is deliberately not 3, so a parent never mistakes it for
    // the RISC-V "skipped" signal.
    let code = std::thread::Builder::new()
        .stack_size(WORKER_STACK_BYTES)
        .spawn(run_cli)
        .expect("failed to spawn worker thread")
        .join()
        .unwrap_or(101);
    std::process::exit(code);
}

fn run_cli() -> i32 {
    // Make `get_zksync_os_dist_dir` work even when the binary is run directly
    // (not through cargo), including in a spawned child.
    ensure_workspace_dir_env();

    let argv: Vec<String> = std::env::args().collect();
    if argv.len() < 2 {
        usage();
        return 3;
    }
    let rest = &argv[2..];
    let result = match argv[1].as_str() {
        "run" => cmd_run(rest),
        "replay" => cmd_replay(rest),
        "export" => cmd_export(rest),
        "-h" | "--help" | "help" => {
            usage();
            return 0;
        }
        other => {
            eprintln!("unknown subcommand: {other}\n");
            usage();
            return 3;
        }
    };
    match result {
        Ok(code) => code,
        Err(err) => {
            eprintln!("error: {err:#}");
            3
        }
    }
}

fn usage() {
    eprintln!(
        "bytecode-lab — differential and stress runner for program_gen programs\n\n\
         USAGE:\n\
         \x20 bytecode-lab run --seed N [--cases N] [--duration 30m] --mode MODE|--modes all \\\n\
         \x20     [--threads N] [--slow-ms MS] [--riscv-every K] [--max-per-signature N] --out DIR\n\
         \x20 bytecode-lab replay --case FILE [--slow-ms MS] [--riscv]\n\
         \x20 bytecode-lab export --case FILE --out scenario.yaml\n\n\
         MODE is one of: gas-agnostic, hook-free, unrestricted, delegation-7702\n\
         (gas-agnostic and delegation-7702 run the REVM differential comparison; see README)\n"
    );
}

// --- run -------------------------------------------------------------------

fn cmd_run(raw: &[String]) -> Result<i32> {
    let args = parse_kv(raw, &[])?;
    let seed = args.req_u64("seed")?;
    let cases = args.opt_u64("cases")?.map(|c| c as usize);
    let duration = args.opt_str("duration").map(parse_duration).transpose()?;
    if cases.is_none() && duration.is_none() {
        return Err(anyhow!(
            "provide --cases N and/or --duration (for example --duration 30m)"
        ));
    }
    let modes = parse_modes(&args)?;
    let slow_ms = args.opt_u64("slow-ms")?.unwrap_or(DEFAULT_SLOW_MS);
    let riscv_every = args.opt_u64("riscv-every")?.unwrap_or(0);
    let threads = args.opt_u64("threads")?.unwrap_or(1).max(1) as usize;
    let max_per_signature = args
        .opt_u64("max-per-signature")?
        .map(|n| n as usize)
        .unwrap_or(DEFAULT_MAX_PER_SIGNATURE);
    let out = PathBuf::from(args.req("out")?);

    let findings_dir = out.join("findings");
    std::fs::create_dir_all(&findings_dir)
        .with_context(|| format!("failed to create {}", findings_dir.display()))?;

    let riscv_available = riscv_every > 0 && riscv_dist_ready().is_some();
    if riscv_every > 0 && !riscv_available {
        eprintln!(
            "RISC-V sampling requested but the app binary is not built \
             (run `cd zksync_os && ./dump_bin.sh --type for-tests`); sampling disabled."
        );
    }

    let cfg = WorkerConfig {
        seed,
        threads,
        cases,
        deadline: duration
            .map(|secs| {
                Instant::now()
                    .checked_add(Duration::from_secs(secs))
                    .context("--duration exceeds the supported deadline")
            })
            .transpose()?,
        modes: modes.clone(),
        slow_ms,
        riscv_every,
        out: out.clone(),
        findings_dir: findings_dir.clone(),
        max_per_signature,
    };
    // Every worker owns a disjoint set of case indices (index % threads == worker)
    // and each case's seed is derived from `seed` and the index, so the whole run
    // is deterministic in `seed` regardless of the thread count and no two workers
    // ever collide on a finding filename.
    let sig_counts: Mutex<HashMap<String, usize>> = Mutex::new(HashMap::new());
    let start = Instant::now();

    let results: Vec<WorkerResult> = if threads == 1 {
        vec![run_worker(0, &cfg, &sig_counts, riscv_available)?]
    } else {
        // Borrow the shared state once; each worker closure captures the (Copy)
        // references, so nothing is moved out of this scope.
        let (cfg_ref, sig_ref) = (&cfg, &sig_counts);
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..threads)
                .map(|worker| {
                    std::thread::Builder::new()
                        .stack_size(WORKER_STACK_BYTES)
                        .spawn_scoped(scope, move || {
                            run_worker(worker, cfg_ref, sig_ref, riscv_available)
                        })
                        .context("failed to spawn worker thread")
                })
                .collect::<Result<_>>()?;
            handles
                .into_iter()
                .map(|h| join_worker(h.join()))
                .collect::<Result<_>>()
        })?
    };

    // Aggregate the workers' results.
    let mut counts = Counts::default();
    let mut findings_meta: Vec<FindingMeta> = Vec::new();
    let mut riscv = RiscvSummary {
        skipped: riscv_every > 0 && !riscv_available,
        ..Default::default()
    };
    let mut total_cases = 0usize;
    for result in results {
        counts.add(&result.counts);
        findings_meta.extend(result.findings);
        riscv.add(&result.riscv);
        total_cases += result.cases_run;
    }
    findings_meta.sort_by_key(|f| f.index);
    let signatures: BTreeMap<String, usize> = sig_counts
        .into_inner()
        .map_err(|_| anyhow!("finding counter lock poisoned"))?
        .into_iter()
        .collect();

    let secs = start.elapsed().as_secs_f64();
    let programs_per_minute = if secs > 0.0 {
        total_cases as f64 / secs * 60.0
    } else {
        0.0
    };

    let s1 = counts.internal_error + counts.revm_mismatch + counts.panic + riscv.findings;
    let s3 = counts.slow + counts.gas_schedule_diff;
    let exit_code = if s1 > 0 {
        1
    } else if s3 > 0 {
        2
    } else {
        0
    };

    let distinct_signatures = signatures.len();
    let summary = Summary {
        master_seed: seed,
        cases: total_cases,
        mode: modes
            .iter()
            .map(|m| mode_cli_name(*m))
            .collect::<Vec<_>>()
            .join(","),
        fork: LAB_FORK.to_string(),
        slow_ms,
        riscv_every,
        threads,
        wall_seconds: secs,
        programs_per_minute,
        counts: counts.clone(),
        riscv,
        distinct_signatures,
        signatures,
        findings: findings_meta,
        exit_code,
    };
    let summary_file = out.join("summary.json");
    std::fs::write(
        &summary_file,
        serde_json::to_string_pretty(&summary).expect("summary is serializable"),
    )
    .with_context(|| format!("failed to write {}", summary_file.display()))?;

    println!(
        "bytecode-lab: {total_cases} cases in {secs:.1}s = {programs_per_minute:.0} programs/min \
         (threads={threads}) | ok={} internal_error={} revm_mismatch={} gas_schedule_diff={} \
         slow={} panic={} | findings S1={s1} S3={s3} distinct={distinct_signatures} | out={}",
        counts.ok,
        counts.internal_error,
        counts.revm_mismatch,
        counts.gas_schedule_diff,
        counts.slow,
        counts.panic,
        out.display(),
    );
    Ok(exit_code)
}

/// Shared, immutable configuration for the run workers.
#[derive(Clone)]
struct WorkerConfig {
    seed: u64,
    threads: usize,
    cases: Option<usize>,
    deadline: Option<Instant>,
    modes: Vec<Mode>,
    slow_ms: u64,
    riscv_every: u64,
    out: PathBuf,
    findings_dir: PathBuf,
    max_per_signature: usize,
}

/// What one worker produced.
#[derive(Default)]
struct WorkerResult {
    counts: Counts,
    findings: Vec<FindingMeta>,
    riscv: RiscvSummary,
    cases_run: usize,
}

/// Runs the worker's shard of cases: indices `worker, worker+threads, ...` until
/// the case count or the deadline is reached. Findings are deduplicated by
/// signature through the shared `sig_counts` (every occurrence is counted, but at
/// most `max_per_signature` are written to disk), so a long soak reports distinct
/// bugs instead of thousands of copies of one.
fn run_worker(
    worker: usize,
    cfg: &WorkerConfig,
    sig_counts: &Mutex<HashMap<String, usize>>,
    riscv_available: bool,
) -> Result<WorkerResult> {
    let mut result = WorkerResult::default();
    let mut index = worker;
    loop {
        if let Some(n) = cfg.cases {
            if index >= n {
                break;
            }
        }
        if let Some(deadline) = cfg.deadline {
            if Instant::now() >= deadline {
                break;
            }
        }

        let case_seed = case_seed_for(cfg.seed, index);
        // Mode selection, like seed selection, depends on the global case index.
        let mode = mode_for_index(&cfg.modes, index);
        let program = generate(case_seed, Fork::Osaka, mode);
        let exec = run_case_forward_catching(&program, mode, cfg.slow_ms, case_seed);
        let class = exec.outcome.class();
        result.counts.bump(class);

        if exec.outcome.is_finding() {
            let signature = exec.outcome.signature();
            // Count every occurrence; write only up to the per-signature cap.
            let write = {
                let mut map = sig_counts
                    .lock()
                    .map_err(|_| anyhow!("finding counter lock poisoned"))?;
                let seen = map.entry(signature).or_insert(0);
                let write = *seen < cfg.max_per_signature;
                *seen += 1;
                write
            };
            if write {
                write_finding(cfg, index, case_seed, mode, &program, &exec)?;
                result.findings.push(FindingMeta {
                    index,
                    case_seed,
                    class: class.tag().to_string(),
                    severity: severity_tag(exec.outcome.severity()),
                    file: cfg
                        .findings_dir
                        .join(format!("case-{index:06}-{}.json", class.tag()))
                        .display()
                        .to_string(),
                });
            }
        }

        // Sample the proving path in a child process so a forward/proving
        // assertion becomes a non-zero child exit, not the end of this batch.
        if riscv_available && cfg.riscv_every > 0 && (index as u64).is_multiple_of(cfg.riscv_every)
        {
            result.riscv.sampled += 1;
            let sample = CaseRecord {
                master_seed: cfg.seed,
                case_seed,
                case_index: index,
                fork: LAB_FORK.to_string(),
                mode,
                program: program.clone(),
                block_context: BlockContextRecord::from_block_context(&exec.block_context),
                outcome: exec.outcome.clone(),
            };
            match run_riscv_child(&sample, &cfg.out, &cfg.findings_dir, index)? {
                RiscvChild::Matched => result.riscv.matched += 1,
                RiscvChild::Finding { file } => {
                    result.riscv.findings += 1;
                    result.findings.push(FindingMeta {
                        index,
                        case_seed,
                        class: "riscv-divergence".to_string(),
                        severity: "S1".to_string(),
                        file,
                    });
                }
            }
        }

        result.cases_run += 1;
        index += cfg.threads;
    }
    Ok(result)
}

fn join_worker(result: std::thread::Result<Result<WorkerResult>>) -> Result<WorkerResult> {
    result.map_err(|_| anyhow!("bytecode-lab worker panicked"))?
}

fn mode_for_index(modes: &[Mode], index: usize) -> Mode {
    modes[index % modes.len()]
}

/// Writes one finding's case file and, for a deterministic S1, the shrunk case
/// next to it.
fn write_finding(
    cfg: &WorkerConfig,
    index: usize,
    case_seed: u64,
    mode: Mode,
    program: &program_gen::Program,
    exec: &crate::runner::CaseExecution,
) -> Result<()> {
    let class = exec.outcome.class();
    let record = CaseRecord {
        master_seed: cfg.seed,
        case_seed,
        case_index: index,
        fork: LAB_FORK.to_string(),
        mode,
        program: program.clone(),
        block_context: BlockContextRecord::from_block_context(&exec.block_context),
        outcome: exec.outcome.clone(),
    };
    let case_file = cfg
        .findings_dir
        .join(format!("case-{index:06}-{}.json", class.tag()));
    std::fs::write(&case_file, record.to_json())
        .with_context(|| format!("failed to write {}", case_file.display()))?;

    // Shrink deterministic S1 findings; slowness is timing-dependent and not shrunk.
    if matches!(
        class,
        OutcomeClass::InternalError | OutcomeClass::RevmMismatch | OutcomeClass::Panic
    ) {
        let minimized = shrink::shrink(
            program,
            mode,
            cfg.slow_ms,
            case_seed,
            &exec.outcome.signature(),
        );
        let min_record = CaseRecord {
            program: minimized,
            ..record.clone()
        };
        let min_file = cfg
            .findings_dir
            .join(format!("case-{index:06}-{}.minimized.json", class.tag()));
        std::fs::write(&min_file, min_record.to_json())
            .with_context(|| format!("failed to write {}", min_file.display()))?;
    }
    Ok(())
}

/// A per-index case seed, derived from the master seed by splitmix64 so it is
/// deterministic and addressable by index (independent of thread count).
fn case_seed_for(seed: u64, index: usize) -> u64 {
    let mut x = seed ^ (index as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

/// Parses a soak duration into seconds: a bare number of seconds, or an `Ns`,
/// `Nm`, or `Nh` suffix.
fn parse_duration(text: &str) -> Result<u64> {
    let text = text.trim();
    let (number, multiplier) = if let Some(n) = text.strip_suffix('h') {
        (n, 3600)
    } else if let Some(n) = text.strip_suffix('m') {
        (n, 60)
    } else if let Some(n) = text.strip_suffix('s') {
        (n, 1)
    } else {
        (text, 1)
    };
    let value: u64 = number
        .trim()
        .parse()
        .with_context(|| format!("--duration '{text}' is not a duration (e.g. 900s, 30m, 2h)"))?;
    value
        .checked_mul(multiplier)
        .context("--duration overflows seconds")
}

/// Resolves the modes to run: `--modes all` (or a comma-separated list), else the
/// single `--mode`.
fn parse_modes(args: &ParsedArgs) -> Result<Vec<Mode>> {
    if let Some(list) = args.opt_str("modes") {
        if list == "all" {
            return Ok(vec![
                Mode::GasAgnostic,
                Mode::Unrestricted,
                Mode::HookFree,
                Mode::Delegation7702,
            ]);
        }
        return list.split(',').map(|m| parse_mode(m.trim())).collect();
    }
    Ok(vec![parse_mode(args.req("mode")?)?])
}

// --- replay ----------------------------------------------------------------

fn cmd_replay(raw: &[String]) -> Result<i32> {
    let args = parse_kv(raw, &["riscv"])?;
    let case_path = PathBuf::from(args.req("case")?);
    let slow_ms = args.opt_u64("slow-ms")?.unwrap_or(DEFAULT_SLOW_MS);
    let riscv = args.flags.contains("riscv");

    let text = std::fs::read_to_string(&case_path)
        .with_context(|| format!("failed to read {}", case_path.display()))?;
    let record = CaseRecord::from_json(&text)?;

    if riscv {
        // The proving path needs the built app binary. Report its absence as a
        // tool error (exit 3), which the parent reads as "skipped".
        if riscv_dist_ready().is_none() {
            eprintln!(
                "RISC-V app binary not found under {}. Build it with \
                 `cd zksync_os && ./dump_bin.sh --type for-tests`.",
                rig::chain::get_zksync_os_dist_dir(&Some(RISCV_APP.to_string())).display()
            );
            return Ok(3);
        }
        // No catch_unwind: a proving divergence asserts in the rig and aborts the
        // process here, which is exactly the non-zero exit the parent looks for.
        let exec = run_case_riscv(&record.program, record.mode, slow_ms, record.case_seed);
        print_replay_report(&exec.outcome, exec.deploy_wall_ms, exec.calls_wall_ms);
        return Ok(exit_for_severity(exec.outcome.severity()));
    }

    let exec = run_case_forward_catching(&record.program, record.mode, slow_ms, record.case_seed);
    print_replay_report(&exec.outcome, exec.deploy_wall_ms, exec.calls_wall_ms);
    Ok(exit_for_severity(exec.outcome.severity()))
}

fn print_replay_report(outcome: &Outcome, deploy_wall_ms: u128, calls_wall_ms: u128) {
    #[derive(Serialize)]
    struct ReplayReport<'a> {
        outcome: &'a Outcome,
        deploy_wall_ms: u128,
        calls_wall_ms: u128,
    }
    let report = ReplayReport {
        outcome,
        deploy_wall_ms,
        calls_wall_ms,
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&report).expect("report is serializable")
    );
}

// --- export ----------------------------------------------------------------

fn cmd_export(raw: &[String]) -> Result<i32> {
    let args = parse_kv(raw, &[])?;
    let case_path = PathBuf::from(args.req("case")?);
    let out_path = PathBuf::from(args.req("out")?);

    let text = std::fs::read_to_string(&case_path)
        .with_context(|| format!("failed to read {}", case_path.display()))?;
    let record = CaseRecord::from_json(&text)?;

    let yaml = export::export_scenario(&record)?;
    std::fs::write(&out_path, yaml)
        .with_context(|| format!("failed to write {}", out_path.display()))?;
    println!("wrote scenario to {}", out_path.display());
    Ok(0)
}

// --- RISC-V child ----------------------------------------------------------

enum RiscvChild {
    Matched,
    Finding { file: String },
}

fn run_riscv_child(
    record: &CaseRecord,
    out_dir: &Path,
    findings_dir: &Path,
    index: usize,
) -> Result<RiscvChild> {
    let sample_path = riscv_sample_path(out_dir, index);
    std::fs::write(&sample_path, record.to_json())
        .with_context(|| format!("failed to write {}", sample_path.display()))?;
    let output = Command::new(std::env::current_exe()?)
        .arg("replay")
        .arg("--case")
        .arg(&sample_path)
        .arg("--riscv")
        // RISC-V sampling checks correctness, not the simulator's wall time.
        .arg("--slow-ms")
        .arg(u64::MAX.to_string())
        .output()
        .context("failed to start RISC-V replay")?;
    std::fs::remove_file(&sample_path)?;
    match output.status.code() {
        Some(0) => Ok(RiscvChild::Matched),
        Some(3) => Err(anyhow!(
            "RISC-V replay failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )),
        _ => {
            let base = findings_dir.join(format!("case-{index:06}-riscv-divergence"));
            let json = base.with_extension("json");
            std::fs::write(&json, record.to_json())?;
            let mut log = output.stdout;
            log.extend_from_slice(&output.stderr);
            std::fs::write(base.with_extension("log"), log)?;
            Ok(RiscvChild::Finding {
                file: json.display().to_string(),
            })
        }
    }
}

fn riscv_sample_path(out_dir: &Path, index: usize) -> PathBuf {
    out_dir.join(format!(".riscv_sample-{index}.json"))
}

/// The dist directory if the RISC-V app binary is present, else `None`.
fn riscv_dist_ready() -> Option<PathBuf> {
    let dir = rig::chain::get_zksync_os_dist_dir(&Some(RISCV_APP.to_string()));
    if dir.join("app.bin").exists() || dir.join("app.elf").exists() {
        Some(dir)
    } else {
        None
    }
}

fn ensure_workspace_dir_env() {
    if std::env::var_os("CARGO_WORKSPACE_DIR").is_none() {
        // The crate dir is <repo>/tests/bytecode_lab at compile time; the repo
        // root is two levels up.
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        if let Some(root) = manifest.parent().and_then(Path::parent) {
            std::env::set_var("CARGO_WORKSPACE_DIR", root);
        }
    }
}

// --- summary types ---------------------------------------------------------

#[derive(Clone, Default, Serialize)]
struct Counts {
    ok: u64,
    internal_error: u64,
    revm_mismatch: u64,
    /// Independent-gas discrepancies (S3), counted apart from the
    /// actionable `revm_mismatch` state divergences.
    gas_schedule_diff: u64,
    slow: u64,
    panic: u64,
}

impl Counts {
    fn bump(&mut self, class: OutcomeClass) {
        match class {
            OutcomeClass::Ok => self.ok += 1,
            OutcomeClass::InternalError => self.internal_error += 1,
            OutcomeClass::RevmMismatch => self.revm_mismatch += 1,
            OutcomeClass::GasScheduleDiff => self.gas_schedule_diff += 1,
            OutcomeClass::Slow => self.slow += 1,
            OutcomeClass::Panic => self.panic += 1,
        }
    }

    fn add(&mut self, other: &Counts) {
        self.ok += other.ok;
        self.internal_error += other.internal_error;
        self.revm_mismatch += other.revm_mismatch;
        self.gas_schedule_diff += other.gas_schedule_diff;
        self.slow += other.slow;
        self.panic += other.panic;
    }
}

#[derive(Default, Serialize)]
struct RiscvSummary {
    sampled: u64,
    matched: u64,
    findings: u64,
    skipped: bool,
}

impl RiscvSummary {
    fn add(&mut self, other: &RiscvSummary) {
        self.sampled += other.sampled;
        self.matched += other.matched;
        self.findings += other.findings;
        self.skipped = self.skipped || other.skipped;
    }
}

#[derive(Serialize)]
struct FindingMeta {
    index: usize,
    case_seed: u64,
    class: String,
    severity: String,
    file: String,
}

#[derive(Serialize)]
struct Summary {
    master_seed: u64,
    cases: usize,
    mode: String,
    fork: String,
    slow_ms: u64,
    riscv_every: u64,
    threads: usize,
    wall_seconds: f64,
    programs_per_minute: f64,
    counts: Counts,
    riscv: RiscvSummary,
    /// Number of distinct finding signatures seen (the deduped bug count).
    distinct_signatures: usize,
    /// Every finding signature and how many cases produced it.
    signatures: BTreeMap<String, usize>,
    findings: Vec<FindingMeta>,
    exit_code: i32,
}

fn severity_tag(severity: Severity) -> String {
    match severity {
        Severity::None => "none",
        Severity::S1 => "S1",
        Severity::S3 => "S3",
    }
    .to_string()
}

fn exit_for_severity(severity: Severity) -> i32 {
    match severity {
        Severity::None => 0,
        Severity::S1 => 1,
        Severity::S3 => 2,
    }
}

// --- mode + argument parsing ----------------------------------------------

fn parse_mode(s: &str) -> Result<Mode> {
    match s {
        "gas-agnostic" | "gas_agnostic" => Ok(Mode::GasAgnostic),
        "hook-free" | "hook_free" => Ok(Mode::HookFree),
        "unrestricted" => Ok(Mode::Unrestricted),
        "delegation-7702" | "delegation_7702" | "7702" => Ok(Mode::Delegation7702),
        other => Err(anyhow!(
            "unknown mode '{other}' (expected gas-agnostic, hook-free, unrestricted or delegation-7702)"
        )),
    }
}

fn mode_cli_name(mode: Mode) -> &'static str {
    match mode {
        Mode::GasAgnostic => "gas-agnostic",
        Mode::HookFree => "hook-free",
        Mode::Unrestricted => "unrestricted",
        Mode::Delegation7702 => "delegation-7702",
    }
}

/// A minimal `--key value` / `--flag` parser. Known flags are boolean; every
/// other `--key` consumes the following token as its value.
struct ParsedArgs {
    opts: HashMap<String, String>,
    flags: HashSet<String>,
}

impl ParsedArgs {
    fn req(&self, key: &str) -> Result<&String> {
        self.opts
            .get(key)
            .ok_or_else(|| anyhow!("missing required --{key}"))
    }

    fn req_u64(&self, key: &str) -> Result<u64> {
        self.req(key)?
            .parse::<u64>()
            .with_context(|| format!("--{key} must be a non-negative integer"))
    }

    fn opt_u64(&self, key: &str) -> Result<Option<u64>> {
        match self.opts.get(key) {
            Some(v) => {
                Ok(Some(v.parse::<u64>().with_context(|| {
                    format!("--{key} must be a non-negative integer")
                })?))
            }
            None => Ok(None),
        }
    }

    fn opt_str(&self, key: &str) -> Option<&str> {
        self.opts.get(key).map(String::as_str)
    }
}

fn parse_kv(raw: &[String], known_flags: &[&str]) -> Result<ParsedArgs> {
    let mut opts = HashMap::new();
    let mut flags = HashSet::new();
    let mut it = raw.iter();
    while let Some(arg) = it.next() {
        let name = arg
            .strip_prefix("--")
            .ok_or_else(|| anyhow!("unexpected argument: {arg}"))?;
        if known_flags.contains(&name) {
            flags.insert(name.to_string());
        } else {
            let value = it
                .next()
                .ok_or_else(|| anyhow!("missing value for --{name}"))?;
            opts.insert(name.to_string(), value.clone());
        }
    }
    Ok(ParsedArgs { opts, flags })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case_modes_and_seeds_do_not_depend_on_worker_count() {
        let modes = [
            Mode::GasAgnostic,
            Mode::HookFree,
            Mode::Unrestricted,
            Mode::Delegation7702,
        ];
        let expected: Vec<_> = (0..32)
            .map(|i| (i, case_seed_for(7, i), mode_for_index(&modes, i)))
            .collect();
        for threads in [1, 2, 3, 8] {
            let mut sharded: Vec<_> = (0..threads)
                .flat_map(|worker| {
                    let modes = &modes;
                    (worker..32)
                        .step_by(threads)
                        .map(move |i| (i, case_seed_for(7, i), mode_for_index(modes, i)))
                })
                .collect();
            sharded.sort_by_key(|case| case.0);
            assert_eq!(sharded, expected);
        }
    }

    #[test]
    fn worker_panics_and_errors_are_not_successful_runs() {
        assert!(join_worker(Err(Box::new("worker panic"))).is_err());
        assert!(join_worker(Ok(Err(anyhow!("finding write failed")))).is_err());
    }

    #[test]
    fn parallel_riscv_samples_have_distinct_paths() {
        let out = Path::new("samples");
        let paths: HashSet<_> = (0..32).map(|i| riscv_sample_path(out, i)).collect();
        assert_eq!(paths.len(), 32);
    }

    #[test]
    fn duration_overflow_is_a_tool_error() {
        assert_eq!(parse_duration("2h").unwrap(), 7200);
        assert!(parse_duration("18446744073709551615h").is_err());
    }
}
