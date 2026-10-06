#![cfg(test)]

//! PUSH2 followed by JUMP or JUMPI on the Ethereum STF's resources. For gas-only resources
//! (`Resources::ERGS_ONLY`) the proving target runs such a pair as one step (`Hot::push2` in the
//! interpreter); the host dispatches both, unless it is built with `--cfg evm_fuse_push2_jumps`.
//! The `evm` instances' `push2_jump_fusion` runs the ZKsync OS STF, which never fuses, so these
//! cases are the ones that run the fused pair: its charge boundaries, invalid destinations, the
//! untaken JUMPI (whose destination is not checked), full stacks, a PUSH2 at the end of the code,
//! budgets whose low 32-bit word can't pay a charge (the low-word charge gives up and the
//! original one runs) and loops.
//!
//! The frames run on the Ethereum proving system types, through the harness of
//! `eth_gas_budgets`. Each case pins how the frame ends, with values that must hold both with and
//! without the cfg: the gas left and the return data, or the EVM error with the `ip` and the
//! stack (bottom first) that the opcode error hook sees. They run on the host only: the host
//! dispatches the JUMPDEST a jump lands on, so a landing short of gas ends past that JUMPDEST
//! either way (the proving target charges it in the jump, `Hot::land_on_jumpdest`). Run both:
//!
//! ```text
//! cargo test -p unit eth_push2_jump_fusion
//! RUSTFLAGS="--cfg evm_fuse_push2_jumps" CARGO_TARGET_DIR=target/fuse \
//!     cargo test -p unit eth_push2_jump_fusion
//! ```
//!
//! With the cfg the tracer sees a fused pair as one PUSH2 step: the JUMP or JUMPI step is
//! missing and its gas goes to the PUSH2. So the cases don't look at the steps (and
//! `eth_gas_budgets`, which does, holds only without the cfg), except `the_cfg_fuses_the_pair`,
//! which counts them to check that the cfg reached the interpreter.

use crate::eth_gas_budgets::{run_with_tracer, EthProvingTypes};
use rig::evm_interpreter::gas_constants::{BASE, HIGH, JUMPDEST, MEMORY, MID, VERYLOW};
use rig::ruint::aliases::{B160, U256};
use rig::zk_ee::execution_environment_type::ExecutionEnvironmentType;
use rig::zk_ee::system::evm::{EvmError, EvmFrameInterface, EvmStackInterface};
use rig::zk_ee::system::tracer::evm_tracer::EvmTracer;
use rig::zk_ee::system::tracer::Tracer;
use rig::zk_ee::system::{CallResult, ExecutionEnvironmentLaunchParams, SystemTypes};
use rig::zk_ee::utils::Bytes32;

const LOW_WORD: u64 = 1 << 32;
const SIGN: u64 = 1 << 31;

/// Largest ergs (`Ergs::MAX_ERGS`)
const MAX_ERGS: u64 = i64::MAX as u64;

/// How the frame ended
#[derive(Debug, PartialEq, Eq)]
enum End {
    /// It stopped or returned, with this gas left and return data
    Success { left: u64, data: Vec<u8> },
    /// It failed with this EVM error, at `ip` and with this stack (bottom first, as limbs)
    Error {
        error: EvmError,
        ip: usize,
        stack: Vec<[u64; 4]>,
    },
}

/// Counts the steps, and records the end of a failed frame
#[derive(Default)]
struct EndTracer {
    steps: usize,
    error: Option<(EvmError, usize, Vec<[u64; 4]>)>,
}

impl EvmTracer<EthProvingTypes> for EndTracer {
    fn before_evm_interpreter_execution_step(
        &mut self,
        _opcode: u8,
        _frame_state: &impl EvmFrameInterface<EthProvingTypes>,
    ) {
        self.steps += 1;
    }

    fn after_evm_interpreter_execution_step(
        &mut self,
        _opcode: u8,
        _frame_state: &impl EvmFrameInterface<EthProvingTypes>,
    ) {
    }

    fn on_opcode_error(
        &mut self,
        error: &EvmError,
        frame_state: &impl EvmFrameInterface<EthProvingTypes>,
    ) {
        assert!(self.error.is_none(), "one frame, one error");
        let stack = frame_state.stack().to_slice();
        self.error = Some((
            error.clone(),
            frame_state.instruction_pointer(),
            stack.iter().map(|v| *v.as_limbs()).collect(),
        ));
    }

