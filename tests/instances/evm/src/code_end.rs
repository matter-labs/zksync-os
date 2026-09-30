//! Running off the end of the code, which the proving target fetches without a bounds check:
//! truncated PUSH immediates (at each place that stops the pointer at the end), the byte after
//! the code (padding, bitmap, or the constructor's own heap), resumes at the last byte, the
//! native of the `STOP` read after the code, and runs that the bootloader never makes.

use crate::test_support::{call_tx, call_tx_with, create_tx, new_tester};
use rig::alloy::primitives::{address, Address, B256, U256 as AlloyU256};
use rig::basic_system::system_implementation::flat_storage_model::{
    FlatStorageCommitment, TREE_HEIGHT,
};
use rig::constants::{DEFAULT_BALANCE, DEFAULT_MAX_FEE, DEFAULT_PRIORITY_FEE, TEST_CHAIN_ID};
use rig::evm_bytecode::BytecodeBuilder;
use rig::evm_interpreter::native_resource_constants::STEP_NATIVE_COST;
use rig::evm_interpreter::v2::Interpreter;
use rig::evm_interpreter::{
    analyze_into, artifacts_byte_len, ExitCode, ARTIFACTS_CACHING_CODE_VERSION_BYTE,
    DEFAULT_CODE_VERSION_BYTE,
};
use rig::forward_system::run::test_impl::{InMemoryPreimageSource, InMemoryTree};
use rig::forward_system::run::{
    make_oracle_for_proofs_and_dumps_for_init_data, BlockContext as SystemBlockContext,
    NoFriProofSidecar, RunMode,
};
use rig::forward_system::system::system_types::ForwardRunningSystem;
use rig::ruint::aliases::B160;
use rig::ruint::aliases::U256;
use rig::zk_ee::common_structs::system_hooks::HooksStorage;
use rig::zk_ee::common_structs::{BytecodeData, CalleeAccountProperties, ProofData};
use rig::zk_ee::execution_environment_type::ExecutionEnvironmentType;
use rig::zk_ee::memory::slice_vec::SliceVec;
use rig::zk_ee::system::evm::{EvmError, EvmFrameInterface};
use rig::zk_ee::system::metadata::chain_config::ChainConfig;
use rig::zk_ee::system::metadata::zk_metadata::{TxLevelMetadata, ZkMetadata};
use rig::zk_ee::system::tracer::evm_tracer::EvmTracer;
use rig::zk_ee::system::tracer::NopTracer;
use rig::zk_ee::system::tracer::Tracer;
use rig::zk_ee::system::validator::NopTxValidator;
use rig::zk_ee::system::{
    CallModifier, CompletedExecution, EnvironmentParameters, ErgsResource, ExecutionEnvironment,
    ExecutionEnvironmentPreemptionPoint, ExternalCallRequest, ReturnValues, System,
};
use rig::zk_ee::system::{
    CallResult, Computational, ExecutionEnvironmentLaunchParams, Resources, SystemTypes,
};
use rig::zk_ee::utils::Bytes32;
use rig::zksync_os_interface::traits::TxListSource;
use rig::{assert_tx_reverted, assert_tx_success, testing_signer, BlockContext, BlockOutput};
use std::collections::{HashMap, VecDeque};
use std::mem::MaybeUninit;

const CONTRACT: Address = address!("00000000000000000000000000000000000005c0");
const CALLEE: Address = address!("00000000000000000000000000000000000005c1");
const GAS_LIMIT: u64 = 500_000;

fn store_prefix() -> Vec<u8> {
    BytecodeBuilder::new().push_u8(42).push0().sstore().finish()
}

fn run(code: &[u8], expected_gas: u64) -> (u64, u64) {
    let signer = testing_signer(0);
    let mut tester = new_tester()
        .with_balance(signer.address(), U256::from(DEFAULT_BALANCE))
        .with_evm_contract(CONTRACT, code);
    let output = tester.execute_block(vec![call_tx(signer, CONTRACT, GAS_LIMIT)]);
    assert_tx_success!(output, 0);
    assert!(output.storage_writes.iter().any(|write| {
        write.account == CONTRACT
            && write.account_key == B256::ZERO
            && write.value == B256::from(AlloyU256::from(42).to_be_bytes::<32>())
    }));
    let result = output.tx_results[0].as_ref().unwrap();
    assert_eq!(result.gas_used, expected_gas);
    (result.gas_used, result.computational_native_used)
}

