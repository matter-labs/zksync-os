#![cfg(test)]

//! Gas budgets of EVM frames of the Ethereum STF at and around 2^31 and 2^32 gas, and up to
//! the largest ergs. The interpreter charges these resources (gas only, see
//! `Resources::ERGS_ONLY`) on the low 32-bit word of the gas where it can, and runs the original
//! charge otherwise, so these cases cover the edges of that word with real instructions: low
//! words of 2^31 and more, a charge that would borrow from the high word, exact exhaustion, the
//! GAS opcode, heap growth across 2^31, the SSTORE stipend check, and CALL and CREATE passing
//! budgets across 2^31 and taking back what the callee left.
//!
//! The frames run directly on the Ethereum proving system types (`run_single_interaction`, as
//! `malicious_oracle::eth_code_padding` does): an Ethereum block run caps a transaction at the
//! chain config's `max_tx_gas_limit` (2^24 by default, and `Chain::run_eth_block` uses the
//! default), and that cap only bounds the budget the entry frame starts with, which is given
//! here directly. The metadata is built with a chain config whose cap is raised to 2^62.
//!
//! Each case pins the result, the gas left, the return data and a digest of every step the
//! opcode logger saw (depth, pc, opcode, gas before the step) against `eth_gas_budgets.golden`,
//! captured with the interpreter before the low-word charge. To regenerate it, run with
//! `ETH_GAS_BUDGETS_GOLDEN_OUT=<dir>` and concatenate the files written there, in case order.

use rig::alloy::consensus::Header;
use rig::alloy::primitives::B256;
use rig::alloy_rlp::Encodable;
use rig::basic_bootloader::bootloader::block_flow::ethereum::EthereumMetadataOp;
use rig::basic_bootloader::bootloader::block_flow::MetadataInitOp;
use rig::basic_bootloader::bootloader::config::BasicBootloaderForwardETHLikeConfig;
use rig::basic_bootloader::bootloader::runner::RunnerMemoryBuffers;
use rig::basic_bootloader::bootloader::transaction_flow::ethereum::EthereumTransactionFlow;
use rig::basic_bootloader::bootloader::{
    BasicBootloader, MAX_HEAP_BUFFER_SIZE, MAX_RETURN_BUFFER_SIZE,
};
use rig::basic_system::system_implementation::ethereum_storage_model::caches::account_properties::EthereumAccountProperties;
use rig::basic_system::system_implementation::ethereum_storage_model::EMPTY_ROOT_HASH;
use rig::forward_system::run::query_processors::{
    DACommitmentSchemeResponder, EthereumCLResponder, EthereumTargetBlockHeaderResponder,
    GenericPreimageResponder, InMemoryEthereumInitialAccountStateResponder,
    InMemoryEthereumInitialStorageSlotValueResponder,
};
use rig::forward_system::run::test_impl::InMemoryPreimageSource;
use rig::forward_system::system::system_types::ethereum::EthereumStorageSystemTypesWithPostOps;
use rig::forward_system::system::tracers::evm_opcodes_logger::EvmOpcodesLogger;
use rig::oracle_provider::{RunMode, ZkEENonDeterminismSource};
use rig::ruint::aliases::{B160, U256};
use rig::zk_ee::common_structs::da_commitment_scheme::DACommitmentScheme;
use rig::zk_ee::common_structs::system_hooks::HooksStorage;
use rig::zk_ee::system::metadata::chain_config::ChainConfig;
use rig::zk_ee::system::tracer::Tracer;
use rig::zk_ee::system::validator::NopTxValidator;
use rig::zk_ee::system::{CallResult, Ergs, IOSubsystemExt, Resources, System, SystemTypes};
use rig::zk_ee::utils::Bytes32;
use std::collections::{BTreeMap, HashMap};

type EthProvingTypes = EthereumStorageSystemTypesWithPostOps<ZkEENonDeterminismSource>;
type R = <EthProvingTypes as SystemTypes>::Resources;

const CALLER: B160 = B160::from_limbs([0x1000, 0, 0]);
const ENTRY: B160 = B160::from_limbs([0x2000, 0, 0]);
const CALLEE: B160 = B160::from_limbs([0x3000, 0, 0]);

const LOW_WORD: u64 = 1 << 32;
const SIGN: u64 = 1 << 31;