    fn on_call_error(&mut self, _error: &EvmError) {}

    fn on_selfdestruct(
        &mut self,
        _beneficiary: B160,
        _token_value: U256,
        _frame_state: &impl EvmFrameInterface<EthProvingTypes>,
    ) {
    }

    fn on_create_request(&mut self, _is_create2: bool) {}
}

impl Tracer<EthProvingTypes> for EndTracer {
    fn evm_tracer(&mut self) -> &mut impl EvmTracer<EthProvingTypes> {
        self
    }

    fn on_new_execution_frame(
        &mut self,
        _request: &ExecutionEnvironmentLaunchParams<EthProvingTypes>,
    ) {
    }

    fn after_execution_frame_completed(
        &mut self,
        _result: Option<(
            &<EthProvingTypes as SystemTypes>::Resources,
            &CallResult<EthProvingTypes>,
        )>,
    ) {
    }

    fn on_storage_read(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        _is_transient: bool,
        _address: B160,
        _key: Bytes32,
        _value: Bytes32,
    ) {
    }

    fn on_storage_write(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        _is_transient: bool,
        _address: B160,
        _key: Bytes32,
        _value: Bytes32,
    ) {
    }

    fn on_bytecode_change(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        _address: B160,
        _new_raw_bytecode: Option<&[u8]>,
        _new_internal_bytecode_hash: Bytes32,
        _new_observable_bytecode_length: u32,
    ) {
    }

    fn on_event(
        &mut self,
        _ee_type: ExecutionEnvironmentType,
        _address: &B160,
        _topics: &[Bytes32],
        _data: &[u8],
    ) {
    }

    fn begin_tx(&mut self, _calldata: &[u8]) {}

    fn finish_tx(&mut self) {}
}

/// Runs `code` as the entry frame with `budget` gas: how it ended, and the steps the tracer saw
fn run_counted(code: &[u8], budget: u64) -> (End, usize) {
    let mut tracer = EndTracer::default();
    let (result, left, data) = run_with_tracer(code, &[], &[], budget, &mut tracer);
    let end = match (result, tracer.error) {
        ("success", None) => End::Success { left, data },
        ("failed", Some((error, ip, stack))) => {
            // an EVM error burns the rest and returns nothing
            assert_eq!((left, data.len()), (0, 0), "{error:?}");
            End::Error { error, ip, stack }
        }
        (result, error) => panic!("{result} with {error:?}"),
    };
    (end, tracer.steps)
}

fn run(code: &[u8], budget: u64) -> End {
    run_counted(code, budget).0
}

fn stop(left: u64) -> End {
    End::Success { left, data: vec![] }
}

fn failed(error: EvmError, ip: usize, stack: &[u64]) -> End {
    End::Error {
        error,
        ip,
        stack: stack.iter().map(|&x| [x, 0, 0, 0]).collect(),
    }
}

fn out_of_gas(ip: usize, stack: &[u64]) -> End {
    failed(EvmError::OutOfGas, ip, stack)
}

/// `n` zeros, then `top`
fn on_zeros(n: usize, top: &[u64]) -> Vec<u64> {
    let mut stack = vec![0; n];
    stack.extend_from_slice(top);
    stack
}

/// A charge of `gas`, and the `ip` and stack the frame ends with if it can't be paid
struct Charge {
    gas: u64,
    ip: usize,
    stack: Vec<u64>,
}

fn charge(gas: u64, ip: usize, stack: &[u64]) -> Charge {
    Charge {
        gas,
        ip,
        stack: stack.to_vec(),
    }
}

/// Runs `code`, which spends `before` gas and then makes `charges` and stops, with every budget
/// from `before` to one past the total: out of gas at the first charge the rest can't pay,
/// otherwise stopped with what is left
fn check_charges(code: &[u8], before: u64, charges: &[Charge]) {
    let total = before + charges.iter().map(|c| c.gas).sum::<u64>();
    for budget in before..=total + 1 {
        let mut paid = before;
        let expected = charges
            .iter()
            .find_map(|c| {
                paid += c.gas;
                (paid > budget).then(|| out_of_gas(c.ip, &c.stack))
            })
            .unwrap_or_else(|| stop(budget - total));
        assert_eq!(run(code, budget), expected, "budget {budget}");
    }
}