#[derive(Default)]
struct PcTracer {
    before: Vec<(u8, usize)>,
    after: Vec<(u8, usize)>,
    /// The native left before each step
    native_before: Vec<u64>,
}

impl EvmTracer<ForwardRunningSystem> for PcTracer {
    fn before_evm_interpreter_execution_step(
        &mut self,
        opcode: u8,
        frame: &impl EvmFrameInterface<ForwardRunningSystem>,
    ) {
        if frame.address() == B160::from_be_bytes(CONTRACT.into_array()) {
            self.before.push((opcode, frame.instruction_pointer()));
            self.native_before.push(frame.resources().native().as_u64());
        }
    }

    fn after_evm_interpreter_execution_step(
        &mut self,
        opcode: u8,
        frame: &impl EvmFrameInterface<ForwardRunningSystem>,
    ) {
        if frame.address() == B160::from_be_bytes(CONTRACT.into_array()) {
            self.after.push((opcode, frame.instruction_pointer()));
        }
    }

    fn on_opcode_error(&mut self, _: &EvmError, _: &impl EvmFrameInterface<ForwardRunningSystem>) {}
    fn on_call_error(&mut self, _: &EvmError) {}
    fn on_selfdestruct(
        &mut self,
        _: B160,
        _: U256,
        _: &impl EvmFrameInterface<ForwardRunningSystem>,
    ) {
    }
    fn on_create_request(&mut self, _: bool) {}
}

impl Tracer<ForwardRunningSystem> for PcTracer {
    fn evm_tracer(&mut self) -> &mut impl EvmTracer<ForwardRunningSystem> {
        self
    }
    fn on_new_execution_frame(
        &mut self,
        _: &ExecutionEnvironmentLaunchParams<ForwardRunningSystem>,
    ) {
    }
    fn after_execution_frame_completed(
        &mut self,
        _: Option<(
            &<ForwardRunningSystem as SystemTypes>::Resources,
            &CallResult<ForwardRunningSystem>,
        )>,
    ) {
    }
    fn on_storage_read(
        &mut self,
        _: ExecutionEnvironmentType,
        _: bool,
        _: B160,
        _: Bytes32,
        _: Bytes32,
    ) {
    }
    fn on_storage_write(
        &mut self,
        _: ExecutionEnvironmentType,
        _: bool,
        _: B160,
        _: Bytes32,
        _: Bytes32,
    ) {
    }
    fn on_bytecode_change(
        &mut self,
        _: ExecutionEnvironmentType,
        _: B160,
        _: Option<&[u8]>,
        _: Bytes32,
        _: u32,
    ) {
    }
    fn on_event(&mut self, _: ExecutionEnvironmentType, _: &B160, _: &[Bytes32], _: &[u8]) {}
    fn begin_tx(&mut self, _: &[u8]) {}
    fn finish_tx(&mut self) {}
}

#[test]
fn code_end_host_tracer_pc_and_guest_outcome() {
    let mut code = store_prefix();
    code.push(0x7f); // final PUSH32, no immediate bytes
    let (expected_gas, expected_native) = run(&code, 43_108);
    let signer = testing_signer(0);
    let mut tester = new_tester()
        .with_balance(signer.address(), U256::from(DEFAULT_BALANCE))
        .with_evm_contract(CONTRACT, &code);
    let mut tracer = PcTracer::default();
    let output = tester.execute_block_with_tracing(
        vec![call_tx(signer, CONTRACT, GAS_LIMIT)],
        &mut tracer,
        &mut NopTxValidator,
    );
    assert_tx_success!(output, 0);
    let result = output.tx_results[0].as_ref().unwrap();
    assert_eq!(
        (result.gas_used, result.computational_native_used),
        (expected_gas, expected_native)
    );
    assert!(output.storage_writes.iter().any(|w| w.account == CONTRACT));
    // The host keeps the unclamped pointer: PUSH32 advances it 32 bytes past its opcode, and
    // the STOP after the code one more.
    assert_eq!(
        tracer.before,
        [(0x60, 0), (0x5f, 2), (0x55, 3), (0x7f, 4), (0x00, 37)]
    );
    assert_eq!(
        tracer.after,
        [(0x60, 2), (0x5f, 3), (0x55, 4), (0x7f, 37), (0x00, 38)]
    );
}

