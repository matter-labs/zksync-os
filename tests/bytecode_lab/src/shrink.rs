//! Shrinking a finding to a smaller program with the same outcome class.
//!
//! Two reductions, in order:
//!
//! 1. Drop calls one at a time, keeping each removal that still reproduces.
//! 2. Truncate the runtime block by block. The builder makes every block start
//!    and end on an empty stack, so a prefix ending on a block boundary is
//!    itself valid bytecode; the block boundaries are recorded in the program's
//!    notes. The smallest prefix that still reproduces is kept.
//!
//! Regenerating through `program_gen`'s builder with fewer blocks would not
//! preserve the earlier blocks' bytes (each block consumes a variable number of
//! RNG draws), so truncating the finished bytes on a recorded boundary is the
//! reduction that keeps a strict, reproducing prefix.

use program_gen::{
    trace_stack, verify_jump_targets, wrap_runtime_as_initcode, CallPlan, Fork, Mode, Program,
};

use crate::runner::run_case_forward_catching;

/// Reduce `program` to the smallest program that still yields `target` (the
/// original outcome's [`crate::classify::Outcome::signature`], so a reduction
/// cannot morph a state divergence into a per-transaction mismatch or a
/// gas-schedule difference), keeping the mode, fork and per-case seed fixed so
/// signers and addresses stay stable.
pub fn shrink(
    program: &Program,
    mode: Mode,
    slow_ms: u64,
    case_seed: u64,
    target: &str,
) -> Program {
    let mut current = program.clone();

    // Phase 1: drop calls greedily. On a successful removal, do not advance the
    // index so the next call slides into the current position.
    let mut i = 0;
    while i < current.calls.len() {
        let mut candidate_calls = current.calls.clone();
        candidate_calls.remove(i);
        let candidate = with_calls(&current, candidate_calls);
        if reproduces(&candidate, mode, slow_ms, case_seed, target) {
            current = candidate;
        } else {
            i += 1;
        }
    }

    // Phase 2: keep the smallest runtime prefix (on a block boundary) that still
    // reproduces.
    for end in block_ends(&current) {
        if let Some(candidate) = with_truncated_runtime(&current, end) {
            if reproduces(&candidate, mode, slow_ms, case_seed, target) {
                current = candidate;
                break;
            }
        }
    }

    current
}

fn reproduces(candidate: &Program, mode: Mode, slow_ms: u64, case_seed: u64, target: &str) -> bool {
    run_case_forward_catching(candidate, mode, slow_ms, case_seed)
        .outcome
        .signature()
        == target
}

fn with_calls(base: &Program, calls: Vec<CallPlan>) -> Program {
    Program {
        calls,
        ..base.clone()
    }
}

/// Truncate the runtime to `end` bytes and re-wrap it as initcode, but only if
/// the prefix is still valid bytecode (stack- and jump-consistent).
fn with_truncated_runtime(base: &Program, end: usize) -> Option<Program> {
    if end == 0 || end >= base.runtime.len() {
        return None;
    }
    let runtime = base.runtime[..end].to_vec();
    trace_stack(&runtime, fork_of(base)).ok()?;
    verify_jump_targets(&runtime).ok()?;
    let initcode = wrap_runtime_as_initcode(&runtime);
    let mut notes = base.notes.clone();
    notes.push(format!("minimized: runtime truncated to {end} bytes"));
    Some(Program {
        runtime,
        initcode,
        notes,
        ..base.clone()
    })
}

fn fork_of(program: &Program) -> Fork {
    program.fork
}

/// The block-boundary end offsets recorded in the notes, ascending and
/// deduplicated. Lines look like `block <name>: runtime bytes [<start>, <end>)`.
fn block_ends(program: &Program) -> Vec<usize> {
    const MARKER: &str = "runtime bytes [";
    let mut ends = Vec::new();
    for line in &program.notes {
        let Some(idx) = line.find(MARKER) else {
            continue;
        };
        let rest = &line[idx + MARKER.len()..];
        let Some((_, end_part)) = rest.split_once(", ") else {
            continue;
        };
        let end_str: String = end_part
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if let Ok(end) = end_str.parse::<usize>() {
            ends.push(end);
        }
    }
    ends.sort_unstable();
    ends.dedup();
    ends
}