/// `PUSH1 byte PUSH0 MSTORE8 PUSH1 1 PUSH0 RETURN`: returns `[byte]`
fn return_byte(byte: u8) -> [u8; 8] {
    [0x60, byte, 0x5f, 0x53, 0x60, 0x01, 0x5f, 0xf3]
}

/// Gas of `return_byte`, with the growth of the heap to one word
const RETURN_BYTE: u64 = VERYLOW + BASE + (VERYLOW + MEMORY) + VERYLOW + BASE;

/// `PUSH2 4 JUMP JUMPDEST STOP` with every budget up to its cost: out of gas at the PUSH2, at the
/// JUMP (with the destination on the stack, as the dispatched PUSH2 has pushed it: at 3 gas the
/// PUSH2 is paid and the JUMP gets nothing), at the landing, or stopped
#[test]
fn jump_charge_boundaries() {
    let code = [0x61, 0x00, 0x04, 0x56, 0x5b, 0x00];
    check_charges(
        &code,
        0,
        &[
            charge(VERYLOW, 1, &[]),
            charge(MID, 4, &[4]),
            charge(JUMPDEST, 5, &[]),
        ],
    );
    assert_eq!(run(&code, 1000), stop(1000 - VERYLOW - MID - JUMPDEST));
}

/// PUSH2 JUMPI with every budget up to its cost, taken and not taken
#[test]
fn jumpi_charge_boundaries() {
    // PUSH1 1 PUSH2 7 JUMPI STOP JUMPDEST STOP
    let taken = [0x60, 0x01, 0x61, 0x00, 0x07, 0x57, 0x00, 0x5b, 0x00];
    check_charges(
        &taken,
        0,
        &[
            charge(VERYLOW, 1, &[]),
            charge(VERYLOW, 3, &[1]),
            charge(HIGH, 6, &[1, 7]),
            charge(JUMPDEST, 8, &[]),
        ],
    );
    // PUSH0 PUSH2 6 JUMPI STOP JUMPDEST STOP
    let not_taken = [0x5f, 0x61, 0x00, 0x06, 0x57, 0x00, 0x5b, 0x00];
    check_charges(
        &not_taken,
        0,
        &[
            charge(BASE, 1, &[]),
            charge(VERYLOW, 2, &[0]),
            charge(HIGH, 5, &[0, 6]),
        ],
    );
}

/// JUMPI takes any non-zero condition, and only those: one with only its high limb set, and the
/// branches return different bytes
#[test]
fn jumpi_conditions() {
    // <condition> PUSH2 dest JUMPI <return 0xaa> dest: JUMPDEST <return 0xbb>
    let code = |condition: &[u8]| {
        let dest = condition.len() + 12;
        let mut code = condition.to_vec();
        code.extend_from_slice(&[0x61, 0x00, dest as u8, 0x57]);
        code.extend_from_slice(&return_byte(0xaa));
        code.push(0x5b);
        code.extend_from_slice(&return_byte(0xbb));
        code
    };
    let mut high = vec![0x7f, 0x80];
    high.resize(33, 0);
    for (condition, push, taken) in [
        (&[0x5f][..], BASE, false),
        (&[0x60, 0x01], VERYLOW, true),
        (&high, VERYLOW, true),
    ] {
        let (gas, data) = if taken {
            (push + VERYLOW + HIGH + JUMPDEST + RETURN_BYTE, 0xbb)
        } else {
            (push + VERYLOW + HIGH + RETURN_BYTE, 0xaa)
        };
        let expected = End::Success {
            left: 1000 - gas,
            data: vec![data],
        };
        assert_eq!(run(&code(condition), 1000), expected);
    }
}