#[test]
fn code_end_truncated_push_widths_and_alignment() {
    for width in [1, 2, 4, 8, 9, 16, 20, 31, 32] {
        for aligned in [false, true] {
            for available in [0, width - 1] {
                let mut code = if aligned { vec![0x5b] } else { vec![] };
                code.extend(store_prefix());
                while if aligned {
                    (code.len() + 1 + available) % 8 != 0
                } else {
                    (code.len() + 1 + available) % 8 == 0
                } {
                    code.push(0x5b);
                }
                code.push(0x5f + width as u8);
                code.extend(std::iter::repeat_n(0x2a, available));
                let jumpdests = code.len() - 5 - available;
                run(&code, 43_108 + jumpdests as u64);
            }
        }
    }
}

#[test]
fn code_end_bitmap_byte_fallback() {
    for len in [8, 16, 64] {
        let mut code = vec![0x5b];
        code.extend(store_prefix());
        code.resize(len, 0x5b);
        run(&code, 43_101 + len as u64);
    }
}

/// Code of 263 bytes that ends in a PUSH`width` without its immediate, and stores 42 before.
/// The length is 7 mod 8, so the code runs from the preimage, where one zero byte of padding
/// follows it and then the bitmap. If the proving target did not stop the pointer at the end
/// of the code after the PUSH, the pointer would move `width` bytes on, onto bitmap byte
/// `width - 1`. A JUMPDEST at offset `8 * (width - 1) + 5`, which the code jumps over, makes
/// that byte 0x20 (SHA3), which fails on the one value on the stack: the guest's run would
/// revert the store instead of stopping.
fn truncated_push_over_bitmap(width: usize) -> Vec<u8> {
    const LEN: usize = 263;
    let mut code = vec![0; LEN];
    code[..4].copy_from_slice(&[0x61, 0x01, 0x01, 0x56]); // PUSH2 257 JUMP
    code[8 * (width - 1) + 5] = 0x5b;
    // 257: JUMPDEST PUSH1 42 PUSH0 SSTORE PUSH<width>
    code[LEN - 6..].copy_from_slice(&[0x5b, 0x60, 0x2a, 0x5f, 0x55, 0x5f + width as u8]);
    code
}

/// Gas of `truncated_push_over_bitmap`: PUSH2 JUMP JUMPDEST, the store and the PUSH
const TRUNCATED_PUSH_OVER_BITMAP_GAS: u64 = 43_108 + 12;

/// The end of the code after a truncated PUSH1 or PUSH3 to PUSH8 (`Hot::push_small`)
#[test]
fn code_end_truncated_push_small_over_bitmap() {
    for width in [1, 3, 4, 5, 6, 7, 8] {
        run(
            &truncated_push_over_bitmap(width),
            TRUNCATED_PUSH_OVER_BITMAP_GAS,
        );
    }
}

/// The end of the code after a truncated PUSH2 (`Hot::push2`, which runs every PUSH2 on the
/// proving target)
#[test]
fn code_end_truncated_push2_over_bitmap() {
    run(
        &truncated_push_over_bitmap(2),
        TRUNCATED_PUSH_OVER_BITMAP_GAS,
    );
}

/// The end of the code after a truncated PUSH9 to PUSH32 (`Hot::push_wide`)
#[test]
fn code_end_truncated_push_wide_over_bitmap() {
    for width in 9..=32 {
        run(
            &truncated_push_over_bitmap(width),
            TRUNCATED_PUSH_OVER_BITMAP_GAS,
        );
    }
}

#[test]
fn code_end_jump_to_last_byte_and_exact_push() {
    let mut code = store_prefix();
    code.extend([0x60, 0x0f, 0x56]); // PUSH1 15; JUMP
    code.resize(15, 0x5b);
    code.push(0x5b);
    run(&code, 43_117);

    let mut code = store_prefix();
    code.extend([0x60, 0x2a]); // PUSH1 exactly reaches code_end
    run(&code, 43_108);
}

