//! A deterministic generator of EVM bytecode programs for differential and
//! stress testing.
//!
//! The crate has no dependency on any part of the virtual machine: it only
//! emits programs and describes them. Give [`generate`] a seed, a fork and a
//! mode and it returns a [`Program`]: the runtime code, the initcode that
//! deploys it, a set of calls to make against the deployed contract, and notes
//! that spell out, in plain words, what each part guarantees. The same seed,
//! fork and mode always produce byte-identical output, so a case can be shared
//! as a small seed and reproduced anywhere.
//!
//! The pieces:
//!
//! * [`opcodes`] is the per-fork table of opcodes and precompiles.
//! * [`builder`] emits bytecode while keeping the stack valid and every jump
//!   pointed at a real JUMPDEST, and it carries the replayers that prove those
//!   properties from the finished bytes.
//! * [`blocks`] is the library of building blocks a program is assembled from.
//! * [`initcode`] wraps a runtime program as deploying initcode.
//! * [`modes`] defines the generation modes and their address restrictions.

pub mod blocks;
pub mod builder;
pub mod initcode;
pub mod modes;
pub mod opcodes;

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

pub use builder::{
    trace_stack, verify_jump_targets, BlockBoundary, JumpViolation, ProgramBuilder, StackReport,
    StackViolation,
};
pub use initcode::wrap_runtime_as_initcode;
pub use modes::Mode;
pub use opcodes::{Fork, Precompile};

/// The largest runtime code the interpreter will accept, mirrored here because
/// this crate does not depend on the interpreter (it matches
/// `evm_interpreter::MAX_CODE_SIZE`).
pub const MAX_RUNTIME_CODE_SIZE: usize = 0x6000;

/// The largest initcode the interpreter will accept (it matches
/// `evm_interpreter::MAX_INITCODE_SIZE`, which is twice the code size).
pub const MAX_INITCODE_SIZE: usize = MAX_RUNTIME_CODE_SIZE * 2;

/// A margin left free while adding body blocks, so the final terminal block and
/// any padding always fit under the runtime size limit. It comfortably exceeds
/// the size of the largest single block.
const RUNTIME_SIZE_BUDGET: usize = MAX_RUNTIME_CODE_SIZE - 4096;

/// One call to make against the deployed program: the calldata to send, the
/// value in wei, and the gas limit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallPlan {
    /// Raw calldata. Serialized as a hex string in case files.
    #[serde(with = "hex::serde")]
    pub calldata: Vec<u8>,
    /// Value to send with the call, in wei.
    pub value: u128,
    /// Gas limit for the call.
    pub gas: u64,
}

/// A generated program and everything needed to deploy, call and understand it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Program {
    /// The execution surface this program was generated for.
    pub fork: Fork,
    /// The restrictions the generator worked under.
    pub mode: Mode,
    /// The deployed runtime code. Serialized as a hex string.
    #[serde(with = "hex::serde")]
    pub runtime: Vec<u8>,
    /// Initcode that deploys `runtime`. Serialized as a hex string.
    #[serde(with = "hex::serde")]
    pub initcode: Vec<u8>,
    /// Calls to make against the deployed program.
    pub calls: Vec<CallPlan>,
    /// Human-readable notes: the fork and mode, one line per block with its byte
    /// range, and the properties each block guarantees. A shrinker can read the
    /// block ranges to truncate the program a block at a time.
    pub notes: Vec<String>,
}

impl Program {
    /// Serialize to pretty JSON for a case file.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("a Program is always serializable")
    }

    /// Parse a program back from JSON.
    pub fn from_json(text: &str) -> serde_json::Result<Self> {
        serde_json::from_str(text)
    }
}

/// Generate a program.
///
/// A single ChaCha8 stream seeded from `seed` drives every choice, so the output
/// is fully determined by `(seed, fork, mode)`. The builder runs on its own
/// stream derived from the master one, which keeps call planning stable even if
/// a block's internal draws change.
pub fn generate(seed: u64, fork: Fork, mode: Mode) -> Program {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    // Decide the overall shape first, from the master stream.
    let body_block_count = rng.random_range(3..9);
    let call_count = rng.random_range(1..5);
    let builder_seed = rng.random::<u64>();

    let mut builder = ProgramBuilder::new(fork, mode, ChaCha8Rng::seed_from_u64(builder_seed));
    builder.note(format!("fork={fork:?} mode={mode:?}"));
    builder.note(
        "runtime byte ranges below are offsets into the runtime code, before initcode wrapping",
    );

    for _ in 0..body_block_count {
        // Stop early if we are close to the size limit; a body block is always
        // smaller than the reserved margin.
        if builder.code_len() > RUNTIME_SIZE_BUDGET {
            break;
        }
        blocks::append_body_block(&mut builder);
    }
    // Always end with a terminal block so execution halts cleanly.
    blocks::append_terminal_block(&mut builder);

    let (runtime, _boundaries, mut notes) = builder.finish();
    let initcode = wrap_runtime_as_initcode(&runtime);

    // These hold by construction; assert so any future change that breaks the
    // size discipline fails loudly rather than producing invalid cases.
    assert!(
        runtime.len() <= MAX_RUNTIME_CODE_SIZE,
        "runtime code exceeded the size limit"
    );
    assert!(
        initcode.len() <= MAX_INITCODE_SIZE,
        "initcode exceeded the size limit"
    );

    let calls = plan_calls(&mut rng, call_count);
    notes.push(format!(
        "calls: {} planned against the deployed program",
        calls.len()
    ));

    Program {
        fork,
        mode,
        runtime,
        initcode,
        calls,
        notes,
    }
}