/// JUMP and taken JUMPI to invalid destinations: past the code, on a 0x5b in a PUSH1's
/// immediate, at the jump itself, on a PUSH1 and at the end of the code. The charges come first;
/// an untaken JUMPI doesn't check its destination.
#[test]
fn invalid_destinations() {
    // PUSH2 dest JUMP PUSH1 0x5b STOP
    for dest in [0xffff, 5, 3, 4, 7] {
        let code = [0x61, (dest >> 8) as u8, dest as u8, 0x56, 0x60, 0x5b, 0x00];
        let gas = VERYLOW + MID;
        for budget in [gas, 1000] {
            assert_eq!(run(&code, budget), failed(EvmError::InvalidJump, 4, &[]));
        }
        assert_eq!(run(&code, gas - 1), out_of_gas(4, &[dest]));
    }
    // PUSH1 condition PUSH2 dest JUMPI PUSH1 0x5b STOP
    for dest in [0xffff, 7, 5, 6, 9] {
        let code = |condition| {
            [
                0x60,
                condition,
                0x61,
                (dest >> 8) as u8,
                dest as u8,
                0x57,
                0x60,
                0x5b,
                0x00,
            ]
        };
        let gas = 2 * VERYLOW + HIGH;
        for budget in [gas, 1000] {
            assert_eq!(run(&code(1), budget), failed(EvmError::InvalidJump, 6, &[]));
        }
        assert_eq!(run(&code(1), gas - 1), out_of_gas(6, &[1, dest]));
        assert_eq!(run(&code(0), 1000), stop(1000 - gas - VERYLOW));
        // nothing left for the PUSH1 after it
        assert_eq!(run(&code(0), gas + VERYLOW - 1), out_of_gas(7, &[]));
    }
}

/// JUMPI with only its destination on the stack fails after both charges; its charge failing
/// beats the underflow
#[test]
fn jumpi_on_empty_stack() {
    // PUSH2 4 JUMPI JUMPDEST STOP
    let code = [0x61, 0x00, 0x04, 0x57, 0x5b, 0x00];
    for budget in [VERYLOW + HIGH, 1000] {
        assert_eq!(
            run(&code, budget),
            failed(EvmError::StackUnderflow, 4, &[4])
        );
    }
    assert_eq!(run(&code, VERYLOW + HIGH - 1), out_of_gas(4, &[4]));
    assert_eq!(run(&code, VERYLOW - 1), out_of_gas(1, &[]));
}

/// PUSH2 JUMP / JUMPI on 1023 values, where the PUSH2 fills the stack (and so does the
/// destination after a failed jump charge), and PUSH2 on a full stack, which fails after its
/// charge
#[test]
fn full_stacks() {
    let fill = |n: usize, tail: &[u8]| {
        let mut code = vec![0x5f; n];
        code.extend_from_slice(tail);
        code
    };
    let fill_gas = |n: u64| n * BASE;
    // 1023 x PUSH0, PUSH2 1027 JUMP 1027: JUMPDEST STOP
    check_charges(
        &fill(1023, &[0x61, 0x04, 0x03, 0x56, 0x5b, 0x00]),
        fill_gas(1023),
        &[
            charge(VERYLOW, 1024, &on_zeros(1023, &[])),
            charge(MID, 1027, &on_zeros(1023, &[1027])),
            charge(JUMPDEST, 1028, &on_zeros(1023, &[])),
        ],
    );
    // 1023 x PUSH0, PUSH2 1028 JUMPI (not taken) STOP 1028: JUMPDEST STOP
    check_charges(
        &fill(1023, &[0x61, 0x04, 0x04, 0x57, 0x00, 0x5b, 0x00]),
        fill_gas(1023),
        &[
            charge(VERYLOW, 1024, &on_zeros(1023, &[])),
            charge(HIGH, 1027, &on_zeros(1023, &[1028])),
        ],
    );
    // 1022 x PUSH0, PUSH1 1, PUSH2 1029 JUMPI (taken) STOP 1029: JUMPDEST STOP
    check_charges(
        &fill(
            1022,
            &[0x60, 0x01, 0x61, 0x04, 0x05, 0x57, 0x00, 0x5b, 0x00],
        ),
        fill_gas(1022) + VERYLOW,
        &[
            charge(VERYLOW, 1025, &on_zeros(1022, &[1])),
            charge(HIGH, 1028, &on_zeros(1022, &[1, 1029])),
            charge(JUMPDEST, 1030, &on_zeros(1022, &[])),
        ],
    );
    // 1024 x PUSH0, PUSH2 1028 JUMP / JUMPI 1028: JUMPDEST STOP
    for jump in [0x56, 0x57] {
        let code = fill(1024, &[0x61, 0x04, 0x04, jump, 0x5b, 0x00]);
        let gas = fill_gas(1024) + VERYLOW;
        let full = on_zeros(1024, &[]);
        for budget in [gas, gas + 1000] {
            assert_eq!(
                run(&code, budget),
                failed(EvmError::StackOverflow, 1027, &full)
            );
        }
        assert_eq!(run(&code, gas - 1), out_of_gas(1025, &full));
    }
}

