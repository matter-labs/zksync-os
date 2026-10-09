//! Self-consistency of the Glamsterdam programs over a block: every program
//! runs natively first, recording the oracle's responses as the RISC-V guest
//! reads them, then on the RISC-V simulator from the recorded words alone, and
//! the simulator's public output must be the hash of the linkage output the
//! native run computed. The ranges of the block run from the initial value
//! tables of the one native pre-execution run, and all outputs are aggregated
//! (`glamsterdam_range::aggregate`). `follow` does this for every finalized
//! Sepolia block as it comes, the long-running form.

use crate::glamsterdam::{fetch_one, finalized_block_number, GlamsterdamBlock};
use crate::glamsterdam_range::{
    aggregate, run_range_recording, split_by_gas, IndexTries, InitialValueTables, RangeStart,
};
use crate::glamsterdam_state::run_state_consistency_recording;
use airbender_host::raw::QuasiUARTSource;
use airbender_host::Program;
use anyhow::Context;
use basic_bootloader::bootloader::block_flow::ethereum_glamsterdam::linkage::ProgramOutput;
use basic_bootloader::bootloader::block_flow::ethereum_glamsterdam::tx_range::block_flow::GlamsterdamRangeOutput;
use basic_bootloader::bootloader::block_flow::ethereum_glamsterdam::tx_range::witness::TransactionRange;
use std::alloc::Global;
use std::path::Path;
use std::time::{Duration, Instant};

/// The default dists of the two programs (`binaries/build.sh`)
pub const PRE_EXECUTION_APP: &str = "binaries/glamsterdam_pre_execution/dist";
pub const TX_RANGE_APP: &str = "binaries/glamsterdam_tx_range/dist";

/// The two programs of a block
pub struct Programs {
    pub pre_execution: Program,
    pub tx_range: Program,
}

impl Programs {
    pub fn load(pre_execution_app: &str, tx_range_app: &str) -> anyhow::Result<Self> {
        Ok(Self {
            pre_execution: crate::ethproofs::load_program_for(pre_execution_app)?,
            tx_range: crate::ethproofs::load_program_for(tx_range_app)?,
        })
    }

    pub fn load_default() -> anyhow::Result<Self> {
        Self::load(PRE_EXECUTION_APP, TX_RANGE_APP)
    }
}

/// One simulator run of a program from recorded words: its output and cycles
fn simulate(program: &Program, words: Vec<u32>) -> anyhow::Result<([u32; 8], u64)> {
    let mut builder = program.transpiler_runner();
    if cfg!(target_arch = "x86_64") {
        builder = builder.with_jit();
    }
    let runner = builder
        .build()
        .context("failed to build the transpiler runner")?;
    let execution = runner
        .run_with_source(QuasiUARTSource::new_with_reads(words))
        .context("transpiler execution failed")?;
    anyhow::ensure!(execution.reached_end, "the program did not reach the end");
    Ok((execution.receipt.output, execution.cycles_executed as u64))
}

/// What a block exercises: logged per block and counted over a run, so that
/// a run says which parts of the protocol it covered
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BlockFeatures {
    /// EIP-4844 blob transactions and the header's blob gas
    pub blob_txs: usize,
    pub blob_gas_used: u64,
    /// EIP-7702 set-code transactions
    pub set_code_txs: usize,
    /// Contract creation transactions
    pub creates: usize,
    /// Consensus layer withdrawals of the body
    pub withdrawals: usize,
    /// EIP-6110 deposit requests over the ranges
    pub deposit_requests: usize,
    /// The non-empty system request types (1 withdrawal, 2 consolidation,
    /// 3 builder deposit, 4 builder exit), as the last range hashed them
    pub request_types: Vec<u8>,
}

