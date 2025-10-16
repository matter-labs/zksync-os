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

/// Labels can be at the start or end of a code block
#[allow(dead_code)]
enum Label {
    Start(&'static str),
    End(&'static str),
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

#[cfg(all(feature = "use_risc_v_simulator", not(target_arch = "riscv32")))]
pub fn print_cycle_markers() -> Option<u64> {
    const BLAKE_DELEGATION_ID: u32 = 1991;
    const BIGINT_DELEGATION_ID: u32 = 1994;
    const BLAKE_DELEGATION_COEFF: u64 = 16;
    const BIGINT_DELEGATION_COEFF: u64 = 4;
    const BLOCK_WIDE_LABEL: &str = "run_prepared";
    use risc_v_simulator::cycle::state::*;
    let cm = take_cycle_marker();
    let labels = LABELS.with(|l| std::mem::take(&mut *l.borrow_mut()));
    use std::collections::HashMap;

    assert_eq!(cm.markers.len(), labels.len());

    let mut label_nonces: HashMap<&'static str, u64> = HashMap::new();
    let mut marker_map: HashMap<(&'static str, u64), (Mark, Mark)> = HashMap::new();
    let mut start_counts: HashMap<(&'static str, u64), Mark> = HashMap::new();

    log_marker("\n=== Cycle markers:");
    for (label, mark) in labels.into_iter().zip(cm.markers.into_iter()) {
        match label {
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
    for ((name, _), _) in start_counts {
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
            // We compute effective cycles for the block execution.
            // That is: raw cycles plus the delegation counts, weighted by
            // the delegation coefficients (derived from the circuits
            // geometry)
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
    block_effective
}
