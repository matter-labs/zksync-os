//! PUSH2 followed by JUMP or JUMPI. The proving target runs such a pair as one step (see
//! `Hot::push2` in the interpreter), so these cases cover the ways the pair can end: a charge
//! short of ergs, of the step's native or of the instruction's native, the stack checks, invalid
//! destinations, the untaken JUMPI (whose destination is not checked) and a PUSH2 at the end of
//! the code. The pair runs as one step only with gas-only resources (the Ethereum STF); the rig
//! runs the ZKsync OS STF, which dispatches both, so here the cases pin what a fused pair must
//! match.
//!
//! Each case runs `body` in a callee that a driver calls with a given gas, and pins what the host
//! sees of it: the EVM error, and the gas and native used from the start of `body` to the end of
//! its last step (before an error burns the rest). With `ZKSYNC_RISC_V_RUN` the rig also compares
//! the RISC-V run with the host through the storage hash. The driver stores the call's result and
//! the gas left after it. Cases without a jump that lands on a JUMPDEST run with native priced at
//! one per gas and check that native sets the fee, so the hash covers the native charges too. A
//! landing charges 20 native less on the proving target (`Hot::land_on_jumpdest`), so cases with
//! one run with the default prices, where gas sets the fee.

use crate::test_support::{call_tx_with, new_tester};
use rig::alloy::primitives::{address, Address, B256, U256 as AlloyU256};
use rig::alloy::signers::local::PrivateKeySigner;
use rig::constants::{DEFAULT_BALANCE, DEFAULT_MAX_FEE, DEFAULT_PRIORITY_FEE, TEST_CHAIN_ID};
use rig::evm_interpreter::gas_constants::{BASE, HIGH, JUMPDEST, MID, VERYLOW};
use rig::evm_interpreter::native_resource_constants::{
    DUP_NATIVE_COST, ISZERO_NATIVE_COST, JUMPDEST_NATIVE_COST, JUMPI_NATIVE_COST, JUMP_NATIVE_COST,
    PUSH_NATIVE_COSTS, STEP_NATIVE_COST, SUB_NATIVE_COST, SWAP_NATIVE_COST,
};
use rig::forward_system::system::system_types::ForwardRunningSystem;
use rig::ruint::aliases::{B160, U256};
use rig::zk_ee::execution_environment_type::ExecutionEnvironmentType;
use rig::zk_ee::system::evm::{EvmError, EvmFrameInterface, EvmStackInterface};
use rig::zk_ee::system::tracer::evm_tracer::EvmTracer;
use rig::zk_ee::system::tracer::Tracer;
use rig::zk_ee::system::validator::NopTxValidator;
use rig::zk_ee::system::{
    CallResult, Computational, ExecutionEnvironmentLaunchParams, Resources, SystemTypes,
};
use rig::zk_ee::utils::Bytes32;
use rig::zksync_os_tests_common::zksync_tx::ZKsyncTxEnvelope;
use rig::{BlockContext, BlockOutput, TestingFramework};

const DRIVER: Address = address!("0000000000000000000000000000000000000f00");
const CALLEE: Address = address!("0000000000000000000000000000000000000f01");

/// JUMPDESTs before every `body`. A transaction whose gas limit is below about 1.19M doesn't
/// reach the callee (at one native per gas), so to run `body` with little native left the
/// preamble burns it: 50 per byte, the same on the host and on the proving target (no jump
/// lands on these).
const PREAMBLE: usize = 2400;

/// Gas limit of the cases with plenty of native
const PLENTY: u64 = 5_000_000;

const ERGS_PER_GAS: u64 = 256;

/// The two bytes of the offset `x` in `body` (the code starts with the preamble)
fn at(x: usize) -> [u8; 2] {
    u16::try_from(PREAMBLE + x).unwrap().to_be_bytes()
}