/// Largest ergs (`Ergs::MAX_ERGS`)
const MAX_ERGS: u64 = i64::MAX as u64;

/// `GAS PUSH0 MSTORE`, a loop of 10 rounds (`JUMPI` back to a `JUMPDEST`), `GAS` at 0x20, a
/// `JUMP` to a `JUMPDEST`, `GAS` at 0x40, then the three readings are the output (`RETURN`, or
/// `REVERT` if `revert`)
fn gas_readings(revert: bool) -> Vec<u8> {
    let mut code = vec![
        0x5a, 0x5f, 0x52, // GAS PUSH0 MSTORE
        0x60, 0x0a, // PUSH1 10
        0x5b, // 5: JUMPDEST
        0x60, 0x01, 0x90, 0x03, // PUSH1 1 SWAP1 SUB
        0x80, 0x61, 0x00, 0x05, 0x57, // DUP1 PUSH2 5 JUMPI
        0x50, // POP
        0x5a, 0x60, 0x20, 0x52, // GAS PUSH1 0x20 MSTORE
        0x61, 0x00, 0x18, 0x56, // PUSH2 24 JUMP
        0x5b, // 24: JUMPDEST
        0x5a, 0x60, 0x40, 0x52, // GAS PUSH1 0x40 MSTORE
        0x60, 0x60, 0x5f, // PUSH1 0x60 PUSH0
    ];
    code.push(if revert { 0xfd } else { 0xf3 }); // RETURN or REVERT
    code
}

/// Calls the address in calldata word 1 with the gas in word 0, keeping 32 bytes of return data
/// at 0, then returns that word, the call's success flag, the gas left and the return data size
fn call_driver() -> Vec<u8> {
    vec![
        0x60, 0x20, 0x5f, 0x5f, 0x5f, 0x5f, // CALL's sizes, offsets and value
        0x60, 0x20, 0x35, // the callee
        0x5f, 0x35, // the gas
        0xf1, // CALL
        0x60, 0x20, 0x52, // success at 0x20
        0x5a, 0x60, 0x40, 0x52, // GAS at 0x40
        0x3d, 0x60, 0x60, 0x52, // RETURNDATASIZE at 0x60
        0x60, 0x80, 0x5f, 0xf3, // RETURN 0x80 bytes
    ]
}

/// CREATE of `gas_readings(true)` (which reverts with its readings), then returns the readings
/// (the return data), the created address (zero) and the gas left
fn create_driver() -> Vec<u8> {
    let init = gas_readings(true);
    let n = init.len() as u8;
    let mut code = vec![
        0x60, n, 0x60, 0x00, 0x5f, 0x39, // CODECOPY(0, <init>, n), offset patched below
        0x60, n, 0x5f, 0x5f, 0xf0, // CREATE(0, 0, n)
        0x60, 0x80, 0x52, // the address at 0x80
        0x5a, 0x60, 0xa0, 0x52, // GAS at 0xa0
        0x3d, 0x5f, 0x5f, 0x3e, // RETURNDATACOPY(0, 0, RETURNDATASIZE)
        0x60, 0xc0, 0x5f, 0xf3, // RETURN 0xc0 bytes
    ];
    code[3] = code.len() as u8;
    code.extend(init);
    code
}

/// `GAS PUSH0 MSTORE`, a store at 0x10000 (64 KiB of heap growth), `GAS` at 0x20, then the two
/// readings
fn heap_growth() -> Vec<u8> {
    vec![
        0x5a, 0x5f, 0x52, // GAS PUSH0 MSTORE
        0x5a, 0x62, 0x01, 0x00, 0x00, 0x52, // GAS PUSH3 0x10000 MSTORE
        0x5a, 0x60, 0x20, 0x52, // GAS PUSH1 0x20 MSTORE
        0x60, 0x40, 0x5f, 0xf3, // RETURN 0x40 bytes
    ]
}

/// The offset of the growing store in `heap_growth`
const HEAP_GROWTH_PC: usize = 8;

/// `SSTORE(0, 1)` after 5 gas of pushes, then `GAS` as the output
fn sstore() -> Vec<u8> {
    vec![
        0x60, 0x01, 0x5f, 0x55, // PUSH1 1 PUSH0 SSTORE
        0x5a, 0x5f, 0x52, 0x60, 0x20, 0x5f, 0xf3, // return GAS
    ]
}