/// A PUSH2 that ends the code, with its immediate whole, cut or missing (it is never followed by
/// a jump there), and a PUSH2 followed by a JUMP or JUMPI that is the last byte of the code
#[test]
fn push2_at_the_end_of_the_code() {
    for code in [
        &[0x5f, 0x61, 0x12, 0x34][..],
        &[0x5f, 0x61, 0x12],
        &[0x5f, 0x61],
    ] {
        check_charges(code, 0, &[charge(BASE, 1, &[]), charge(VERYLOW, 2, &[0])]);
    }
    // PUSH0 PUSH2 0 JUMPI: not taken, then the end of the code
    check_charges(
        &[0x5f, 0x61, 0x00, 0x00, 0x57],
        0,
        &[
            charge(BASE, 1, &[]),
            charge(VERYLOW, 2, &[0]),
            charge(HIGH, 5, &[0, 0]),
        ],
    );
    // JUMPDEST PUSH2 dest JUMP, to the JUMP itself and to the end of the code
    for dest in [4, 5] {
        let code = [0x5b, 0x61, 0x00, dest, 0x56];
        for budget in [JUMPDEST + VERYLOW + MID, 1000] {
            assert_eq!(run(&code, budget), failed(EvmError::InvalidJump, 5, &[]));
        }
    }
    // JUMPDEST PUSH2 0 JUMP: loops until out of gas, at the JUMPDEST, the PUSH2 or the JUMP
    let iteration = JUMPDEST + VERYLOW + MID;
    for budget in 0..3 * iteration {
        let expected = match budget % iteration {
            0 => out_of_gas(1, &[]),
            1..4 => out_of_gas(2, &[]),
            _ => out_of_gas(5, &[0]),
        };
        assert_eq!(
            run(&[0x5b, 0x61, 0x00, 0x00, 0x56], budget),
            expected,
            "{budget}"
        );
    }
    // JUMPDEST PUSH1 1 PUSH2 0 JUMPI: the same with a taken JUMPI
    let iteration = JUMPDEST + VERYLOW + VERYLOW + HIGH;
    for budget in 0..3 * iteration {
        let expected = match budget % iteration {
            0 => out_of_gas(1, &[]),
            1..4 => out_of_gas(2, &[]),
            4..7 => out_of_gas(4, &[1]),
            _ => out_of_gas(7, &[1, 0]),
        };
        assert_eq!(
            run(&[0x5b, 0x60, 0x01, 0x61, 0x00, 0x00, 0x57], budget),
            expected,
            "{budget}"
        );
    }
}

/// A JUMP, an untaken and a taken JUMPI, each after a PUSH2, with every budget up to their cost,
/// and with budgets whose low 32-bit word is short of a charge (which then borrows from the high
/// word), is 2^31 or more, or is at their edges, up to the largest ergs and past them
#[test]
fn low_word_budgets() {
    let code = [
        0x61, 0x00, 0x04, 0x56, // PUSH2 4 JUMP
        0x5b, // 4: JUMPDEST
        0x5f, 0x61, 0xff, 0xff, 0x57, // PUSH0 PUSH2 0xffff JUMPI (not taken)
        0x60, 0x01, 0x61, 0x00, 0x10, 0x57, // PUSH1 1 PUSH2 16 JUMPI
        0x5b, 0x00, // 16: JUMPDEST STOP
    ];
    let charges = [
        charge(VERYLOW, 1, &[]),
        charge(MID, 4, &[4]),
        charge(JUMPDEST, 5, &[]),
        charge(BASE, 6, &[]),
        charge(VERYLOW, 7, &[0]),
        charge(HIGH, 10, &[0, 0xffff]),
        charge(VERYLOW, 11, &[]),
        charge(VERYLOW, 13, &[1]),
        charge(HIGH, 16, &[1, 16]),
        charge(JUMPDEST, 17, &[]),
    ];
    check_charges(&code, 0, &charges);
    let cost: u64 = charges.iter().map(|c| c.gas).sum();
    for base in [
        SIGN,
        LOW_WORD,
        LOW_WORD + SIGN,
        3 * LOW_WORD,
        MAX_ERGS - cost - 1,
        1 << 63,
        u64::MAX - cost - 1,
    ] {
        for delta in 0..=cost + 1 {
            for budget in [base + delta, base - 1 - delta] {
                assert_eq!(run(&code, budget), stop(budget - cost), "{budget:#x}");
            }
        }
    }
}