impl BlockFeatures {
    pub fn of(fixture: &GlamsterdamBlock, outputs: &[GlamsterdamRangeOutput<Global>]) -> Self {
        use alloy::consensus::Transaction as _;
        use alloy::consensus::Typed2718 as _;
        let transactions = fixture.block.transactions.clone().into_transactions_vec();
        let count = |ty: u8| transactions.iter().filter(|tx| tx.inner.ty() == ty).count();
        Self {
            blob_txs: count(3),
            blob_gas_used: fixture.block.header.inner.blob_gas_used.unwrap_or(0),
            set_code_txs: count(4),
            creates: transactions
                .iter()
                .filter(|tx| tx.inner.kind().is_create())
                .count(),
            withdrawals: fixture
                .block
                .withdrawals
                .as_ref()
                .map_or(0, |withdrawals| withdrawals.len()),
            deposit_requests: outputs
                .iter()
                .map(|output| output.requests.deposit_requests() as usize)
                .sum(),
            request_types: outputs
                .last()
                .and_then(|output| output.requests.system.as_ref())
                .map_or(Vec::new(), |system| system.types().to_vec()),
        }
    }

    fn request_type_name(request_type: u8) -> &'static str {
        match request_type {
            1 => "withdrawal",
            2 => "consolidation",
            3 => "builder-deposit",
            4 => "builder-exit",
            _ => "unknown",
        }
    }

    /// One line, human-readable
    pub fn line(&self) -> String {
        let requests = if self.request_types.is_empty() {
            "none".to_string()
        } else {
            self.request_types
                .iter()
                .map(|t| Self::request_type_name(*t))
                .collect::<Vec<_>>()
                .join("+")
        };
        format!(
            "blob txs {} ({} blob gas), set-code txs {}, creates {}, withdrawals {}, deposit requests {}, system requests {requests}",
            self.blob_txs,
            self.blob_gas_used,
            self.set_code_txs,
            self.creates,
            self.withdrawals,
            self.deposit_requests
        )
    }

    pub const CSV_HEADER: &'static str =
        "blob_txs,blob_gas_used,set_code_txs,creates,withdrawals,deposit_requests,request_types";

    /// The CSV columns of `CSV_HEADER`
    pub fn csv(&self) -> String {
        format!(
            "{},{},{},{},{},{},{}",
            self.blob_txs,
            self.blob_gas_used,
            self.set_code_txs,
            self.creates,
            self.withdrawals,
            self.deposit_requests,
            self.request_types
                .iter()
                .map(|t| t.to_string())
                .collect::<Vec<_>>()
                .join("+")
        )
    }
}

/// How many blocks of a run exercised each feature
#[derive(Debug, Default)]
pub struct FeatureCounts {
    pub blocks: usize,
    pub with_blobs: usize,
    pub with_set_code: usize,
    pub with_creates: usize,
    pub with_withdrawals: usize,
    pub with_deposit_requests: usize,
    /// By request type 1..=4
    pub with_request_type: [usize; 5],
}

impl FeatureCounts {
    pub fn add(&mut self, features: &BlockFeatures) {
        self.blocks += 1;
        self.with_blobs += (features.blob_txs > 0) as usize;
        self.with_set_code += (features.set_code_txs > 0) as usize;
        self.with_creates += (features.creates > 0) as usize;
        self.with_withdrawals += (features.withdrawals > 0) as usize;
        self.with_deposit_requests += (features.deposit_requests > 0) as usize;
        for request_type in &features.request_types {
            if let Some(slot) = self.with_request_type.get_mut(*request_type as usize) {
                *slot += 1;
            }
        }
    }

    pub fn line(&self) -> String {
        format!(
            "coverage over {} block(s): blobs {}, set-code {}, creates {}, withdrawals {}, deposit requests {}, withdrawal requests {}, consolidation requests {}, builder deposit requests {}, builder exit requests {}",
            self.blocks,
            self.with_blobs,
            self.with_set_code,
            self.with_creates,
            self.with_withdrawals,
            self.with_deposit_requests,
            self.with_request_type[1],
            self.with_request_type[2],
            self.with_request_type[3],
            self.with_request_type[4],
        )
    }
}