#[test]
fn code_end_created_empty_runtime() {
    let signer = testing_signer(0);
    let mut tester = new_tester().with_balance(signer.address(), U256::from(DEFAULT_BALANCE));
    let output = tester.execute_block(vec![create_tx(signer.clone(), GAS_LIMIT, vec![0x00])]);
    assert_tx_success!(output, 0);
    let address = match &output.tx_results[0].as_ref().unwrap().execution_result {
        rig::zksync_os_interface::types::ExecutionResult::Success(
            rig::zksync_os_interface::types::ExecutionOutput::Create(_, address),
        ) => *address,
        other => panic!("unexpected create result: {other:?}"),
    };
    let output = tester.execute_block(vec![call_tx_with(
        signer,
        address,
        1,
        GAS_LIMIT,
        AlloyU256::ZERO,
        vec![],
        DEFAULT_MAX_FEE,
        DEFAULT_PRIORITY_FEE,
        TEST_CHAIN_ID,
    )]);
    assert_tx_success!(output, 0);
}

#[test]
fn code_end_constructor_heap_boundary() {
    for truncated in [false, true] {
        let mut init = BytecodeBuilder::new()
            .push_u8(0xfe)
            .push0()
            .mstore8()
            .finish();
        init.resize(32, 0x5b);
        if truncated {
            init[31] = 0x7f;
        }
        // The copied initcode occupies exactly one 32-byte heap word. The
        // constructor overwrites byte 0 of its own heap with INVALID.
        let mut factory = vec![
            0x60, 32, 0x60, 16, 0x5f, 0x39, 0x60, 32, 0x5f, 0x5f, 0xf0, 0x15, 0x15, 0x5f, 0x55,
            0x00,
        ];
        factory.extend(init);
        let signer = testing_signer(0);
        let mut tester = new_tester()
            .with_balance(signer.address(), U256::from(DEFAULT_BALANCE))
            .with_evm_contract(CONTRACT, &factory);
        let output = tester.execute_block(vec![call_tx(signer, CONTRACT, GAS_LIMIT)]);
        assert_tx_success!(output, 0);
        assert!(output.storage_writes.iter().any(|w| {
            w.account == CONTRACT
                && w.account_key == B256::ZERO
                && w.value == B256::from(AlloyU256::from(1).to_be_bytes::<32>())
        }));
    }
}

#[test]
fn code_end_truncated_push_errors() {
    let mut code = store_prefix();
    code.extend(std::iter::repeat_n(0x5f, 1024));
    code.push(0x60);
    let signer = testing_signer(0);
    let mut tester = new_tester()
        .with_balance(signer.address(), U256::from(DEFAULT_BALANCE))
        .with_evm_contract(CONTRACT, &code);
    let output = tester.execute_block(vec![call_tx(signer, CONTRACT, GAS_LIMIT)]);
    assert_tx_reverted!(output, 0);
    assert_eq!(output.tx_results[0].as_ref().unwrap().gas_used, GAS_LIMIT);
    assert!(!output.storage_writes.iter().any(|w| w.account == CONTRACT));

    // Exactly two execution gas remain after intrinsic gas; PUSH1 needs three.
    let signer = testing_signer(0);
    let mut tester = new_tester()
        .with_balance(signer.address(), U256::from(DEFAULT_BALANCE))
        .with_evm_contract(CONTRACT, &[0x60]);
    let output = tester.execute_block(vec![call_tx(signer, CONTRACT, 21_002)]);
    assert_tx_reverted!(output, 0);
    assert_eq!(output.tx_results[0].as_ref().unwrap().gas_used, 21_002);
    assert!(!output.storage_writes.iter().any(|w| w.account == CONTRACT));
}

#[test]
fn code_end_call_and_staticcall_at_final_byte() {
    let callee = store_prefix();
    for static_call in [false, true] {
        let mut code = store_prefix();
        if static_call {
            code.extend(
                BytecodeBuilder::new()
                    .push0_n(4)
                    .push_address(CALLEE)
                    .gas()
                    .finish(),
            );
            code.push(0xfa);
        } else {
            code.extend(
                BytecodeBuilder::new()
                    .push0_n(5)
                    .push_address(CALLEE)
                    .gas()
                    .finish(),
            );
            code.push(0xf1);
        }
        let signer = testing_signer(0);
        let mut tester = new_tester()
            .with_balance(signer.address(), U256::from(DEFAULT_BALANCE))
            .with_evm_contract(CONTRACT, &code)
            .with_evm_contract(CALLEE, &callee);
        let output = tester.execute_block(vec![call_tx(signer, CONTRACT, GAS_LIMIT)]);
        assert_tx_success!(output, 0);
        assert!(output.storage_writes.iter().any(|w| w.account == CONTRACT));
        assert_eq!(
            output.storage_writes.iter().any(|w| w.account == CALLEE),
            !static_call
        );
    }
}