/// Loops of 200 rounds that add the PC at the same instruction each round, then return the sum:
/// one with a `PUSH2 JUMP` back edge and an `ISZERO PUSH2 JUMPI` exit, one with a
/// `DUP1 PUSH2 JUMPI` back edge
#[test]
fn loops() {
    let rounds = 200;
    let sum = 5 * rounds;
    let mut expected_data = vec![0; 30];
    expected_data.extend_from_slice(&(sum as u16).to_be_bytes());
    let with_jump = [
        0x5f,
        0x60,
        rounds as u8, // PUSH0 (the sum) PUSH1 rounds
        0x5b,         // 3: JUMPDEST
        0x90,
        0x58,
        0x01,
        0x90, // SWAP1 PC ADD SWAP1: the PC is 5
        0x60,
        0x01,
        0x90,
        0x03, // the rounds left - 1
        0x80,
        0x15,
        0x61,
        0x00,
        0x16,
        0x57, // DUP1 ISZERO PUSH2 22 JUMPI
        0x61,
        0x00,
        0x03,
        0x56, // PUSH2 3 JUMP
        0x5b,
        0x50, // 22: JUMPDEST POP
        0x5f,
        0x52,
        0x60,
        0x20,
        0x5f,
        0xf3, // return the sum
    ];
    let with_jumpi = [
        0x5f,
        0x60,
        rounds as u8, // PUSH0 (the sum) PUSH1 rounds
        0x5b,         // 3: JUMPDEST
        0x90,
        0x58,
        0x01,
        0x90, // SWAP1 PC ADD SWAP1: the PC is 5
        0x60,
        0x01,
        0x90,
        0x03, // the rounds left - 1
        0x80,
        0x61,
        0x00,
        0x03,
        0x57, // DUP1 PUSH2 3 JUMPI
        0x50, // POP
        0x5f,
        0x52,
        0x60,
        0x20,
        0x5f,
        0xf3, // return the sum
    ];
    let start = BASE + VERYLOW;
    // JUMPDEST, SWAP1 PC ADD SWAP1, PUSH1 SWAP1 SUB, DUP1
    let round = JUMPDEST + (3 * VERYLOW + BASE) + 3 * VERYLOW + VERYLOW;
    // POP, then PUSH0 MSTORE (one word of heap) PUSH1 PUSH0 RETURN
    let end = BASE + (BASE + VERYLOW + MEMORY + VERYLOW + BASE);
    let jump_cost = start
        + rounds * (round + VERYLOW + VERYLOW + HIGH)
        + (rounds - 1) * (VERYLOW + MID)
        + JUMPDEST
        + end;
    let jumpi_cost = start + rounds * (round + VERYLOW + HIGH) + end;
    for (code, cost) in [(&with_jump[..], jump_cost), (&with_jumpi[..], jumpi_cost)] {
        for budget in [cost, LOW_WORD + 5] {
            let expected = End::Success {
                left: budget - cost,
                data: expected_data.clone(),
            };
            assert_eq!(run(code, budget), expected, "{budget:#x}");
        }
        // short of the last PUSH0
        assert_eq!(run(code, cost - 1), out_of_gas(code.len() - 1, &[0x20]));
    }
}

/// The step count tells whether the pair ran as one step: with the cfg, a PUSH2 JUMP or PUSH2
/// JUMPI is one step to the tracer
#[test]
fn the_cfg_fuses_the_pair() {
    let fused = cfg!(evm_fuse_push2_jumps);
    // PUSH2 4 JUMP JUMPDEST STOP; PUSH1 1 PUSH2 7 JUMPI STOP JUMPDEST STOP
    for (code, steps) in [
        (&[0x61, 0x00, 0x04, 0x56, 0x5b, 0x00][..], 4),
        (&[0x60, 0x01, 0x61, 0x00, 0x07, 0x57, 0x00, 0x5b, 0x00], 5),
    ] {
        let (end, seen) = run_counted(code, 1000);
        assert!(matches!(end, End::Success { .. }), "{end:?}");
        assert_eq!(seen, steps - fused as usize, "fused: {fused}");
    }
}