fn keccak(bytes: &[u8]) -> Bytes32 {
    use rig::crypto::MiniDigest;
    Bytes32::from_array(*rig::crypto::sha3::Keccak256::digest(bytes))
}

/// A header the Ethereum STF parses (it has every field up to `requests_hash`)
fn pectra_header(number: u64) -> Header {
    Header {
        number,
        gas_limit: 1 << 62,
        base_fee_per_gas: Some(7),
        withdrawals_root: Some(B256::ZERO),
        blob_gas_used: Some(0),
        excess_blob_gas: Some(0),
        parent_beacon_block_root: Some(B256::ZERO),
        requests_hash: Some(B256::ZERO),
        ..Default::default()
    }
}

fn rlp(header: &Header) -> Vec<u8> {
    let mut out = vec![];
    header.encode(&mut out);
    out
}

/// What a frame did
struct Outcome {
    line: String,
    left: u64,
    steps: Vec<(usize, usize, u8, u64)>,
}

/// Runs `entry_code` (with `callee_code` at `CALLEE`) as the entry frame of a transaction, with
/// `budget` gas
fn run(entry_code: &[u8], callee_code: &[u8], calldata: &[u8], budget: u64) -> Outcome {
    let account = |code: &[u8]| EthereumAccountProperties {
        nonce: 1,
        bytecode_hash: keccak(code),
        ..EthereumAccountProperties::EMPTY_BUT_EXISTING_ACCOUNT
    };
    let accounts = HashMap::from([
        (
            CALLER,
            EthereumAccountProperties {
                balance: U256::from(1u64 << 40),
                ..EthereumAccountProperties::EMPTY_BUT_EXISTING_ACCOUNT
            },
        ),
        (ENTRY, account(entry_code)),
        (CALLEE, account(callee_code)),
    ]);
    let preimages = HashMap::from([
        (keccak(entry_code), entry_code.to_vec()),
        (keccak(callee_code), callee_code.to_vec()),
    ]);
    let parent = pectra_header(0);
    let target = pectra_header(1);

    let mut oracle = ZkEENonDeterminismSource::new(RunMode::NativeRunOnly);
    oracle.add_external_processor(EthereumTargetBlockHeaderResponder {
        target_header_encoding: rlp(&target),
        target_header: target,
    });
    oracle.add_external_processor(EthereumCLResponder {
        withdrawals_list: vec![],
        parent_headers_encodings_list: vec![rlp(&parent)],
        parent_headers_list: vec![parent],
    });
    oracle.add_external_processor(DACommitmentSchemeResponder {
        da_commitment_scheme: Some(DACommitmentScheme::None),
    });
    // an empty state root: accounts not listed (the created one) are empty
    oracle.add_external_processor(InMemoryEthereumInitialAccountStateResponder::new(
        *EMPTY_ROOT_HASH.as_u8_array_ref(),
        accounts.clone(),
        BTreeMap::new(),
    ));
    oracle.add_external_processor(InMemoryEthereumInitialStorageSlotValueResponder::new(
        accounts,
        BTreeMap::new(),
    ));
    oracle.add_external_processor(GenericPreimageResponder {
        preimage_source: InMemoryPreimageSource { inner: preimages },
    });

    let chain_config = ChainConfig::new(1, false, 1 << 62).expect("chain config");
    let metadata = <EthereumMetadataOp as MetadataInitOp<EthProvingTypes>>::metadata_op::<
        BasicBootloaderForwardETHLikeConfig,
    >(&mut oracle, Default::default(), chain_config)
    .expect("block metadata");
    let mut system = System::<EthProvingTypes>::init_from_metadata_and_oracle(metadata, oracle)
        .expect("system init");
    system.io.begin_next_tx();
    let mut hooks = HooksStorage::new_in(system.get_allocator());

    // every frame's heap must be 32 bytes aligned, as the bootloader sets it up
    let mut heaps = Box::new_uninit_slice(MAX_HEAP_BUFFER_SIZE + 32);
    let heaps_start = heaps.as_ptr().addr().next_multiple_of(32) - heaps.as_ptr().addr();
    let mut return_data = Box::new_uninit_slice(MAX_RETURN_BUFFER_SIZE);
    let memories = RunnerMemoryBuffers {
        heaps: &mut heaps[heaps_start..heaps_start + MAX_HEAP_BUFFER_SIZE],
        return_data: &mut return_data,
    };

    let mut tracer = EvmOpcodesLogger::<EthProvingTypes>::default();
    tracer.begin_tx(calldata);
    let completed = BasicBootloader::<EthProvingTypes, EthereumTransactionFlow<EthProvingTypes>>::run_single_interaction(
        &mut system,
        &mut hooks,
        memories,
        calldata,
        &CALLER,
        &ENTRY,
        R::from_ergs(Ergs(budget)),
        &U256::ZERO,
        true,
        &mut tracer,
        &mut NopTxValidator,
    )
    .expect("the call runs");

    let left = completed.resources_returned.ergs().0;
    let (result, data) = match &completed.result {
        CallResult::Successful { return_values } => ("success", return_values.returndata),
        CallResult::Failed { return_values } => ("failed", return_values.returndata),
        CallResult::PreparationStepFailed => ("preparation failed", &[][..]),
    };
    let steps: Vec<_> = tracer
        .transaction_logs
        .iter()
        .flat_map(|log| &log.steps)
        .map(|s| (s.depth, s.pc, s.opcode_raw, s.gas))
        .collect();
    // FNV-1a over the steps
    let mut digest = 0xcbf2_9ce4_8422_2325u64;
    for step in &steps {
        for byte in format!("{step:?};").bytes() {
            digest = (digest ^ byte as u64).wrapping_mul(0x0100_0000_01b3);
        }
    }
    let line = format!(
        "budget {budget:#x}: {result}, left {left:#x}, data {}, {} steps {digest:016x}",
        hex::encode(data),
        steps.len()
    );
    Outcome { line, left, steps }
}