#[test]
fn code_end_create_at_final_byte() {
    let mut code = store_prefix();
    code.extend(BytecodeBuilder::new().push0_n(3).create().finish());
    run(&code, 75_111);
}

#[test]
fn code_end_nested_fallback_copy_survives_resume() {
    // Each live frame owns a fallback copy because its length is a multiple
    // of eight and its first byte is JUMPDEST.
    let third = address!("00000000000000000000000000000000000005c2");
    let make_code = |next: Option<Address>| {
        let mut code = vec![0x5b];
        if let Some(next) = next {
            code.extend(BytecodeBuilder::new().call_simple(next).pop().finish());
        }
        code.extend(store_prefix());
        code.resize(code.len().div_ceil(8) * 8, 0x5b);
        code
    };
    let signer = testing_signer(0);
    let mut tester = new_tester()
        .with_balance(signer.address(), U256::from(DEFAULT_BALANCE))
        .with_evm_contract(CONTRACT, &make_code(Some(CALLEE)))
        .with_evm_contract(CALLEE, &make_code(Some(third)))
        .with_evm_contract(third, &make_code(None));
    let output = tester.execute_block(vec![call_tx(signer, CONTRACT, GAS_LIMIT)]);
    assert_tx_success!(output, 0);
    for address in [CONTRACT, CALLEE, third] {
        assert!(output.storage_writes.iter().any(|w| w.account == address));
    }
}

/// The worst case for the uncharged constructor copies: nested CREATEs of a maximum-size
/// initcode until the gas runs out. It prints the gas and native used and, with
/// `ZKSYNC_RISC_V_RUN`, the cycles of the whole block on the proving target.
#[test]
#[ignore]
fn nested_max_initcode_creates() {
    // Recursive host frames exceed the test harness's default thread stack
    // before 30M gas is spent.
    std::thread::Builder::new()
        .stack_size(128 * 1024 * 1024)
        .spawn(|| {
            const LEN: usize = 49_152;
            // Each constructor copies all its own code into memory and CREATEs it.
            // The deepest CREATE runs out of forwarded gas; its parents consume the
            // failure and STOP, so the top-level deployment succeeds deterministically.
            let mut init = vec![
                0x61, 0xc0, 0x00, 0x5f, 0x5f, 0x39, 0x61, 0xc0, 0x00, 0x5f, 0x5f, 0xf0, 0x50, 0x00,
            ];
            init.resize(LEN, 0x5b);
            let signer = testing_signer(0);
            let mut tester = new_tester()
                .with_max_tx_gas_limit(30_000_000)
                .with_balance(signer.address(), U256::from(DEFAULT_BALANCE));
            let output = tester.execute_block(vec![create_tx(signer, 30_000_000, init)]);
            assert_tx_success!(output, 0);
            let result = output.tx_results[0].as_ref().unwrap();
            let cycles = tester
                .last_executed_block_info()
                .and_then(|info| info.block_extra_stats.effective_used);
            println!(
                "gas_used={} native_used={} block_cycles={cycles:?}",
                result.gas_used, result.computational_native_used
            );
        })
        .unwrap()
        .join()
        .unwrap();
}

/// Gas limit of a transaction with plenty of native at one native per gas (about 1.2M of it go
/// to the validation)
const PLENTY: u64 = 5_000_000;

/// Native priced at one per gas (the gas price is the base fee), so that native sets the fee:
/// with `ZKSYNC_RISC_V_RUN` the storage hash then covers the native of every step
fn native_visible() -> BlockContext {
    BlockContext {
        native_price: U256::from(DEFAULT_MAX_FEE),
        ..Default::default()
    }
}

/// Runs one transaction with `gas_limit` to `code` at `CONTRACT` (and `store_prefix` at
/// `CALLEE`), at one native per gas
fn run_native(code: &[u8], gas_limit: u64, forward_only: bool) -> (BlockOutput, PcTracer) {
    let signer = testing_signer(0);
    let mut tester = new_tester()
        .with_block_context(native_visible())
        .with_balance(signer.address(), U256::from(DEFAULT_BALANCE))
        .with_evm_contract(CONTRACT, code)
        .with_evm_contract(CALLEE, &store_prefix());
    if forward_only {
        tester.set_run_config(Some(rig::run_config::forward_only()));
    }
    let mut tracer = PcTracer::default();
    let output = tester.execute_block_with_tracing(
        vec![call_tx(signer, CONTRACT, gas_limit)],
        &mut tracer,
        &mut NopTxValidator,
    );
    (output, tracer)
}

