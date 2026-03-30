#![cfg_attr(target_arch = "riscv32", no_std)]

//! Markers to capture basic RISC-V simulator measurements for
//! a block of rust code.
//!
//! Should be used through the macro:
//!
//! cycle_marker::wrap!("label", {your code});
//!
//! For gas model we include a helper that will also log ergs consumed
//!
//! cycle_marker::wrap_with_resources("label", resources, {your code});
//!
//! zksync_os binary has to be built using `dump_bin_with_markers.sh`
//! and tests need to enable the `cycle_marker` feature.
//!
//! Traces are dumped to a file, the path can be set using the
//! MARKER_PATH environment variable.
//!
//! ## Per-opcode cycle tracking
//!
//! For EVM opcode benchmarking, use the opcode-specific macros:
//!
//! cycle_marker::opcode_start!();
//! // ... opcode execution ...
//! cycle_marker::opcode_end!(label);
//!
//! These are separated from block-level markers and aggregated by label
//! in `print_cycle_markers`. On RISC-V they use the same CSR mechanism.

/// Labels can be at the start or end of a code block.
/// OpcodeStart/OpcodeEnd are for per-EVM-opcode cycle tracking —
/// they use the same CSR mechanism on RISC-V but are aggregated
/// (summed by label) rather than matched individually.
#[allow(dead_code)]
enum Label {
    Start(&'static str),
    End(&'static str),
    OpcodeStart,
    OpcodeEnd(&'static str),
}

#[cfg(not(target_arch = "riscv32"))]
thread_local! {
  /// Forward run collects the labels, so that we don't incur in more RISC-V cycles
  static LABELS: std::cell::RefCell<Vec<Label>> = const { std::cell::RefCell::new(Vec::new()) };

  #[cfg(feature="log_to_file")]
  static MARKER_FILE: std::cell::RefCell<std::fs::File> = std::cell::RefCell::new(init_marker_file());
}

#[allow(dead_code)]
#[cfg(not(target_arch = "riscv32"))]
fn init_marker_file() -> std::fs::File {
    let path = std::env::var("MARKER_PATH").unwrap_or("markers.bench".to_string());
    std::fs::File::create(path).expect("Failed to create marker file")
}

#[allow(dead_code)]
#[cfg(all(not(feature = "log_to_file"), not(target_arch = "riscv32")))]
pub fn log_marker(_msg: &str) {}

#[cfg(all(feature = "log_to_file", not(target_arch = "riscv32")))]
pub fn log_marker(msg: &str) {
    use std::io::Write;
    MARKER_FILE.with(|f| {
        writeln!(f.borrow_mut(), "{}", msg).unwrap();
    });
}

/// Start a marker. For RISC-V this will use a special CSR to
/// let the simulator know that we need a new marker.
/// For forward run this will just collect the label.
pub fn start(_label: &'static str) {
    #[cfg(target_arch = "riscv32")]
    {
        unsafe {
            let word = 0;
            core::arch::asm!(
                "csrrw x0, 0x7ff, {rd}",
                rd = in(reg) word,
                options(nomem, nostack, preserves_flags)
            )
        }
    }

    #[cfg(not(target_arch = "riscv32"))]
    LABELS.with_borrow_mut(|v| v.push(Label::Start(_label)))
}

/// End a marker. For RISC-V this will use a special CSR to
/// let the simulator know that we need a new marker.
/// For forward run this will just collect the label.
pub fn end(_label: &'static str) {
    #[cfg(target_arch = "riscv32")]
    {
        unsafe {
            let word = 0;
            core::arch::asm!(
                "csrrw x0, 0x7ff, {rd}",
                rd = in(reg) word,
                options(nomem, nostack, preserves_flags)
            )
        }
    }

    #[cfg(not(target_arch = "riscv32"))]
    LABELS.with_borrow_mut(|v| v.push(Label::End(_label)))
}

/// Start an opcode-level cycle marker. Uses the same CSR mechanism as `start()`
/// on RISC-V. On the host side, recorded as `OpcodeStart` for aggregation.
pub fn start_opcode() {
    #[cfg(target_arch = "riscv32")]
    {
        // SAFETY: CSR write to simulator-intercepted register 0x7ff. No memory access,
        // no stack effect, flags preserved. Mirrors the pattern in `start()`/`end()`.
        unsafe {
            let word = 0;
            core::arch::asm!(
                "csrrw x0, 0x7ff, {rd}",
                rd = in(reg) word,
                options(nomem, nostack, preserves_flags)
            )
        }
    }

    #[cfg(not(target_arch = "riscv32"))]
    LABELS.with_borrow_mut(|v| v.push(Label::OpcodeStart))
}

/// End an opcode-level cycle marker. Uses the same CSR mechanism as `end()`
/// on RISC-V. On the host side, recorded as `OpcodeEnd` with label for aggregation.
pub fn end_opcode(_label: &'static str) {
    #[cfg(target_arch = "riscv32")]
    {
        // SAFETY: CSR write to simulator-intercepted register 0x7ff. No memory access,
        // no stack effect, flags preserved. Mirrors the pattern in `start()`/`end()`.
        unsafe {
            let word = 0;
            core::arch::asm!(
                "csrrw x0, 0x7ff, {rd}",
                rd = in(reg) word,
                options(nomem, nostack, preserves_flags)
            )
        }
    }

    #[cfg(not(target_arch = "riscv32"))]
    LABELS.with_borrow_mut(|v| v.push(Label::OpcodeEnd(_label)))
}

#[macro_export]
macro_rules! start {
    ($label:expr) => {
        #[cfg(feature = "cycle_marker")]
        {
            $crate::start($label);
        }
    };
}

#[macro_export]
macro_rules! end {
    ($label:expr) => {
        #[cfg(feature = "cycle_marker")]
        {
            $crate::end($label);
        }
    };
}

#[macro_export]
macro_rules! opcode_start {
    () => {
        #[cfg(feature = "cycle_marker")]
        {
            $crate::start_opcode();
        }
    };
}

#[macro_export]
macro_rules! opcode_end {
    ($label:expr) => {
        #[cfg(feature = "cycle_marker")]
        {
            $crate::end_opcode($label);
        }
    };
}

#[macro_export]
macro_rules! wrap {
    ($label:expr, $code:block) => {{
        $crate::start!($label);
        let __result = (|| $code)();
        $crate::end!($label);
        __result
    }};
}

#[macro_export]
macro_rules! wrap_with_resources {
    ($label:expr, $resources:expr, $code:block) => {{
        #[cfg(not(target_arch = "riscv32"))]
        {
            use alloc::format;
            let resources_before = $resources.clone();
            $crate::start!($label);
            let __result = (|| $code)();
            $crate::end!($label);
            use zk_ee::system::resources::Resource;

            let spent_resources = resources_before.diff($resources.clone());
            cycle_marker::log_marker(&format!(
                "Spent ergs for [{}]: {:?}\n",
                $label,
                spent_resources.ergs().0
            ));
            use zk_ee::system::Computational;
            cycle_marker::log_marker(&format!(
                "Spent native for [{}]: {}\n",
                $label,
                spent_resources.native().as_u64()
            ));
            __result
        }
        #[cfg(target_arch = "riscv32")]
        {
            $crate::start!($label);
            let __result = (|| $code)();
            $crate::end!($label);
            __result
        }
    }};
}

// Snapshotting mechanism, used for tests
// We run multiple native runs of the program, so labels can be duplicated.
// This is a way to ignore some of those side effects.
#[cfg(not(target_arch = "riscv32"))]
pub struct Snapshot {
    labels_len: usize,
    #[cfg(feature = "log_to_file")]
    file_len: u64,
}

#[cfg(target_arch = "riscv32")]
pub struct Snapshot;

#[cfg(not(target_arch = "riscv32"))]
pub fn snapshot() -> Snapshot {
    let labels_len = LABELS.with(|l| l.borrow().len());

    #[cfg(feature = "log_to_file")]
    let file_len = MARKER_FILE.with(|f| {
        use std::io::Seek;
        use std::io::SeekFrom;

        let mut file = f.borrow_mut();
        // Get current position; writing always appends, so this is effectively the "index"
        file.seek(SeekFrom::Current(0))
            .expect("Failed to seek marker file")
    });

    Snapshot {
        labels_len,
        #[cfg(feature = "log_to_file")]
        file_len,
    }
}

#[cfg(target_arch = "riscv32")]
pub fn snapshot() -> Snapshot {
    Snapshot
}

#[cfg(not(target_arch = "riscv32"))]
pub fn revert(snap: Snapshot) {
    // Restore LABELS length
    LABELS.with(|l| {
        let mut v = l.borrow_mut();
        if v.len() > snap.labels_len {
            v.truncate(snap.labels_len);
        }
    });

    // Restore file length/position if logging to file
    #[cfg(feature = "log_to_file")]
    {
        use std::io::{Seek, SeekFrom};

        MARKER_FILE.with(|f| {
            let mut file = f.borrow_mut();
            file.set_len(snap.file_len)
                .expect("Failed to truncate marker file");
            file.seek(SeekFrom::Start(snap.file_len))
                .expect("Failed to seek marker file");
        });
    }
}

#[cfg(target_arch = "riscv32")]
pub fn revert(_: Snapshot) {}

/// Per-opcode aggregated cycle statistics.
#[cfg(all(feature = "use_risc_v_simulator", not(target_arch = "riscv32")))]
#[derive(Debug, Clone)]
pub struct OpcodeCycleStats {
    pub name: &'static str,
    pub total_cycles: u64,
    pub count: u64,
    pub min_cycles: u64,
    pub max_cycles: u64,
    pub median_cycles: u64,
}

/// Results from processing cycle markers.
#[cfg(all(feature = "use_risc_v_simulator", not(target_arch = "riscv32")))]
pub struct CycleMarkerResults {
    pub block_effective: Option<u64>,
    pub opcode_cycle_stats: Vec<OpcodeCycleStats>,
}

#[cfg(all(feature = "use_risc_v_simulator", not(target_arch = "riscv32")))]
pub fn print_cycle_markers() -> CycleMarkerResults {
    const BLAKE_DELEGATION_ID: u32 = 1991;
    const BIGINT_DELEGATION_ID: u32 = 1994;
    const BLAKE_DELEGATION_COEFF: u64 = 16;
    const BIGINT_DELEGATION_COEFF: u64 = 4;
    const BLOCK_WIDE_LABEL: &str = "process_block";
    use risc_v_simulator::cycle::state::*;
    let cm = take_cycle_marker();
    let labels = LABELS.with(|l| std::mem::take(&mut *l.borrow_mut()));
    use std::collections::HashMap;

    assert_eq!(cm.markers.len(), labels.len());

    // Block-level markers: use existing nonce-based matching
    let mut label_nonces: HashMap<&'static str, u64> = HashMap::new();
    let mut marker_map: HashMap<(&'static str, u64), (Mark, Mark)> = HashMap::new();
    let mut start_counts: HashMap<(&'static str, u64), Mark> = HashMap::new();

    // Opcode-level markers: aggregate by label name.
    // OpcodeStart/OpcodeEnd pairs are always adjacent (leaf-level, no nesting).
    struct OpcodeAcc {
        total: u64,
        count: u64,
        min: u64,
        max: u64,
        samples: Vec<u64>,
    }
    impl OpcodeAcc {
        fn new() -> Self {
            Self {
                total: 0,
                count: 0,
                min: u64::MAX,
                max: 0,
                samples: Vec::new(),
            }
        }
        fn record(&mut self, cycles: u64) {
            self.total += cycles;
            self.count += 1;
            self.min = self.min.min(cycles);
            self.max = self.max.max(cycles);
            self.samples.push(cycles);
        }
        fn median(&mut self) -> u64 {
            if self.samples.is_empty() {
                return 0;
            }
            self.samples.sort_unstable();
            let mid = self.samples.len() / 2;
            if self.samples.len() % 2 == 0 {
                ((self.samples[mid - 1] as u128 + self.samples[mid] as u128) / 2) as u64
            } else {
                self.samples[mid]
            }
        }
    }
    let mut opcode_aggregated: HashMap<&'static str, OpcodeAcc> = HashMap::new();
    let mut pending_opcode_start: Option<Mark> = None;

    log_marker("\n=== Cycle markers:");
    for (label, mark) in labels.into_iter().zip(cm.markers.into_iter()) {
        match label {
            Label::OpcodeStart => {
                debug_assert!(
                    pending_opcode_start.is_none(),
                    "Consecutive OpcodeStart without OpcodeEnd — previous start marker lost"
                );
                pending_opcode_start = Some(mark);
            }
            Label::OpcodeEnd(name) => {
                if let Some(start_mark) = pending_opcode_start.take() {
                    let diff = mark.diff(&start_mark);
                    opcode_aggregated
                        .entry(name)
                        .or_insert_with(OpcodeAcc::new)
                        .record(diff.cycles);
                }
            }
            Label::Start(name) => {
                let nonce = label_nonces
                    .entry(name)
                    .and_modify(|n| *n += 1)
                    .or_insert(0);
                start_counts.insert((name, *nonce), mark);
            }
            Label::End(name) => {
                // Assuming markers with same name don't overlap
                let nonce = label_nonces.get(name).unwrap();
                if let Some(start_count) = start_counts.remove(&(name, *nonce)) {
                    marker_map.insert((name, *nonce), (start_count, mark));
                } else {
                    eprintln!("Warning: end label '{}', {} has no start", name, nonce);
                }
            }
        }
    }
    for ((name, _), _) in &start_counts {
        eprintln!("Warning: start label '{}' has no end", name);
    }
    let mut markers: Vec<(&'static str, (Mark, Mark))> = marker_map
        .into_iter()
        .map(|((label, _), value)| (label, value))
        .collect();
    markers.sort_by_key(|(_, (start, _))| start.cycles);

    let mut block_effective: Option<u64> = None;

    for (label, (start, end)) in markers {
        let diff = end.diff(&start);
        log_marker(&format!(
            "{}: net cycles: {}, net delegations: {:?}",
            label, diff.cycles, diff.delegations
        ));
        if label == BLOCK_WIDE_LABEL {
            block_effective = Some(
                diff.cycles
                    + BLAKE_DELEGATION_COEFF
                        * diff
                            .delegations
                            .get(&BLAKE_DELEGATION_ID)
                            .cloned()
                            .unwrap_or_default()
                    + BIGINT_DELEGATION_COEFF
                        * diff
                            .delegations
                            .get(&BIGINT_DELEGATION_ID)
                            .cloned()
                            .unwrap_or_default(),
            )
        }
    }
    log_marker(&format!(
        "Total delegations: {:?}\n==================",
        cm.delegation_counter
    ));

    // Dump per-execution cycle samples if requested via env var
    if let Ok(dir) = std::env::var("OPCODE_CYCLE_SAMPLES_DIR") {
        let dir = std::path::Path::new(&dir);
        std::fs::create_dir_all(dir).expect("Failed to create cycle samples dir");
        for (name, acc) in &opcode_aggregated {
            if acc.samples.is_empty() {
                continue;
            }
            let path = dir.join(format!("{}.cycles", name));
            let mut f = std::fs::File::create(path).expect("Failed to create cycle samples file");
            use std::io::Write;
            for &c in &acc.samples {
                writeln!(f, "{}", c).expect("Failed to write cycle sample");
            }
        }
    }

    // Collect and sort opcode stats
    let mut opcode_cycle_stats: Vec<OpcodeCycleStats> = opcode_aggregated
        .into_iter()
        .map(|(name, mut acc)| {
            let median = acc.median();
            OpcodeCycleStats {
                name,
                total_cycles: acc.total,
                count: acc.count,
                min_cycles: if acc.count > 0 { acc.min } else { 0 },
                max_cycles: acc.max,
                median_cycles: median,
            }
        })
        .collect();
    opcode_cycle_stats.sort_by(|a, b| b.total_cycles.cmp(&a.total_cycles));

    if !opcode_cycle_stats.is_empty() {
        log_marker("\n=== Per-opcode cycle stats:");
        log_marker(&format!(
            "{:<20} {:>12} {:>14} {:>10} {:>10} {:>10} {:>10}",
            "opcode", "count", "total_cycles", "avg", "median", "min", "max"
        ));
        for stat in &opcode_cycle_stats {
            let avg = stat.total_cycles as f64 / stat.count as f64;
            log_marker(&format!(
                "{:<20} {:>12} {:>14} {:>10.1} {:>10} {:>10} {:>10}",
                stat.name,
                stat.count,
                stat.total_cycles,
                avg,
                stat.median_cycles,
                stat.min_cycles,
                stat.max_cycles
            ));
        }
        log_marker("==================");
    }

    CycleMarkerResults {
        block_effective,
        opcode_cycle_stats,
    }
}