/// What the check of a block established
#[derive(Debug)]
pub struct BlockReport {
    pub number: u64,
    pub gas_used: u64,
    pub transactions: usize,
    pub features: BlockFeatures,
    pub pre_execution_cycles: u64,
    pub pre_execution_words: usize,
    /// Per range: the transaction range, the simulator cycles, the recorded words
    pub ranges: Vec<(TransactionRange, u64, usize)>,
    pub elapsed: Duration,
}

impl BlockReport {
    pub fn range_cycles(&self) -> u64 {
        self.ranges.iter().map(|(_, cycles, _)| cycles).sum()
    }

    pub fn total_cycles(&self) -> u64 {
        self.pre_execution_cycles + self.range_cycles()
    }

    pub fn line(&self) -> String {
        format!(
            "block {}: {} gas, {} txs, {} range(s): pre-execution {} cycles ({} words), ranges {} cycles, total {} cycles ({:.2}/gas), {:.1}s",
            self.number,
            self.gas_used,
            self.transactions,
            self.ranges.len(),
            self.pre_execution_cycles,
            self.pre_execution_words,
            self.range_cycles(),
            self.total_cycles(),
            self.total_cycles() as f64 / self.gas_used.max(1) as f64,
            self.elapsed.as_secs_f64(),
        )
    }
}

/// The initial value tables and the ranges of a block, dumped next to its
/// fixture before the ranges run, so that a failing range can be replayed from
/// the files alone
pub const INITIAL_VALUES_FILE: &str = "initial_values.bin";
pub const RANGES_FILE: &str = "ranges.txt";

/// Checks one block: native pre-execution with the witness recorded, the
/// simulator from the recording, then the ranges (split into `parts` by gas)
/// the same way, and the aggregation of all outputs. With `dump_dir`, the
/// tables of the pre-execution run and the ranges are written there first.
pub fn check_block(
    fixture: &GlamsterdamBlock,
    programs: &Programs,
    parts: u32,
    dump_dir: Option<&Path>,
) -> anyhow::Result<BlockReport> {
    let started = Instant::now();
    let number = fixture.block.header.inner.number;
    let tries = IndexTries::of(fixture)?;
    println!(
        "[block {number}] {} gas, {} transactions, {} parts",
        fixture.block.header.inner.gas_used,
        tries.transactions.len(),
        parts
    );

    // the pre-execution check: natively with the words recorded, then from the words
    let step = Instant::now();
    let (state, words) = run_state_consistency_recording(fixture)
        .with_context(|| format!("block {number}: native pre-execution"))?;
    let pre_execution_words = words.len();
    println!(
        "[block {number}] pre-execution: native run OK in {:.1}s, {} accounts, {} slots, {} words recorded",
        step.elapsed().as_secs_f64(),
        state.counts.accounts,
        state.counts.slots,
        pre_execution_words
    );
    let expected = ProgramOutput::of_pre_execution(&state).hash_words();
    let step = Instant::now();
    let (output, pre_execution_cycles) = simulate(&programs.pre_execution, words)
        .with_context(|| format!("block {number}: pre-execution on the simulator"))?;
    anyhow::ensure!(
        output == expected,
        "block {number}: the pre-execution program's output is not the hash of the linkage output"
    );
    println!(
        "[block {number}] pre-execution: simulator OK in {:.1}s, {pre_execution_cycles} cycles, output matches",
        step.elapsed().as_secs_f64()
    );

    // the ranges, from the tables of the pre-execution run
    let tables = InitialValueTables::of(&state);
    let ranges = split_by_gas(&tries, parts)?;
    if let Some(dir) = dump_dir {
        tables.write(&dir.join(INITIAL_VALUES_FILE))?;
        let listed: Vec<String> = ranges
            .iter()
            .map(|range| format!("{}..{}", range.start, range.end))
            .collect();
        std::fs::write(dir.join(RANGES_FILE), listed.join("\n") + "\n")?;
    }
    let mut outputs: Vec<GlamsterdamRangeOutput<Global>> = Vec::with_capacity(ranges.len());
    let mut range_reports = Vec::with_capacity(ranges.len());
    let count = ranges.len();
    for (i, range) in ranges.into_iter().enumerate() {
        let label = format!("range {}/{count} ({}..{})", i + 1, range.start, range.end);
        let start = RangeStart::after(&outputs);
        let step = Instant::now();
        let (output, words) = run_range_recording(fixture, &tries, &tables, range, &start)
            .with_context(|| format!("block {number}: native range {range:?}"))?;
        println!(
            "[block {number}] {label}: native run OK in {:.1}s, {} transactions, {} words recorded, accessed {}/{} records",
            step.elapsed().as_secs_f64(),
            range.len(),
            words.len(),
            output.accessed.count_ones(),
            output.accessed.len()
        );
        let expected = ProgramOutput::of_range(&output).hash_words();
        let recorded = words.len();
        let step = Instant::now();
        let (public_output, cycles) = simulate(&programs.tx_range, words)
            .with_context(|| format!("block {number}: range {range:?} on the simulator"))?;
        anyhow::ensure!(
            public_output == expected,
            "block {number}: the range program's output for {range:?} is not the hash of the range's linkage output"
        );
        println!(
            "[block {number}] {label}: simulator OK in {:.1}s, {cycles} cycles, output matches",
            step.elapsed().as_secs_f64()
        );
        range_reports.push((range, cycles, recorded));
        outputs.push(output);
    }
    aggregate(&state, &outputs).with_context(|| format!("block {number}: aggregation"))?;
    println!("[block {number}] aggregation OK: the ranges cover the access list and link to the pre-execution check");
    let features = BlockFeatures::of(fixture, &outputs);
    println!("[block {number}] features: {}", features.line());

    Ok(BlockReport {
        number,
        gas_used: fixture.block.header.inner.gas_used,
        transactions: tries.transactions.len(),
        features,
        pre_execution_cycles,
        pre_execution_words,
        ranges: range_reports,
        elapsed: started.elapsed(),
    })
}