/// Codes that end without a `STOP`, which the proving target reads after them (the zero byte
/// after the code): after an instruction, after a truncated PUSH of each kind (whose pointer
/// it stops at the end of the code), after a PUSH that ends with the code, after a call at the
/// last byte returned, and in a copy of the code. No jump lands in them, since a landing
/// charges less native on the proving target (see `push2_jump_fusion`).
fn synthetic_stop_codes() -> Vec<(&'static str, Vec<u8>)> {
    let ending = |tail: &[u8]| {
        let mut code = store_prefix();
        code.extend_from_slice(tail);
        code
    };
    let mut call = BytecodeBuilder::new()
        .push0_n(5)
        .push_address(CALLEE)
        .gas()
        .finish();
    call.push(0xf1);
    // 8 bytes with a JUMPDEST among them: bitmap byte 0 follows the code, and is not zero
    let mut copied = vec![0x5b];
    copied.extend(store_prefix());
    copied.resize(8, 0x5b);
    vec![
        ("instruction", store_prefix()),
        ("truncated PUSH1", ending(&[0x60])),
        ("truncated PUSH2", ending(&[0x61, 0x01])),
        ("truncated PUSH32", ending(&[0x7f, 0x01, 0x02])),
        ("exact PUSH2", ending(&[0x61, 0x01, 0x02])),
        ("call", ending(&call)),
        ("copy", copied),
    ]
}

/// What a transaction did: its status, gas and native, and whether `CONTRACT` stored 42
fn outcome(output: &BlockOutput) -> (bool, u64, u64, bool) {
    let tx = output.tx_results[0]
        .as_ref()
        .expect("the transaction is accepted");
    let stored = output.storage_writes.iter().any(|w| {
        w.account == CONTRACT
            && w.account_key == B256::ZERO
            && w.value == B256::from(AlloyU256::from(42).to_be_bytes::<32>())
    });
    (tx.is_success(), tx.gas_used, tx.native_used, stored)
}

/// The `STOP` the proving target reads after the code, at one native per gas: native sets the
/// fee, so with `ZKSYNC_RISC_V_RUN` the storage hash covers its charge. Each code runs with
/// plenty of native, and with the native before that `STOP` one short of its charge, exactly
/// its charge, and one more: the transaction runs out of native at the `STOP`, or uses all of
/// its native (nothing after the `STOP` charges any).
#[test]
fn code_end_synthetic_stop_native() {
    for (case, code) in synthetic_stop_codes() {
        // the native before the `STOP` with plenty (forward only: the runs below are the
        // check, with the default configuration)
        let (output, probe) = run_native(&code, PLENTY, true);
        let (success, gas, native, stored) = outcome(&output);
        assert!(success && stored, "{case}");
        assert_eq!(gas, native, "{case}: native sets the fee");
        assert_eq!(probe.before.last().map(|step| step.0), Some(0x00), "{case}");
        let before_stop = *probe.native_before.last().unwrap();
        let exact = PLENTY - before_stop + STEP_NATIVE_COST;

        let (output, _) = run_native(&code, PLENTY, false);
        assert_eq!(outcome(&output), (true, gas, native, true), "{case}");
        let (output, tracer) = run_native(&code, exact - 1, false);
        assert_eq!(
            tracer.before.last().map(|step| step.0),
            Some(0x00),
            "{case}"
        );
        assert_eq!(
            tracer.native_before.last(),
            Some(&(STEP_NATIVE_COST - 1)),
            "{case}"
        );
        assert_eq!(
            outcome(&output),
            (false, exact - 1, exact - 1, false),
            "{case}"
        );
        for gas_limit in [exact, exact + 1] {
            let (output, _) = run_native(&code, gas_limit, false);
            assert_eq!(
                outcome(&output),
                (true, exact, exact, true),
                "{case} {gas_limit}"
            );
        }
    }
    // init code, which runs from a copy (the deployment after it charges native too)
    let signer = testing_signer(0);
    let mut tester = new_tester()
        .with_block_context(native_visible())
        .with_balance(signer.address(), U256::from(DEFAULT_BALANCE));
    let output = tester.execute_block(vec![create_tx(signer, PLENTY, store_prefix())]);
    assert_tx_success!(output, 0);
    let tx = output.tx_results[0].as_ref().unwrap();
    assert_eq!(tx.gas_used, tx.native_used, "native sets the fee");
}