/// The callee: the preamble, then `body`
fn with_preamble(body: &[u8]) -> Vec<u8> {
    let mut code = vec![0x5b; PREAMBLE];
    code.extend_from_slice(body);
    code
}

/// Native of a step on the host: the instruction's and the step's
fn step(native: u64) -> u64 {
    native + STEP_NATIVE_COST
}

/// Calls the address in calldata word 1 with the gas in word 0, then stores the call's success
/// flag in slot 0 and the gas left in slot 1
fn driver_code() -> Vec<u8> {
    vec![
        0x5f, 0x5f, 0x5f, 0x5f, 0x5f, // CALL's sizes, offsets and value
        0x60, 0x20, 0x35, // the callee
        0x5f, 0x35, // the gas
        0xf1, // CALL
        0x5f, 0x55, // SSTORE(0, success)
        0x5a, 0x60, 0x01, 0x55, // SSTORE(1, gas)
        0x00,
    ]
}

/// A transaction of `signer` to the driver, which calls `callee` with `call_gas`
fn driver_tx(
    signer: &PrivateKeySigner,
    nonce: u64,
    gas_limit: u64,
    call_gas: u64,
    callee: Address,
) -> ZKsyncTxEnvelope {
    let mut calldata = U256::from(call_gas).to_be_bytes::<32>().to_vec();
    calldata.extend_from_slice(&[0; 12]);
    calldata.extend_from_slice(callee.as_slice());
    call_tx_with(
        signer.clone(),
        DRIVER,
        nonce,
        gas_limit,
        AlloyU256::ZERO,
        calldata,
        DEFAULT_MAX_FEE,
        DEFAULT_PRIORITY_FEE,
        TEST_CHAIN_ID,
    )
}

/// Native priced at one per gas (the gas price is the base fee), so that native sets the fee
/// whenever a transaction uses more of it than gas
fn native_visible() -> BlockContext {
    BlockContext {
        native_price: U256::from(DEFAULT_MAX_FEE),
        ..Default::default()
    }
}

/// A chain with the driver and `code` at `CALLEE`
fn tester(code: &[u8], context: BlockContext, signer: &PrivateKeySigner) -> TestingFramework {
    new_tester()
        .with_block_context(context)
        .with_balance(signer.address(), U256::from(DEFAULT_BALANCE))
        .with_evm_contract(DRIVER, &driver_code())
        .with_evm_contract(CALLEE, code)
}

/// What the host tracer sees of the callee in one transaction
#[derive(Default)]
struct BodyRecord {
    /// Ergs and native before the first step of `body`
    start: Option<(u64, u64)>,
    /// Ergs and native after the last step
    end: Option<(u64, u64)>,
    /// The EVM error the frame ended with
    error: Option<EvmError>,
    /// The stack it ended with then, bottom first, as limbs
    error_stack: Option<Vec<[u64; 4]>>,
}

/// Records the callee's frame from the instruction at `PREAMBLE` on
struct BodyTracer {
    callee: B160,
    record: BodyRecord,
}

fn amounts(resources: &<ForwardRunningSystem as SystemTypes>::Resources) -> (u64, u64) {
    (resources.ergs().as_u64(), resources.native().as_u64())
}

impl EvmTracer<ForwardRunningSystem> for BodyTracer {
    fn before_evm_interpreter_execution_step(
        &mut self,
        _opcode: u8,
        frame_state: &impl EvmFrameInterface<ForwardRunningSystem>,
    ) {
        if frame_state.address() == self.callee
            && frame_state.instruction_pointer() == PREAMBLE
            && self.record.start.is_none()
        {
            self.record.start = Some(amounts(frame_state.resources()));
        }
    }

    fn after_evm_interpreter_execution_step(
        &mut self,
        _opcode: u8,
        frame_state: &impl EvmFrameInterface<ForwardRunningSystem>,
    ) {
        if frame_state.address() == self.callee {
            self.record.end = Some(amounts(frame_state.resources()));
        }
    }