/// Checks every fixture under `path`
pub fn check_path(
    path: &Path,
    programs: &Programs,
    parts: u32,
) -> anyhow::Result<Vec<BlockReport>> {
    let dirs = crate::glamsterdam::fixture_dirs(path)?;
    let mut reports = Vec::with_capacity(dirs.len());
    for dir in &dirs {
        let fixture = GlamsterdamBlock::from_dir(dir)
            .with_context(|| format!("Failed to load fixture {}", dir.display()))?;
        let report = check_block(&fixture, programs, parts, None)
            .with_context(|| format!("fixture {}", dir.display()))?;
        println!("{}", report.line());
        reports.push(report);
    }
    Ok(reports)
}

/// How often the finalized head is polled while waiting for a block
const POLL_INTERVAL: Duration = Duration::from_secs(12);
/// How many times a block's fetch is retried before the block is given up
const FETCH_ATTEMPTS: u32 = 10;

/// The files the follower maintains in its work directory: one row per block
/// and per range with the cycles and the gas, the numbers of the blocks that
/// failed, and (kept next to its fixture) what failed for each of them
pub const BLOCKS_CSV: &str = "blocks.csv";
pub const RANGES_CSV: &str = "ranges.csv";
pub const DIVERGING_BLOCKS: &str = "diverging_blocks.txt";
pub const FAILURE_FILE: &str = "failure.txt";

/// What a run of the follower established
#[derive(Debug, Default)]
pub struct FollowSummary {
    pub reports: Vec<BlockReport>,
    /// The blocks whose check failed or panicked (their fixtures are kept)
    pub failed: Vec<u64>,
    pub coverage: FeatureCounts,
}

/// Appends a line to a file in the work directory, writing the header first
/// when the file is new
fn append_line(work_dir: &Path, file: &str, header: &str, line: &str) -> anyhow::Result<()> {
    use std::io::Write;
    let path = work_dir.join(file);
    let new = !path.exists();
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("Failed to open {}", path.display()))?;
    if new {
        writeln!(file, "{header}")?;
    }
    writeln!(file, "{line}")?;
    Ok(())
}