/// A system on an empty state, for an interpreter driven without the bootloader
fn bare_system() -> System<ForwardRunningSystem> {
    let tree = InMemoryTree::<false>::empty();
    let proof_data = ProofData {
        state_root_view: FlatStorageCommitment::<{ TREE_HEIGHT }> {
            root: *tree.storage_tree.root(),
            next_free_slot: tree.storage_tree.next_free_slot,
        },
        last_block_timestamp: 0,
    };
    let block_level = SystemBlockContext::new_for_test();
    let metadata = ZkMetadata {
        block_level,
        tx_level: TxLevelMetadata::default(),
        chain_config: ChainConfig::default(),
        _marker: std::marker::PhantomData,
    };
    let oracle = make_oracle_for_proofs_and_dumps_for_init_data(
        block_level,
        tree,
        InMemoryPreimageSource {
            inner: HashMap::new(),
        },
        TxListSource {
            transactions: VecDeque::new(),
        },
        NoFriProofSidecar,
        None,
        Some(proof_data),
        None,
        false,
        RunMode::NativeRunOnly,
    );
    System::init_from_metadata_and_oracle(metadata, oracle).unwrap()
}

type FrameResources = <ForwardRunningSystem as SystemTypes>::Resources;

fn resources(gas: u64, native: u64) -> FrameResources {
    FrameResources::from_ergs_and_native(
        <FrameResources as Resources>::Ergs::from_legacy_gas_saturating(gas),
        <FrameResources as Resources>::Native::from_computational(native),
    )
}

fn native_of(resources: &FrameResources) -> u64 {
    resources.native().as_u64()
}

/// A frame heap: the interpreter wants it 32-byte aligned
#[repr(align(32))]
struct FrameHeap([MaybeUninit<u8>; 1024]);

/// A preimage buffer, aligned for the words of its bitmap
#[repr(align(8))]
struct Preimage([u8; 64]);

/// The native a frame that ended successfully gave back
fn native_left(end: CompletedExecution<'_, ForwardRunningSystem>) -> u64 {
    assert!(!end.failed());
    native_of(&end.resources_returned)
}