/// Compares the lines of `case` with the golden file (or writes them, see the module docs)
fn check(case: &str, lines: Vec<String>) {
    let lines: Vec<String> = lines.into_iter().map(|l| format!("{case} {l}")).collect();
    if let Ok(dir) = std::env::var("ETH_GAS_BUDGETS_GOLDEN_OUT") {
        let mut text = lines.join("\n");
        text.push('\n');
        std::fs::write(format!("{dir}/{case}.golden"), text).expect("write the golden lines");
        return;
    }
    let golden: Vec<&str> = include_str!("eth_gas_budgets.golden")
        .lines()
        .filter(|l| l.split(' ').next() == Some(case))
        .collect();
    assert!(!golden.is_empty(), "no golden lines for {case}");
    assert_eq!(lines, golden, "{case}");
}

/// Budgets with a low word of 2^31 and more, at and around 2^31, 2^32 and their multiples, and
/// up to the largest ergs; each plus `spent` and around it, so that the frame reaches 2^31 (or
/// the next multiple of 2^32) during its run
fn edge_budgets(spent: u64) -> Vec<u64> {
    let mut budgets = vec![];
    for base in [
        SIGN,
        LOW_WORD,
        LOW_WORD + SIGN,
        3 * LOW_WORD,
        1 << 40,
        MAX_ERGS - spent - 2,
    ] {
        for delta in [0, 1, 2, spent / 2, spent - 1, spent, spent + 1] {
            budgets.extend([base + delta, base - 1 - delta.min(base - 1)]);
        }
    }
    budgets.extend([MAX_ERGS - 1, MAX_ERGS]);
    budgets.sort();
    budgets.dedup();
    budgets
}

/// What a run of `code` spends with plenty of gas
fn spent_by(code: &[u8], callee: &[u8], calldata: &[u8]) -> u64 {
    let budget = 1 << 30;
    budget - run(code, callee, calldata, budget).left
}

#[test]
fn gas_readings_across_the_edges() {
    let code = gas_readings(false);
    let spent = spent_by(&code, &[], &[]);
    let mut lines = vec![];
    // exact exhaustion, and one short
    for budget in [spent, spent - 1, spent + 1] {
        lines.push(run(&code, &[], &[], budget).line);
    }
    for budget in edge_budgets(spent) {
        let outcome = run(&code, &[], &[], budget);
        assert_eq!(outcome.left, budget - spent, "{budget:#x}");
        lines.push(outcome.line);
    }
    check("gas_readings", lines);
}