/// Records a block's cycles and gas in the work directory's tables
fn record_report(work_dir: &Path, report: &BlockReport) -> anyhow::Result<()> {
    append_line(
        work_dir,
        BLOCKS_CSV,
        &format!(
            "block,gas_used,transactions,parts,pre_execution_cycles,pre_execution_words,range_cycles,total_cycles,cycles_per_gas,seconds,{}",
            BlockFeatures::CSV_HEADER
        ),
        &format!(
            "{},{},{},{},{},{},{},{},{:.3},{:.1},{}",
            report.number,
            report.gas_used,
            report.transactions,
            report.ranges.len(),
            report.pre_execution_cycles,
            report.pre_execution_words,
            report.range_cycles(),
            report.total_cycles(),
            report.total_cycles() as f64 / report.gas_used.max(1) as f64,
            report.elapsed.as_secs_f64(),
            report.features.csv(),
        ),
    )?;
    for (range, cycles, words) in &report.ranges {
        append_line(
            work_dir,
            RANGES_CSV,
            "block,start_tx,end_tx,cycles,words",
            &format!(
                "{},{},{},{},{}",
                report.number, range.start, range.end, cycles, words
            ),
        )?;
    }
    Ok(())
}

/// Records a failed block: its number in the list of diverging blocks, the
/// failure next to its fixture, which is kept for analysis
fn record_failure(work_dir: &Path, dir: &Path, number: u64, failure: &str) {
    eprintln!("block {number} FAILED: {failure}");
    if let Err(e) = std::fs::create_dir_all(dir)
        .and_then(|_| std::fs::write(dir.join(FAILURE_FILE), format!("{failure}\n")))
    {
        eprintln!("could not write the failure of block {number}: {e}");
    }
    if let Err(e) = append_line(
        work_dir,
        DIVERGING_BLOCKS,
        "# blocks whose check failed",
        &number.to_string(),
    ) {
        eprintln!("could not record block {number} as diverging: {e}");
    }
}

/// The message of a panic payload
fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else {
        "panic with a non-string payload".to_string()
    }
}

/// Calls the endpoint until it answers, waiting between attempts
fn retrying<T>(what: &str, mut call: impl FnMut() -> anyhow::Result<T>) -> T {
    loop {
        match call() {
            Ok(value) => return value,
            Err(e) => {
                eprintln!("{what} failed ({e:#}); retrying in {POLL_INTERVAL:?}");
                std::thread::sleep(POLL_INTERVAL);
            }
        }
    }
}