/// Runs of the interpreter that the bootloader never makes, driven directly: `new` then runs
/// (on the placeholder code, which is empty), and more runs of a frame that ran off the end of
/// its code, also after a call at its last byte returned. This is the host, which checks the end
/// of the code at every fetch; the proving target reads the zero byte after the code instead,
/// and moves the pointer of a run back to the end of the code. Every run executes a `STOP`,
/// which charges its native and nothing else.
#[test]
fn code_end_runs_outside_the_bootloader() {
    let mut system = bare_system();
    let mut hooks = HooksStorage::new_in(system.get_allocator());
    let mut tracer = NopTracer::default();

    // `new` then runs: the host's pointer moves on past the empty code, one byte per run
    let mut placeholder = Interpreter::<ForwardRunningSystem>::new(&mut system).unwrap();
    for runs in 1..=3 {
        placeholder.reclaim_resources(resources(0, STEP_NATIVE_COST));
        let exit = placeholder
            .run(&mut system, &mut hooks, &mut tracer)
            .unwrap();
        assert_eq!(exit, ExitCode::Stop);
        assert_eq!(native_of(&placeholder.hot.resources), 0);
        assert_eq!(placeholder.hot.instruction_pointer, runs);
    }
    // the STOP's native is charged: one short fails the run
    placeholder.reclaim_resources(resources(0, STEP_NATIVE_COST - 1));
    let exit = placeholder
        .run(&mut system, &mut hooks, &mut tracer)
        .unwrap();
    assert!(matches!(exit, ExitCode::FatalRuntime(_)), "{exit:?}");

    system.start_global_frame().unwrap();
    // PUSH1 1 PUSH1, whose immediate is cut off; and a CALL at the last byte
    let mut call_at_end = vec![0x5f; 5];
    call_at_end.push(0x73);
    call_at_end.extend_from_slice(CALLEE.as_slice());
    call_at_end.extend_from_slice(&[0x5a, 0xf1]);
    for code in [vec![0x60, 0x01, 0x60], call_at_end] {
        for code_version in [
            DEFAULT_CODE_VERSION_BYTE,
            ARTIFACTS_CACHING_CODE_VERSION_BYTE,
        ] {
            // the layout of `code_version`: the code alone, or `code || padding || bitmap`
            let mut preimage = Box::new(Preimage([0; 64]));
            preimage.0[..code.len()].copy_from_slice(&code);
            let (preimage_len, artifacts_len) = if code_version == DEFAULT_CODE_VERSION_BYTE {
                (code.len(), 0)
            } else {
                let mut bitmap = vec![0usize; artifacts_byte_len(code.len()) / size_of::<usize>()];
                analyze_into(&code, &mut bitmap);
                let padded_len = code.len().next_multiple_of(8);
                let bitmap: Vec<u8> = bitmap.iter().flat_map(|w| w.to_le_bytes()).collect();
                preimage.0[padded_len..padded_len + bitmap.len()].copy_from_slice(&bitmap);
                (padded_len + bitmap.len(), bitmap.len())
            };
            let mut heap = Box::new(FrameHeap([MaybeUninit::uninit(); 1024]));
            let mut frame = Interpreter::<ForwardRunningSystem>::new(&mut system).unwrap();
            let launch = ExecutionEnvironmentLaunchParams {
                external_call: ExternalCallRequest {
                    available_resources: resources(100_000, 1_000_000),
                    ergs_to_pass: resources(100_000, 0).ergs(),
                    caller: B160::from_be_bytes(CALLEE.into_array()),
                    callee: B160::from_be_bytes(CONTRACT.into_array()),
                    callers_caller: B160::ZERO,
                    modifier: CallModifier::NoModifier,
                    input: &[],
                    nominal_token_value: U256::ZERO,
                    call_scratch_space: None,
                },
                environment_parameters: EnvironmentParameters {
                    scratch_space_len: 0,
                    callstack_depth: 1,
                    callee_account_properties: CalleeAccountProperties {
                        nominal_token_balance: U256::ZERO,
                        nonce: 0,
                        bytecode: BytecodeData::Available {
                            bytecode: &preimage.0[..preimage_len],
                            unpadded_code_len: code.len() as u32,
                            artifacts_len: artifacts_len as u32,
                        },
                        ee_type: ExecutionEnvironmentType::EVM_EE_BYTE,
                        code_version,
                    },
                },
            };
            // the temporary that borrows `frame` ends with the statement
            let call = match frame
                .start_executing_frame(
                    &mut system,
                    &mut hooks,
                    launch,
                    SliceVec::new(&mut heap.0),
                    &mut tracer,
                )
                .unwrap()
            {
                // the call returns at once, with what it was given
                ExecutionEnvironmentPreemptionPoint::CallRequest { request, .. } => {
                    Ok(request.available_resources)
                }
                ExecutionEnvironmentPreemptionPoint::End(end) => Err(native_left(end)),
            };
            let left = match call {
                Err(left) => left,
                Ok(returned) => match frame
                    .continue_after_preemption(
                        &mut system,
                        &mut hooks,
                        returned,
                        CallResult::Successful {
                            return_values: ReturnValues::empty(),
                        },
                        &mut tracer,
                    )
                    .unwrap()
                {
                    ExecutionEnvironmentPreemptionPoint::End(end) => native_left(end),
                    ExecutionEnvironmentPreemptionPoint::CallRequest { .. } => {
                        panic!("a second call")
                    }
                },
            };
            assert!(left > 0);
            let pointer = frame.hot.instruction_pointer;
            assert!(pointer > code.len(), "{pointer}");
            // more runs of the frame that ended: a STOP each
            for runs in 1..=2 {
                frame.reclaim_resources(resources(0, STEP_NATIVE_COST));
                let exit = frame.run(&mut system, &mut hooks, &mut tracer).unwrap();
                assert_eq!(exit, ExitCode::Stop);
                assert_eq!(native_of(&frame.hot.resources), 0);
                assert_eq!(frame.hot.instruction_pointer, pointer + runs);
            }
        }
    }
}