#[test]
fn heap_growth_across_the_edges() {
    let code = heap_growth();
    let spent = spent_by(&code, &[], &[]);
    // the gas before the growing store and its cost, from a run with plenty
    let reference = run(&code, &[], &[], 1 << 30);
    let at = reference
        .steps
        .iter()
        .position(|s| s.1 == HEAP_GROWTH_PC)
        .expect("the store runs");
    let before = (1u64 << 30) - reference.steps[at].3;
    let cost = reference.steps[at].3 - reference.steps[at + 1].3;
    let mut lines = vec![];
    // the growth's charge fails, or leaves nothing
    for budget in [before + cost - 1, before + cost, spent - 1, spent] {
        lines.push(run(&code, &[], &[], budget).line);
    }
    // the growth takes the frame across 2^31 and 2^32
    for base in [SIGN, LOW_WORD, LOW_WORD + SIGN] {
        for delta in [0, 1, cost / 2, cost - 1, cost, cost + 1] {
            lines.push(run(&code, &[], &[], base + before + delta).line);
        }
    }
    for budget in edge_budgets(spent) {
        lines.push(run(&code, &[], &[], budget).line);
    }
    check("heap_growth", lines);
}

#[test]
fn sstore_stipend_across_the_edges() {
    let code = sstore();
    let mut lines = vec![];
    // the stipend check sees 2299, 2300, 2301 and 2302 gas
    for left in [2299, 2300, 2301, 2302] {
        lines.push(run(&code, &[], &[], left + 5).line);
    }
    for budget in [SIGN, SIGN + 4, SIGN + 5, SIGN + 6, LOW_WORD + 5, MAX_ERGS] {
        lines.push(run(&code, &[], &[], budget).line);
    }
    check("sstore_stipend", lines);
}

fn call_data(gas: u64) -> Vec<u8> {
    let mut calldata = U256::from(gas).to_be_bytes::<32>().to_vec();
    calldata.extend_from_slice(&[0; 12]);
    calldata.extend_from_slice(&CALLEE.to_be_bytes::<20>());
    calldata
}

#[test]
fn call_across_the_edges() {
    let code = call_driver();
    let callee = gas_readings(false);
    let mut lines = vec![];
    // the callee starts at and around 2^31 and 2^32 (an explicit gas under 63/64 of what is
    // left), and the caller is at its edges when the callee gives back what it left
    for gas in [
        SIGN - 2,
        SIGN - 1,
        SIGN,
        SIGN + 1,
        LOW_WORD - 1,
        LOW_WORD,
        LOW_WORD + 1,
    ] {
        for budget in [1 << 34, SIGN + (SIGN >> 5), LOW_WORD + (LOW_WORD >> 5)] {
            lines.push(run(&code, &callee, &call_data(gas), budget).line);
        }
    }
    // all the gas (63/64 of it reaches the callee), from caller budgets across the edges
    let spent = spent_by(&code, &callee, &call_data(u64::MAX));
    for budget in edge_budgets(spent) {
        lines.push(run(&code, &callee, &call_data(u64::MAX), budget).line);
    }
    // the callee runs out exactly, or one short
    let callee_spent = spent_by(&callee, &[], &[]);
    for gas in [callee_spent - 1, callee_spent] {
        lines.push(run(&code, &callee, &call_data(gas), SIGN + 7).line);
    }
    check("call", lines);
}

#[test]
fn create_across_the_edges() {
    let code = create_driver();
    let mut lines = vec![];
    // CREATE passes 63/64 of what is left: caller budgets so that the init code starts at and
    // around 2^31 and 2^32
    let spent = spent_by(&code, &[], &[]);
    for edge in [SIGN, LOW_WORD] {
        let budget = edge + (edge >> 6) + 2;
        for delta in [0, 1, 2, 64, 65, 66, spent] {
            lines.push(run(&code, &[], &[], budget + delta).line);
            lines.push(run(&code, &[], &[], budget - delta).line);
        }
    }
    for budget in edge_budgets(spent) {
        lines.push(run(&code, &[], &[], budget).line);
    }
    check("create", lines);
}