/// Checks consecutive Sepolia blocks as they are finalized, from `first_block`
/// (the finalized head when `None`), `count` of them (forever when `None`).
/// Each block's fixture is fetched into `work_dir/<block>/` and checked; the
/// fixture is removed afterwards unless `keep`, and always kept when the check
/// fails or panics, with the failure, the initial value tables and the ranges
/// written next to it and the block number appended to `diverging_blocks.txt`. The cycles and the gas of every checked
/// block and range go to `blocks.csv` and `ranges.csv`. A failing block does
/// not stop the run, nor does a transient endpoint error.
pub fn follow(
    endpoint: &str,
    first_block: Option<u64>,
    count: Option<u64>,
    work_dir: &Path,
    programs: &Programs,
    parts: u32,
    keep: bool,
) -> anyhow::Result<FollowSummary> {
    std::fs::create_dir_all(work_dir)
        .with_context(|| format!("Failed to create {}", work_dir.display()))?;
    let mut finalized = retrying("reading the finalized head", || {
        finalized_block_number(endpoint)
    });
    let mut next = first_block.unwrap_or(finalized);
    let last = count.map(|count| next + count.saturating_sub(1));
    let mut summary = FollowSummary::default();
    println!(
        "following from block {next} (finalized head {finalized}), {} block(s), {parts} parts each; tables in {}",
        last.map_or("unbounded".to_string(), |last| (last - next + 1).to_string()),
        work_dir.display()
    );
    while last.is_none_or(|last| next <= last) {
        if next > finalized {
            println!("waiting for block {next} to be finalized (head {finalized})");
        }
        while next > finalized {
            std::thread::sleep(POLL_INTERVAL);
            finalized = retrying("reading the finalized head", || {
                finalized_block_number(endpoint)
            });
        }
        let number = next;
        next += 1;
        let dir = work_dir.join(number.to_string());

        // the fixture, with the endpoint given a few chances
        let fetch_started = Instant::now();
        let mut attempt = 0;
        let fixture = loop {
            attempt += 1;
            match fetch_one(endpoint, number, &dir)
                .and_then(|_| GlamsterdamBlock::from_dir(&dir))
                .and_then(|fixture| fixture.check().map(|_| fixture))
            {
                Ok(fixture) => {
                    println!(
                        "[block {number}] fetched in {:.1}s",
                        fetch_started.elapsed().as_secs_f64()
                    );
                    break Some(fixture);
                }
                Err(e) if attempt < FETCH_ATTEMPTS => {
                    eprintln!("fetching block {number} failed ({e:#}); attempt {attempt} of {FETCH_ATTEMPTS}");
                    // a partial or inconsistent fixture is fetched again from scratch
                    let _ = std::fs::remove_dir_all(&dir);
                    std::thread::sleep(POLL_INTERVAL);
                }
                Err(e) => {
                    record_failure(work_dir, &dir, number, &format!("fetch: {e:#}"));
                    break None;
                }
            }
        };
        let Some(fixture) = fixture else {
            summary.failed.push(number);
            continue;
        };

        // the check, with a panic treated as a failure of the block
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            check_block(&fixture, programs, parts, Some(&dir))
        }));
        match outcome {
            Ok(Ok(report)) => {
                println!("{}", report.line());
                summary.coverage.add(&report.features);
                println!(
                    "progress: {} block(s) OK, {} diverging; {}",
                    summary.reports.len() + 1,
                    summary.failed.len(),
                    summary.coverage.line()
                );
                record_report(work_dir, &report)?;
                if !keep {
                    std::fs::remove_dir_all(&dir)
                        .with_context(|| format!("Failed to remove {}", dir.display()))?;
                }
                summary.reports.push(report);
            }
            Ok(Err(e)) => {
                record_failure(work_dir, &dir, number, &format!("{e:#}"));
                summary.failed.push(number);
            }
            Err(payload) => {
                record_failure(
                    work_dir,
                    &dir,
                    number,
                    &format!("panic: {}", panic_message(payload)),
                );
                summary.failed.push(number);
            }
        }
    }
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every stored fixture is self-consistent, split into three ranges
    #[test]
    #[ignore = "needs the programs from `binaries/build.sh`"]
    fn fixtures_are_self_consistent() {
        let programs = Programs::load_default().expect("the programs");
        let reports =
            check_path(&crate::glamsterdam::default_fixture_dir(), &programs, 3).expect("fixtures");
        assert!(!reports.is_empty());
    }

    /// The long-running check over live Sepolia blocks: the endpoint comes from
    /// `GLAMSTERDAM_ENDPOINT`, the number of blocks from `GLAMSTERDAM_BLOCKS`
    /// (two by default), starting at the finalized head
    #[test]
    #[ignore = "needs `GLAMSTERDAM_ENDPOINT` and the programs from `binaries/build.sh`"]
    fn sepolia_blocks_are_self_consistent() {
        let endpoint = std::env::var("GLAMSTERDAM_ENDPOINT").expect("GLAMSTERDAM_ENDPOINT");
        let count = std::env::var("GLAMSTERDAM_BLOCKS")
            .ok()
            .map(|count| count.parse::<u64>().expect("a block count"))
            .unwrap_or(2);
        let programs = Programs::load_default().expect("the programs");
        let work_dir =
            std::env::temp_dir().join(format!("glamsterdam_follow_{}", std::process::id()));
        let summary = follow(&endpoint, None, Some(count), &work_dir, &programs, 3, false)
            .expect("the follower runs");
        assert!(
            summary.failed.is_empty(),
            "diverging blocks {:?}, fixtures kept under {}",
            summary.failed,
            work_dir.display()
        );
        assert_eq!(summary.reports.len() as u64, count);
        let _ = std::fs::remove_dir_all(&work_dir);
    }
}