/// Draw a set of calls to make against the deployed program.
fn plan_calls(rng: &mut ChaCha8Rng, count: usize) -> Vec<CallPlan> {
    let mut calls = Vec::with_capacity(count);
    for _ in 0..count {
        // Mostly short calldata, occasionally a few hundred bytes.
        let len = if rng.random_range(0..5) == 0 {
            rng.random_range(0..257)
        } else {
            rng.random_range(0..65)
        };
        let mut calldata = vec![0u8; len];
        for byte in calldata.iter_mut() {
            *byte = rng.random::<u8>();
        }
        // Value is usually zero; sometimes a small amount.
        let value = if rng.random_range(0..4) == 0 {
            rng.random_range(0..1_000_000u128)
        } else {
            0
        };
        let gas = rng.random_range(100_000..30_000_001u64);
        calls.push(CallPlan {
            calldata,
            value,
            gas,
        });
    }
    calls
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODES: [Mode; 4] = [
        Mode::GasAgnostic,
        Mode::HookFree,
        Mode::Unrestricted,
        Mode::Delegation7702,
    ];
    const FORKS: [Fork; 2] = [Fork::Osaka, Fork::CancunWith7702];

    #[test]
    fn generated_programs_are_stack_valid() {
        // A thousand programs per mode: five hundred seeds across the two forks.
        // The replay confirms the stack never underflows and never passes 1024,
        // and it uses the fork's own opcode set, so a stray fork-illegal opcode
        // (for example CLZ on the older fork) would fail here too.
        for mode in MODES {
            let mut checked = 0;
            for fork in FORKS {
                for seed in 0..500u64 {
                    let program = generate(seed, fork, mode);
                    let report = trace_stack(&program.runtime, fork).unwrap_or_else(|e| {
                        panic!("stack invalid for seed {seed} fork {fork:?} mode {mode:?}: {e:?}")
                    });
                    assert!(report.max_depth <= builder::STACK_LIMIT);
                    checked += 1;
                }
            }
            assert_eq!(checked, 1000, "expected a thousand programs for {mode:?}");
        }
    }

    #[test]
    fn every_jump_targets_a_jumpdest() {
        for mode in MODES {
            for fork in FORKS {
                for seed in 0..500u64 {
                    let program = generate(seed, fork, mode);
                    verify_jump_targets(&program.runtime).unwrap_or_else(|e| {
                        panic!("bad jump for seed {seed} fork {fork:?} mode {mode:?}: {e:?}")
                    });
                    // The initcode is not checked for jump consistency: its
                    // preamble has no jumps, and the runtime bytes appended after
                    // the preamble's RETURN are data to be copied out, not code
                    // the initcode executes.
                }
            }
        }
    }

    #[test]
    fn the_same_seed_gives_byte_identical_programs() {
        for mode in MODES {
            for fork in FORKS {
                for seed in [0u64, 1, 2, 7, 42, 1000, u64::MAX] {
                    let a = generate(seed, fork, mode);
                    let b = generate(seed, fork, mode);
                    assert_eq!(
                        a, b,
                        "nondeterministic for seed {seed} fork {fork:?} mode {mode:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn code_sizes_stay_within_the_limits() {
        for mode in MODES {
            for fork in FORKS {
                for seed in 0..500u64 {
                    let program = generate(seed, fork, mode);
                    assert!(
                        program.runtime.len() <= MAX_RUNTIME_CODE_SIZE,
                        "runtime too large: {}",
                        program.runtime.len()
                    );
                    assert!(
                        program.initcode.len() <= MAX_INITCODE_SIZE,
                        "initcode too large: {}",
                        program.initcode.len()
                    );
                }
            }
        }
    }

    #[test]
    fn gas_agnostic_programs_never_read_gas() {
        // The GAS opcode must not appear as an instruction (push data does not
        // count) in either mode whose soundness depends on being gas-agnostic:
        // plain gas-agnostic and the 7702 delegation mode.
        for mode in [Mode::GasAgnostic, Mode::Delegation7702] {
            for fork in FORKS {
                for seed in 0..500u64 {
                    let program = generate(seed, fork, mode);
                    assert!(
                        !contains_opcode(&program.runtime, opcodes::GAS),
                        "gas-agnostic program read GAS for seed {seed} fork {fork:?} mode {mode:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn json_round_trips() {
        let program = generate(123, Fork::Osaka, Mode::Unrestricted);
        let text = program.to_json();
        let parsed = Program::from_json(&text).unwrap();
        assert_eq!(program, parsed);
    }

    #[test]
    fn notes_record_block_boundaries() {
        let program = generate(5, Fork::Osaka, Mode::Unrestricted);
        let boundary_lines = program
            .notes
            .iter()
            .filter(|line| line.starts_with("block ") && line.contains("runtime bytes ["))
            .count();
        assert!(boundary_lines >= 2, "expected block boundary notes");
    }

    /// Scan `code` as instructions (skipping push immediates) and report whether
    /// `wanted` appears as a real opcode.
    fn contains_opcode(code: &[u8], wanted: u8) -> bool {
        let mut pc = 0;
        while pc < code.len() {
            let byte = code[pc];
            if byte == wanted {
                return true;
            }
            let imm = opcodes::lookup(byte)
                .map(|s| s.immediate_bytes as usize)
                .unwrap_or(0);
            pc += 1 + imm;
        }
        false
    }
}