    fn on_opcode_error(
        &mut self,
        error: &EvmError,
        frame_state: &impl EvmFrameInterface<ForwardRunningSystem>,
    ) {
        if frame_state.address() == self.callee {
            self.record.error = Some(error.clone());
            let stack = frame_state.stack().to_slice();
            self.record.error_stack = Some(stack.iter().map(|v| *v.as_limbs()).collect());
        }
    }

    fn on_call_error(&mut self, _error: &EvmError) {}

    fn on_selfdestruct(
        &mut self,
        _beneficiary: B160,
        _token_value: U256,
        _frame_state: &impl EvmFrameInterface<ForwardRunningSystem>,
    ) {
    }

    fn on_create_request(&mut self, _is_create2: bool) {}
}

impl Tracer<ForwardRunningSystem> for BodyTracer {
    fn evm_tracer(&mut self) -> &mut impl EvmTracer<ForwardRunningSystem> {
        self
    }

    fn on_new_execution_frame(
        &mut self,
        _request: &ExecutionEnvironmentLaunchParams<ForwardRunningSystem>,
    ) {
    }

    fn after_execution_frame_completed(
        &mut self,
        _result: Option<(
            &<ForwardRunningSystem as SystemTypes>::Resources,
            &CallResult<ForwardRunningSystem>,
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

/// Prices and native budget of a case
#[derive(Clone, Copy)]
enum Budget {
    /// Default prices (gas sets the fee), plenty of native
    GasFee,
    /// Native at one per gas, plenty of it: native must set the fee
    NativeFee,
    /// Native at one per gas, with this much left when `body` starts
    Native(u64),
}

/// How the callee ended
#[derive(Debug, PartialEq, Eq)]
enum End {
    /// It stopped
    Stop,
    /// With this EVM error
    Error(EvmError),
    /// The transaction ran out of native in it
    OutOfNative,
}

/// What the host saw of `body`: how it ended, and the gas and native it used
#[derive(Debug, PartialEq, Eq)]
struct Seen {
    end: End,
    gas: u64,
    native: u64,
}

/// Runs the callee `code` (whose `body` starts after the preamble) with `gas` for the body
fn run_code(code: &[u8], gas: u64, budget: Budget) -> (BlockOutput, BodyRecord) {
    let call_gas = PREAMBLE as u64 * JUMPDEST + gas;
    let (context, gas_limit) = match budget {
        Budget::GasFee => (BlockContext::default(), PLENTY),
        Budget::NativeFee => (native_visible(), PLENTY),
        Budget::Native(native) => {
            // the native left at `body` is the gas limit less a fixed overhead: measure it
            // once with plenty, forward only (the case itself is the check)
            let signer = PrivateKeySigner::random();
            let mut probe = tester(code, native_visible(), &signer);
            probe.set_run_config(Some(rig::run_config::forward_only()));
            let (_, record) = run_tx(&mut probe, &signer, PLENTY, call_gas);
            let (_, start_native) = record.start.expect("the body ran");
            (native_visible(), PLENTY - start_native + native)
        }
    };
    let signer = PrivateKeySigner::random();
    let mut tester = tester(code, context, &signer);
    let (output, record) = run_tx(&mut tester, &signer, gas_limit, call_gas);
    if let Budget::Native(native) = budget {
        assert_eq!(record.start.map(|(_, n)| n), Some(native));
    }
    let tx = output.tx_results[0]
        .as_ref()
        .expect("the transaction is accepted");
    if let Budget::NativeFee = budget {
        // so the storage hash of the RISC-V run covers every native charge
        assert_eq!(tx.gas_used, tx.native_used, "native sets the fee");
    }
    (output, record)
}

fn run_tx(
    tester: &mut TestingFramework,
    signer: &PrivateKeySigner,
    gas_limit: u64,
    call_gas: u64,
) -> (BlockOutput, BodyRecord) {
    let mut tracer = BodyTracer {
        callee: B160::from_be_bytes(CALLEE.into_array()),
        record: BodyRecord::default(),
    };
    let tx = driver_tx(signer, 0, gas_limit, call_gas, CALLEE);
    let output = tester.execute_block_with_tracing(vec![tx], &mut tracer, &mut NopTxValidator);
    (output, tracer.record)
}

/// The driver's call succeeded
fn call_succeeded(output: &BlockOutput) -> bool {
    let slot_0 = B256::ZERO;
    output.storage_writes.iter().any(|w| {
        w.account == DRIVER && w.account_key == slot_0 && w.value == B256::with_last_byte(1)
    })
}

/// Runs `body` with `gas` and returns what the host saw
fn run(body: &[u8], gas: u64, budget: Budget) -> Seen {
    let (output, record) = run_code(&with_preamble(body), gas, budget);
    let (start_ergs, start_native) = record.start.expect("the body ran");
    let (end_ergs, end_native) = record.end.expect("the body ran");
    let tx = output.tx_results[0].as_ref().unwrap();
    let end = match record.error {
        Some(error) => End::Error(error),
        // out of native: the callee's frame never returns
        None if end_native == 0 && !tx.is_success() => End::OutOfNative,
        None => {
            assert!(call_succeeded(&output));
            End::Stop
        }
    };
    assert_eq!((start_ergs - end_ergs) % ERGS_PER_GAS, 0);
    Seen {
        end,
        gas: (start_ergs - end_ergs) / ERGS_PER_GAS,
        native: start_native - end_native,
    }
}

fn seen(end: End, gas: u64, native: u64) -> Seen {
    Seen { end, gas, native }
}

const PUSH0: u64 = PUSH_NATIVE_COSTS[0];
const PUSH1: u64 = PUSH_NATIVE_COSTS[1];
const PUSH2: u64 = PUSH_NATIVE_COSTS[2];
const PUSH32: u64 = PUSH_NATIVE_COSTS[32];

/// `PUSH2 dest JUMP` to an invalid destination: past the code, and inside an immediate
#[test]
fn jump_to_invalid_destination() {
    let [d0, d1] = at(5);
    let past_code = [0x61, 0xff, 0xff, 0x56];
    let in_immediate = [0x61, d0, d1, 0x56, 0x60, 0x5b, 0x00];
    for body in [&past_code[..], &in_immediate[..]] {
        let gas = VERYLOW + MID;
        let native = step(PUSH2) + step(JUMP_NATIVE_COST);
        let expected = seen(End::Error(EvmError::InvalidJump), gas, native);
        assert_eq!(run(body, 100, Budget::NativeFee), expected);
    }
}

/// An untaken JUMPI doesn't check its destination, and the code after it runs
#[test]
fn jumpi_not_taken() {
    let [d0, d1] = at(8);
    // PUSH0 PUSH2 8 JUMPI PUSH1 42 STOP JUMPDEST STOP
    let valid = [0x5f, 0x61, d0, d1, 0x57, 0x60, 0x2a, 0x00, 0x5b, 0x00];
    let gas = BASE + VERYLOW + HIGH + VERYLOW;
    let native = step(PUSH0) + step(PUSH2) + step(JUMPI_NATIVE_COST) + step(PUSH1) + step(0);
    assert_eq!(
        run(&valid, 100, Budget::NativeFee),
        seen(End::Stop, gas, native)
    );
    // PUSH0 PUSH2 0xffff JUMPI STOP
    let invalid = [0x5f, 0x61, 0xff, 0xff, 0x57, 0x00];
    let gas = BASE + VERYLOW + HIGH;
    let native = step(PUSH0) + step(PUSH2) + step(JUMPI_NATIVE_COST) + step(0);
    assert_eq!(
        run(&invalid, 100, Budget::NativeFee),
        seen(End::Stop, gas, native)
    );
    // with exactly the gas of its steps: the unchecked destination costs nothing
    assert_eq!(
        run(&invalid, gas, Budget::NativeFee),
        seen(End::Stop, gas, native)
    );
}

/// A taken JUMPI to an invalid destination
#[test]
fn jumpi_taken_to_invalid_destination() {
    // PUSH1 1 PUSH2 0xffff JUMPI
    let body = [0x60, 0x01, 0x61, 0xff, 0xff, 0x57];
    let gas = VERYLOW + VERYLOW + HIGH;
    let native = step(PUSH1) + step(PUSH2) + step(JUMPI_NATIVE_COST);
    assert_eq!(
        run(&body, 100, Budget::NativeFee),
        seen(End::Error(EvmError::InvalidJump), gas, native)
    );
    // out of gas beats the invalid destination
    assert_eq!(
        run(&body, gas - 1, Budget::NativeFee),
        seen(
            End::Error(EvmError::OutOfGas),
            gas - 1,
            step(PUSH1) + step(PUSH2) + STEP_NATIVE_COST
        )
    );
}

/// JUMPI with only its destination on the stack fails after both charges; its own charge
/// failing beats the underflow
#[test]
fn jumpi_on_empty_stack() {
    let [d0, d1] = at(4);
    // PUSH2 4 JUMPI JUMPDEST STOP
    let body = [0x61, d0, d1, 0x57, 0x5b, 0x00];
    let gas = VERYLOW + HIGH;
    let native = step(PUSH2) + step(JUMPI_NATIVE_COST);
    assert_eq!(
        run(&body, 100, Budget::NativeFee),
        seen(End::Error(EvmError::StackUnderflow), gas, native)
    );
    assert_eq!(
        run(&body, gas - 1, Budget::NativeFee),
        seen(
            End::Error(EvmError::OutOfGas),
            gas - 1,
            step(PUSH2) + STEP_NATIVE_COST
        )
    );
    // native for the JUMPI's step but not for its cost, or not even for the step
    for left in [STEP_NATIVE_COST, STEP_NATIVE_COST - 1] {
        let native = step(PUSH2) + left;
        assert_eq!(
            run(&body, 100, Budget::Native(native)),
            seen(End::OutOfNative, VERYLOW + HIGH, native)
        );
    }
}

/// PUSH2 on a full stack fails before the jump; its charge failing beats the overflow
#[test]
fn push2_on_full_stack() {
    let [d0, d1] = at(1028);
    let mut body = vec![0x5f; 1024];
    body.extend_from_slice(&[0x61, d0, d1, 0x56, 0x5b, 0x00]);
    let fill_gas = 1024 * BASE;
    let fill_native = 1024 * step(PUSH0);
    assert_eq!(
        run(&body, 10_000, Budget::NativeFee),
        seen(
            End::Error(EvmError::StackOverflow),
            fill_gas + VERYLOW,
            fill_native + step(PUSH2)
        )
    );
    assert_eq!(
        run(&body, fill_gas + VERYLOW - 1, Budget::NativeFee),
        seen(
            End::Error(EvmError::OutOfGas),
            fill_gas + VERYLOW - 1,
            fill_native + STEP_NATIVE_COST
        )
    );
    // short of the PUSH2's native, with and without the ergs
    for (gas, left) in [(10_000, STEP_NATIVE_COST), (10_000, 10), (fill_gas + 2, 10)] {
        let native = fill_native + left;
        let used_gas = if gas == 10_000 {
            fill_gas + VERYLOW
        } else {
            gas
        };
        assert_eq!(
            run(&body, gas, Budget::Native(native)),
            seen(End::OutOfNative, used_gas, native)
        );
    }
}

/// PUSH2 JUMP with the JUMP charge short, and with exactly its gas (an invalid destination:
/// nothing is left for a landing)
#[test]
fn jump_charge_boundaries() {
    let body = [0x61, 0xff, 0xff, 0x56];
    assert_eq!(
        run(&body, VERYLOW + MID - 1, Budget::NativeFee),
        seen(
            End::Error(EvmError::OutOfGas),
            VERYLOW + MID - 1,
            step(PUSH2) + STEP_NATIVE_COST
        )
    );
    assert_eq!(
        run(&body, VERYLOW + MID, Budget::NativeFee),
        seen(
            End::Error(EvmError::InvalidJump),
            VERYLOW + MID,
            step(PUSH2) + step(JUMP_NATIVE_COST)
        )
    );
    for left in [
        STEP_NATIVE_COST - 1,
        STEP_NATIVE_COST,
        step(JUMP_NATIVE_COST) - 1,
    ] {
        let native = step(PUSH2) + left;
        assert_eq!(
            run(&body, 100, Budget::Native(native)),
            seen(End::OutOfNative, VERYLOW + MID, native)
        );
    }
}

/// A PUSH2 that ends the code (it is never followed by a jump there), with its immediate
/// whole, cut, or missing, and JUMP / JUMPI as the last byte
#[test]
fn push2_at_the_end_of_the_code() {
    for body in [
        &[0x5f, 0x61, 0x12, 0x34][..],
        &[0x5f, 0x61, 0x12],
        &[0x5f, 0x61],
    ] {
        assert_eq!(
            run(body, 100, Budget::NativeFee),
            seen(
                End::Stop,
                BASE + VERYLOW,
                step(PUSH0) + step(PUSH2) + step(0)
            )
        );
    }
    // PUSH0 PUSH2 0 JUMPI: not taken, then the end of the code
    assert_eq!(
        run(&[0x5f, 0x61, 0x00, 0x00, 0x57], 100, Budget::NativeFee),
        seen(
            End::Stop,
            BASE + VERYLOW + HIGH,
            step(PUSH0) + step(PUSH2) + step(JUMPI_NATIVE_COST) + step(0)
        )
    );
    // JUMPDEST PUSH2 JUMP to the JUMP itself, and to one past the end of the code
    for dest in [3, 5] {
        let [d0, d1] = at(dest);
        assert_eq!(
            run(&[0x5b, 0x61, d0, d1, 0x56], 100, Budget::NativeFee),
            seen(
                End::Error(EvmError::InvalidJump),
                JUMPDEST + VERYLOW + MID,
                step(JUMPDEST_NATIVE_COST) + step(PUSH2) + step(JUMP_NATIVE_COST)
            )
        );
    }
}

/// A code of a multiple of 8 bytes, whose jumpdest bitmap follows it at once, ending with a
/// PUSH2: the bitmap's first byte (JUMPDESTs at 1, 2, 4 and 6) is 0x56, i.e. JUMP, and must
/// not run after the push
#[test]
fn push2_ending_the_code_before_its_bitmap() {
    let mut code = vec![0x5f, 0x5b, 0x5b, 0x50, 0x5b, 0x5f, 0x5b, 0x50];
    code.resize(PREAMBLE, 0x5b);
    code.extend_from_slice(&[0x5f, 0x5f, 0x5f, 0x5f, 0x5f, 0x61, 0x00, 0x01]);
    assert_eq!(code.len() % 8, 0);
    let (output, record) = run_code(&code, 100, Budget::NativeFee);
    assert_eq!(record.error, None);
    assert!(call_succeeded(&output));
    let (start_ergs, _) = record.start.unwrap();
    let (end_ergs, _) = record.end.unwrap();
    assert_eq!(start_ergs - end_ergs, (5 * BASE + VERYLOW) * ERGS_PER_GAS);
}

/// A constructor ending with a PUSH2, whose code is followed in the caller's memory by 0x56,
/// i.e. JUMP, which must not run: the deployment succeeds and the callee stores the address
#[test]
fn constructor_ending_in_push2() {
    // MSTORE(0, JUMPDEST PUSH2 0 | JUMP ...), CREATE(0, 0, 4), SSTORE(0, address)
    let mut body = vec![0x7f, 0x5b, 0x61, 0x00, 0x00, 0x56];
    body.resize(33, 0);
    body.extend_from_slice(&[0x5f, 0x52, 0x60, 0x04, 0x5f, 0x5f, 0xf0, 0x5f, 0x55, 0x00]);
    let (output, record) = run_code(&with_preamble(&body), 200_000, Budget::NativeFee);
    assert_eq!(record.error, None);
    assert!(call_succeeded(&output));
    let deployed = output
        .storage_writes
        .iter()
        .find(|w| w.account == CALLEE && w.account_key == B256::ZERO)
        .map(|w| w.value);
    assert!(deployed.is_some_and(|a| a != B256::ZERO), "{deployed:?}");
}

/// Jumps that land: JUMP, JUMPI on a condition with only its high limb set, and both right
/// below a full stack
#[test]
fn jumps_that_land() {
    let [d0, d1] = at(5);
    // PUSH2 5 JUMP INVALID JUMPDEST STOP
    assert_eq!(
        run(&[0x61, d0, d1, 0x56, 0xfe, 0x5b, 0x00], 100, Budget::GasFee),
        seen(
            End::Stop,
            VERYLOW + MID + JUMPDEST,
            step(PUSH2) + step(JUMP_NATIVE_COST) + step(JUMPDEST_NATIVE_COST) + step(0)
        )
    );
    // PUSH1 1 / PUSH32 1 << 255, PUSH2 d JUMPI STOP JUMPDEST STOP
    let [d0, d1] = at(7);
    let low = vec![0x60, 0x01, 0x61, d0, d1, 0x57, 0x00, 0x5b, 0x00];
    let [d0, d1] = at(38);
    let mut high = vec![0x7f, 0x80];
    high.resize(33, 0);
    high.extend_from_slice(&[0x61, d0, d1, 0x57, 0x00, 0x5b, 0x00]);
    for (body, push) in [(low, PUSH1), (high, PUSH32)] {
        assert_eq!(
            run(&body, 100, Budget::GasFee),
            seen(
                End::Stop,
                VERYLOW + VERYLOW + HIGH + JUMPDEST,
                step(push)
                    + step(PUSH2)
                    + step(JUMPI_NATIVE_COST)
                    + step(JUMPDEST_NATIVE_COST)
                    + step(0)
            )
        );
    }
    // 1023 x PUSH0, PUSH2 d JUMP / JUMPI (not taken) JUMPDEST STOP
    let [d0, d1] = at(1027);
    for (jump, gas, native) in [
        (0x56, MID, JUMP_NATIVE_COST),
        (0x57, HIGH, JUMPI_NATIVE_COST),
    ] {
        let mut body = vec![0x5f; 1023];
        body.extend_from_slice(&[0x61, d0, d1, jump, 0x5b, 0x00]);
        assert_eq!(
            run(&body, 10_000, Budget::GasFee),
            seen(
                End::Stop,
                1023 * BASE + VERYLOW + gas + JUMPDEST,
                1023 * step(PUSH0)
                    + step(PUSH2)
                    + step(native)
                    + step(JUMPDEST_NATIVE_COST)
                    + step(0)
            )
        );
    }
}

/// A loop of 5 with an `ISZERO PUSH2 JUMPI` exit and a `PUSH2 JUMP` back edge, and a jump
/// back that loops until out of gas at a landing
#[test]
fn loops() {
    let [e0, e1] = at(0x11);
    let [l0, l1] = at(2);
    let body = [
        0x60, 0x05, // PUSH1 5
        0x5b, // loop: JUMPDEST
        0x60, 0x01, 0x90, 0x03, // counter - 1
        0x80, 0x15, // DUP1 ISZERO
        0x61, e0, e1, 0x57, // PUSH2 exit JUMPI
        0x61, l0, l1, 0x56, // PUSH2 loop JUMP
        0x5b, 0x00, // exit: JUMPDEST STOP
    ];
    let iteration_gas = JUMPDEST + 6 * VERYLOW + HIGH;
    let iteration_native = step(JUMPDEST_NATIVE_COST)
        + step(PUSH1)
        + step(SWAP_NATIVE_COST)
        + step(SUB_NATIVE_COST)
        + step(DUP_NATIVE_COST)
        + step(ISZERO_NATIVE_COST)
        + step(PUSH2)
        + step(JUMPI_NATIVE_COST);
    let back_gas = VERYLOW + MID;
    let back_native = step(PUSH2) + step(JUMP_NATIVE_COST);
    assert_eq!(
        run(&body, 1000, Budget::GasFee),
        seen(
            End::Stop,
            VERYLOW + 5 * iteration_gas + 4 * back_gas + JUMPDEST,
            step(PUSH1)
                + 5 * iteration_native
                + 4 * back_native
                + step(JUMPDEST_NATIVE_COST)
                + step(0)
        )
    );
    // JUMPDEST PUSH2 0 JUMP, with the gas of 10 iterations but the last landing
    let [d0, d1] = at(0);
    let iteration = JUMPDEST + VERYLOW + MID;
    assert_eq!(
        run(&[0x5b, 0x61, d0, d1, 0x56], 10 * iteration, Budget::GasFee),
        seen(
            End::Error(EvmError::OutOfGas),
            10 * iteration,
            10 * (step(JUMPDEST_NATIVE_COST) + step(PUSH2) + step(JUMP_NATIVE_COST))
                + STEP_NATIVE_COST
        )
    );
}

/// A failed charge of the jump leaves the destination on the stack, as when the two are
/// dispatched (the PUSH2 has pushed it by then): the frame ends with `[dest]` after a JUMP and
/// with `[condition, dest]` after a JUMPI. The host tracer sees the stack of a frame that ran out
/// of gas; running out of native ends the transaction instead, so there the stack is left to the
/// guest-vs-guest comparison of the two dispatches.
#[test]
fn failed_jump_charges_leave_the_destination() {
    let [d0, d1] = at(4);
    let limbs = |x: usize| [x as u64, 0, 0, 0];
    // PUSH2 4 JUMP JUMPDEST STOP, with the gas of the PUSH2 only
    let jump = [0x61, d0, d1, 0x56, 0x5b, 0x00];
    let [d0, d1] = at(6);
    // PUSH1 1 PUSH2 6 JUMPI JUMPDEST STOP, with the gas of the pushes only
    let jumpi = [0x60, 0x01, 0x61, d0, d1, 0x57, 0x5b, 0x00];
    for (body, gas, native, stack) in [
        (&jump[..], VERYLOW, step(PUSH2), vec![limbs(PREAMBLE + 4)]),
        (
            &jumpi[..],
            2 * VERYLOW,
            step(PUSH1) + step(PUSH2),
            vec![limbs(1), limbs(PREAMBLE + 6)],
        ),
    ] {
        let (output, record) = run_code(&with_preamble(body), gas, Budget::NativeFee);
        assert!(!call_succeeded(&output));
        assert_eq!(record.error, Some(EvmError::OutOfGas));
        assert_eq!(record.error_stack, Some(stack));
        let (start_ergs, start_native) = record.start.unwrap();
        let (end_ergs, end_native) = record.end.unwrap();
        assert_eq!(start_ergs - end_ergs, gas * ERGS_PER_GAS);
        assert_eq!(start_native - end_native, native + STEP_NATIVE_COST);
    }
    // the JUMPI short of its native, at its step and at its cost (`jump_charge_boundaries` has
    // the JUMP)
    for left in [STEP_NATIVE_COST - 1, step(JUMPI_NATIVE_COST) - 1] {
        let native = step(PUSH1) + step(PUSH2) + left;
        assert_eq!(
            run(&jumpi, 100, Budget::Native(native)),
            seen(End::OutOfNative, 2 * VERYLOW + HIGH, native)
        );
    }
}
