//! Running one case against ZKsync OS and, for gas-agnostic cases, REVM.
//!
//! Per case: a fresh [`TestingFramework`] with the block context for the mode,
//! two prefunded deterministic signers, a first block that deploys the program
//! (a CREATE transaction carrying the initcode), then a second block with the
//! program's calls against the deployed address. Each block's wall time is
//! measured, and the REVM differential comparison is run here (the rig's
//! built-in check is disabled) on a clone of the chain taken before the block,
//! exactly like `evm_divergence_validator`.
//!
//! Gas-agnostic modes disable native correction and avoid GAS reads. Comparisons
//! can still differ at nested gas limits; see the README for triage semantics.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::str::FromStr;
use std::time::Instant;

use program_gen::{wrap_runtime_as_initcode, CallPlan, Mode, Program};
use rig::alloy::consensus::{TxEip1559, TxEip7702};
use rig::alloy::eips::eip7702::{Authorization, SignedAuthorization};
use rig::alloy::primitives::{Address, TxKind, U256 as AlloyU256};
use rig::alloy::signers::local::PrivateKeySigner;
use rig::alloy::signers::{Signer, SignerSync};
use rig::chain::{Chain, RunConfig};
use rig::constants::{DEFAULT_MAX_FEE, DEFAULT_NATIVE_PRICE, DEFAULT_PRIORITY_FEE, TEST_CHAIN_ID};
use rig::forward_system::run::output::BlockOutput;
use rig::revm_consistency_checker::{generate_block_context_interface, ChainStateView};
use rig::ruint::aliases::U256;
use rig::{BlockContext, TestingFramework};
use zk_ee::system::errors::root_cause::{GetRootCause, RootCause};
use zksync_os_revm_runner::revm_runner::RevmRunner;
use zksync_os_tests_common::zksync_tx::ZKsyncTxEnvelope;

use crate::classify::Outcome;

/// Role tags mixed into the per-case seed so the deployer and caller keys never
/// coincide while both stay a deterministic function of the case seed. The
/// secondary authority and the empty-target address exist only for the richer
/// 7702 delegation structures (multiple authorities, chained and empty targets).
const DEPLOYER_ROLE: u64 = 0x01;
const CALLER_ROLE: u64 = 0x02;
const SECONDARY_AUTHORITY_ROLE: u64 = 0x03;
const EMPTY_TARGET_ROLE: u64 = 0x04;

/// Balance funded to each signer. Large enough that gas and any transferred
/// value are always affordable; the excess is identical baseline state on both
/// engines and is never part of the comparison.
const FUNDING_WEI: u128 = 1_000_000_000_000_000_000_000_000;

/// The result of running one case.
pub struct CaseExecution {
    pub outcome: Outcome,
    pub deploy_wall_ms: u128,
    pub calls_wall_ms: u128,
    pub block_context: BlockContext,
}

/// Run a case in the forward path, catching a panic as a last-resort finding.
pub fn run_case_forward_catching(
    program: &Program,
    mode: Mode,
    slow_ms: u64,
    case_seed: u64,
) -> CaseExecution {
    let result = catch_unwind(AssertUnwindSafe(|| {
        run_case_inner(program, mode, slow_ms, case_seed, false)
    }));
    result.unwrap_or_else(|payload| CaseExecution {
        outcome: Outcome::Panic {
            message: panic_message(payload.as_ref()),
        },
        deploy_wall_ms: 0,
        calls_wall_ms: 0,
        block_context: block_context_for_mode(mode),
    })
}

/// Run a case in the proving (RISC-V) path.
///
/// Deliberately does not `catch_unwind`: a forward/proving divergence asserts
/// inside the rig, and that assertion must abort this (child) process so the
/// parent observes a non-zero exit rather than a swallowed panic.
pub fn run_case_riscv(
    program: &Program,
    mode: Mode,
    slow_ms: u64,
    case_seed: u64,
) -> CaseExecution {
    run_case_inner(program, mode, slow_ms, case_seed, true)
}

fn run_case_inner(
    program: &Program,
    mode: Mode,
    slow_ms: u64,
    case_seed: u64,
    riscv: bool,
) -> CaseExecution {
    let bc = block_context_for_mode(mode);
    // The proving path uses the rig's forward-vs-RISC-V check instead of REVM.
    let compare = matches!(mode, Mode::GasAgnostic | Mode::Delegation7702) && !riscv;

    let mut run_config = if riscv {
        RunConfig::with_riscv_run()
    } else {
        RunConfig::without_riscv_run()
    };
    if !riscv {
        // Skip the second, proving-mode execution the rig runs to build prover
        // input; the forward run alone yields the block output and updates state,
        // which roughly halves per-case cost.
        run_config.do_prover_input_run = false;
    }
    // We run REVM ourselves (or, in the proving path, rely on the rig's internal
    // forward/proving check), so the built-in REVM check is always off here.
    run_config.disable_revm_consistency_check();

    let chain_id = TEST_CHAIN_ID;
    let mut tester = TestingFramework::new()
        .with_system_contracts(true, true)
        .with_run_config(run_config)
        .with_block_context(bc.clone());

    // Mirror ZKsync OS's per-tx gas cap into REVM so the checker never diverges
    // on transaction admission (EIP-7825).
    let tx_gas_cap = tester.chain_config().max_tx_gas_limit();

    let deployer = deterministic_signer(case_seed, DEPLOYER_ROLE, chain_id);
    let caller = deterministic_signer(case_seed, CALLER_ROLE, chain_id);
    let funding = U256::from(FUNDING_WEI);
    tester.set_balance(deployer.address(), funding);
    tester.set_balance(caller.address(), funding);

    // Block 1: deploy the program at the deployer's nonce 0; the deployed address
    // follows from the deployer and that nonce. In the 7702 delegation mode a plan
    // derived from the case seed decides the delegation structure, and when it asks
    // to observe the delegated EOA the observation probe is deployed alongside, at
    // the deployer's nonce 1.
    let plan = matches!(mode, Mode::Delegation7702).then(|| DelegationPlan::derive(case_seed));
    let deployed_addr = deployer.address().create(0);

    let mut block1 = vec![create_tx(
        &deployer,
        0,
        tx_gas_cap,
        program.initcode.clone(),
        chain_id,
    )];
    let probe_addr = match &plan {
        Some(p) if p.observe => {
            block1.push(create_tx(
                &deployer,
                1,
                tx_gas_cap,
                wrap_runtime_as_initcode(&probe_runtime()),
                chain_id,
            ));
            Some(deployer.address().create(1))
        }
        _ => None,
    };
    // The deployer's next free nonce, after the program deploy and any probe deploy.
    let sender_next_nonce = block1.len() as u64;

    let mut internal: Option<(String, String)> = None;
    let mut comparison = RevmComparison::Clean;
    let mut deploy_wall_ms = 0u128;
    let mut calls_wall_ms = 0u128;

    match run_block(&mut tester, block1, compare, tx_gas_cap) {
        BlockRun::Internal { root_cause, detail } => internal = Some((root_cause, detail)),
        BlockRun::Ran {
            wall_ms,
            comparison: c,
        } => {
            deploy_wall_ms = wall_ms;
            comparison = c;
        }
    }

    // Block 2. Normal modes: the program's calls against the deployed contract.
    // Delegation7702: build the setcode transaction(s) and calls for the plan's
    // delegation structure, driving calls at the EOA's own address so the
    // delegated code runs in the EOA's context. Skipped if the deploy already
    // errored, or if there is nothing to run.
    if internal.is_none() {
        let call_txs: Vec<ZKsyncTxEnvelope> = if let Some(plan) = &plan {
            build_delegation_block(
                &deployer,
                sender_next_nonce,
                &caller,
                deployed_addr,
                probe_addr,
                plan,
                &program.calls,
                case_seed,
                chain_id,
            )
        } else {
            program
                .calls
                .iter()
                .enumerate()
                .map(|(nonce, call)| call_tx(&caller, deployed_addr, nonce as u64, call, chain_id))
                .collect()
        };
        if call_txs.is_empty() {
            // Nothing to run in block 2 (a normal case with no calls left after shrinking).
        } else {
            match run_block(&mut tester, call_txs, compare, tx_gas_cap) {
                BlockRun::Internal { root_cause, detail } => internal = Some((root_cause, detail)),
                BlockRun::Ran {
                    wall_ms,
                    comparison: c,
                } => {
                    calls_wall_ms = wall_ms;
                    comparison = comparison.max(c);
                }
            }
        }
    }

    // Priority: an internal error outranks a candidate state divergence (S1), which
    // outranks a gas-schedule difference (S3) or slowness (S3), which outrank a
    // clean run. Independent-only discrepancies require triage.
    let max_wall = deploy_wall_ms.max(calls_wall_ms);
    let outcome = if let Some((root_cause, detail)) = internal {
        Outcome::InternalError { root_cause, detail }
    } else {
        match comparison {
            RevmComparison::StateDivergence(detail) => Outcome::RevmMismatch { detail },
            RevmComparison::GasScheduleDiff(detail) => Outcome::GasScheduleDiff { detail },
            RevmComparison::Clean if max_wall > slow_ms as u128 => Outcome::Slow {
                wall_ms: max_wall,
                slow_ms,
            },
            RevmComparison::Clean => Outcome::Ok,
        }
    };

    CaseExecution {
        outcome,
        deploy_wall_ms,
        calls_wall_ms,
        block_context: bc,
    }
}

/// The outcome of executing one block.
enum BlockRun {
    /// The bootloader returned a block-level internal error.
    Internal { root_cause: String, detail: String },
    /// The block executed; `comparison` classifies any REVM divergence.
    Ran {
        wall_ms: u128,
        comparison: RevmComparison,
    },
}

fn run_block(
    tester: &mut TestingFramework,
    txs: Vec<ZKsyncTxEnvelope>,
    compare: bool,
    tx_gas_cap: u64,
) -> BlockRun {
    // Snapshot the pre-block chain and context for the REVM replay before
    // execution, mirroring evm_divergence_validator. Unchanged accounts (such as
    // the pre-minted treasury) never enter the comparison, so a pre-mint clone is
    // sound.
    let pre_block_chain = tester.chain().clone();
    let block_context = tester.block_context().cloned().unwrap_or_default();

    let start = Instant::now();
    let output = match tester.execute_block_no_panic(txs.clone()) {
        Ok(output) => output,
        Err(err) => {
            let root_cause = match err.root_cause() {
                RootCause::Internal(_) => "internal",
                RootCause::Runtime(_) => "runtime",
                RootCause::Usage(_) => "usage",
            }
            .to_string();
            return BlockRun::Internal {
                root_cause,
                detail: format!("{err:?}"),
            };
        }
    };
    let wall_ms = start.elapsed().as_millis();

    let comparison = if compare {
        compare_against_revm(pre_block_chain, &block_context, txs, output, tx_gas_cap)
    } else {
        RevmComparison::Clean
    };
    BlockRun::Ran {
        wall_ms,
        comparison,
    }
}

/// The result of comparing a block against REVM in both gas modes.
#[derive(Clone)]
pub enum RevmComparison {
    /// The node and REVM agree in both modes.
    Clean,
    /// A divergence that survives comparing with the node's own gas (the node
    /// consistency-checker's mode): a candidate state divergence requiring triage.
    StateDivergence(String),
    /// A divergence that appears only under an independent standard-EVM gas
    /// schedule and disappears once the node's own gas is used: a gas-model
    /// difference (a spec question), reported informationally.
    GasScheduleDiff(String),
}

impl RevmComparison {
    /// Keeps the more serious of two comparisons (state divergence outranks a gas
    /// difference, which outranks clean), so a case is judged by its worst block.
    fn max(self, other: RevmComparison) -> RevmComparison {
        match (&self, &other) {
            (RevmComparison::StateDivergence(_), _) => self,
            (_, RevmComparison::StateDivergence(_)) => other,
            (RevmComparison::GasScheduleDiff(_), _) => self,
            (_, RevmComparison::GasScheduleDiff(_)) => other,
            _ => RevmComparison::Clean,
        }
    }
}

/// Compares a block against REVM twice: first with the node's own gas (the
/// consistency-checker's mode, which compares state),
/// then under an independent standard-EVM gas schedule. Independent-only
/// discrepancies are retained at lower severity for triage.
fn compare_against_revm(
    pre_block_chain: Chain<false>,
    block_context: &BlockContext,
    txs: Vec<ZKsyncTxEnvelope>,
    output: BlockOutput,
    tx_gas_cap: u64,
) -> RevmComparison {
    // The node consistency-checker's mode: REVM uses ZKsync OS's own `gas_used`,
    // which reduces fee-accounting noise but does not equalize nested gas budgets.
    if let Some(detail) = revm_diff(
        pre_block_chain.clone(),
        block_context,
        txs.clone(),
        output.clone(),
        tx_gas_cap,
        false,
    ) {
        return RevmComparison::StateDivergence(detail);
    }
    // The stronger, independent-gas comparison. A divergence here (that vanished
    // above) is retained for triage, including per-transaction mismatches.
    match revm_diff(
        pre_block_chain,
        block_context,
        txs,
        output,
        tx_gas_cap,
        true,
    ) {
        Some(detail) => RevmComparison::GasScheduleDiff(detail),
        None => RevmComparison::Clean,
    }
}

/// Runs one REVM comparison in the given gas mode and returns a human-readable
/// detail string on divergence.
fn revm_diff(
    pre_block_chain: Chain<false>,
    block_context: &BlockContext,
    txs: Vec<ZKsyncTxEnvelope>,
    output: BlockOutput,
    tx_gas_cap: u64,
    independent_gas: bool,
) -> Option<String> {
    let block_context_interface = generate_block_context_interface(&pre_block_chain, block_context);
    let mut runner = RevmRunner::new(ChainStateView {
        chain: pre_block_chain,
    })
    .with_independent_gas(independent_gas)
    .with_tx_gas_limit_cap(Some(tx_gas_cap));

    match runner.run_with_call_traces(txs, block_context_interface, Some(output)) {
        Ok((_traces, _invalid, compare_report, tx_mismatches)) => {
            let mut parts = Vec::new();
            if let Some(report) = compare_report {
                if !report.is_empty() {
                    parts.push(format!(
                        "state mismatch: storage={} account={}\n  storage_detail={:#?}\n  account_detail={:#?}",
                        report.storage.len(),
                        report.accounts.len(),
                        report.storage,
                        report.accounts,
                    ));
                }
            }
            if !tx_mismatches.is_empty() {
                parts.push(format!("per-transaction mismatch:\n  {tx_mismatches:#?}"));
            }
            if parts.is_empty() {
                None
            } else {
                Some(parts.join("\n"))
            }
        }
        // A runner error (bad conversion, header/database error) is not a
        // divergence proper, but on a well-formed block it is unexpected, so
        // surface it rather than dropping it silently.
        Err(err) => Some(format!("revm runner error (surfaced for triage): {err:#}")),
    }
}

/// The block context for a mode.
pub fn block_context_for_mode(mode: Mode) -> BlockContext {
    let native_price = if matches!(mode, Mode::GasAgnostic | Mode::Delegation7702) {
        // Unlimited native budget: disables the ZKsync OS native correction so
        // the gas model matches REVM, as the revm runner's docs require for the
        // sound (comparison) modes.
        U256::ZERO
    } else {
        U256::from(DEFAULT_NATIVE_PRICE)
    };
    BlockContext {
        native_price,
        ..BlockContext::default()
    }
}

fn create_tx(
    signer: &PrivateKeySigner,
    nonce: u64,
    gas_limit: u64,
    init_code: Vec<u8>,
    chain_id: u64,
) -> ZKsyncTxEnvelope {
    let tx = TxEip1559 {
        chain_id,
        nonce,
        max_fee_per_gas: DEFAULT_MAX_FEE,
        max_priority_fee_per_gas: DEFAULT_PRIORITY_FEE,
        gas_limit,
        to: TxKind::Create,
        value: AlloyU256::ZERO,
        access_list: Default::default(),
        input: init_code.into(),
    };
    ZKsyncTxEnvelope::from_eth_tx(tx, signer.clone())
}

fn call_tx(
    signer: &PrivateKeySigner,
    to: Address,
    nonce: u64,
    call: &CallPlan,
    chain_id: u64,
) -> ZKsyncTxEnvelope {
    let tx = TxEip1559 {
        chain_id,
        nonce,
        max_fee_per_gas: DEFAULT_MAX_FEE,
        max_priority_fee_per_gas: DEFAULT_PRIORITY_FEE,
        // The program's requested gas is passed through verbatim; a value above
        // the per-tx cap is rejected identically by both engines, which is a
        // consistent (non-finding) outcome.
        gas_limit: call.gas,
        to: TxKind::Call(to),
        value: AlloyU256::from(call.value),
        access_list: Default::default(),
        input: call.calldata.clone().into(),
    };
    ZKsyncTxEnvelope::from_eth_tx(tx, signer.clone())
}

/// Gas for a setcode or probe transaction the plan adds without a program call
/// of its own: enough for the delegation plus a run of the target's code in the
/// EOA's context.
const DELEGATION_DEFAULT_GAS: u64 = 3_000_000;

/// What an authority delegates its code to in a 7702 setcode transaction. Every
/// variant here keeps the differential comparison sound (REVM models them all);
/// a system or hook address, which REVM cannot model, is deliberately never a
/// target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DelegationTarget {
    /// The deployed generated program: calling the EOA runs that program in the
    /// EOA's context. This is the only target whose code the generated `calls`
    /// meaningfully exercise.
    Deployed,
    /// A precompile address (`0x01..=0x0a`): a designator pointing at an account
    /// with no EVM bytecode, so calling the EOA runs empty code.
    Precompile(u8),
    /// The zero address, which per EIP-7702 *clears* any delegation and leaves the
    /// account a plain EOA.
    Zero,
    /// The authority's own address: a self-referential designator. 7702 follows a
    /// designator only one hop, so calling the EOA loads the designator bytes
    /// (`0xef01..`) and executes them, which halts on the reserved `0xef` opcode.
    SelfAddress,
    /// A fresh, unused account with no code: another empty-code designator.
    Empty,
    /// A second EOA that is itself delegated (to the deployed program). Exercises
    /// the single-hop rule: calling the first EOA must not follow through to the
    /// program.
    Chained,
}

/// Extra authorizations bundled into the first setcode transaction, on top of the
/// primary authority→target one. Each is a differential probe: both engines must
/// apply the same 7702 validation rules, so if they disagree on which
/// authorizations take effect the resulting state diverges.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct AuthQuirks {
    /// A second, now-stale authorization for the primary authority (nonce 0
    /// again). Once the first has applied, the authority's nonce is 1, so this one
    /// is invalid and must be skipped by both engines.
    duplicate_primary: bool,
    /// An authorization with an unreachable nonce: always invalid, always skipped.
    invalid_nonce: bool,
    /// Also authorize a second, independent EOA to delegate (to the deployed
    /// program) in the same transaction — multiple authorities at once.
    second_authority: bool,
    /// Give the second authority's authorization chain id 0 (valid on any chain)
    /// rather than this chain's id, to exercise that acceptance rule.
    second_authority_any_chain: bool,
}

/// A per-case EIP-7702 delegation structure, derived deterministically from the
/// case seed. It selects what the primary authority delegates to, any extra
/// authorization quirks, an optional re-delegation/clearing step, and whether to
/// observe the delegated EOA through a probe contract. Being a pure function of
/// the case seed, it reproduces byte-for-byte on replay.
#[derive(Clone, Debug, PartialEq, Eq)]
struct DelegationPlan {
    target: DelegationTarget,
    quirks: AuthQuirks,
    /// A second setcode step that re-points or clears the primary authority's
    /// delegation. `None` leaves the first delegation in place.
    redelegate: Option<DelegationTarget>,
    /// Deploy an observation probe and call the delegated EOA through it, so the
    /// EOA's observable code size, code hash, copied code, balance and call
    /// behavior are compared across engines.
    observe: bool,
}

impl DelegationPlan {
    fn derive(case_seed: u64) -> Self {
        // About half the cases keep the deployed program as the target so the
        // generated program actually runs in the EOA; the rest spread across the
        // structural edges.
        let target = match seed_choice(case_seed, 0x7702_0001, 10) {
            0..=4 => DelegationTarget::Deployed,
            5 => DelegationTarget::Precompile(1 + seed_choice(case_seed, 0x7702_0002, 10) as u8),
            6 => DelegationTarget::Zero,
            7 => DelegationTarget::SelfAddress,
            8 => DelegationTarget::Empty,
            _ => DelegationTarget::Chained,
        };
        let quirks = AuthQuirks {
            duplicate_primary: seed_choice(case_seed, 0x7702_0003, 3) == 0,
            invalid_nonce: seed_choice(case_seed, 0x7702_0004, 3) == 0,
            second_authority: seed_choice(case_seed, 0x7702_0005, 3) == 0,
            second_authority_any_chain: seed_choice(case_seed, 0x7702_0006, 2) == 0,
        };
        let redelegate = match seed_choice(case_seed, 0x7702_0007, 4) {
            0 => Some(DelegationTarget::Zero),     // clear the delegation
            1 => Some(DelegationTarget::Deployed), // re-point it at the program
            _ => None,
        };
        let observe = seed_choice(case_seed, 0x7702_0008, 2) == 0;
        Self {
            target,
            quirks,
            redelegate,
            observe,
        }
    }
}

/// A deterministic choice in `[0, modulo)` from the case seed and a salt, so a
/// delegation plan reproduces exactly on replay. Splitmix64 over `seed ^ salt*C`,
/// the same mixing family the worker uses to derive per-case seeds.
fn seed_choice(case_seed: u64, salt: u64, modulo: u64) -> u64 {
    let mut z = case_seed ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    z % modulo
}

/// Resolve a [`DelegationTarget`] to the concrete address it delegates to.
fn resolve_target(
    target: DelegationTarget,
    deployed: Address,
    eoa: Address,
    secondary: Address,
    empty: Address,
) -> Address {
    match target {
        DelegationTarget::Deployed => deployed,
        DelegationTarget::Precompile(low) => {
            let mut bytes = [0u8; 20];
            bytes[19] = low;
            Address::from(bytes)
        }
        DelegationTarget::Zero => Address::ZERO,
        DelegationTarget::SelfAddress => eoa,
        DelegationTarget::Empty => empty,
        DelegationTarget::Chained => secondary,
    }
}

/// Builds block 2 for the EIP-7702 delegation mode from the plan: a type-4
/// setcode transaction that authorizes the primary `authority` (an EOA) to
/// delegate to the plan's target — plus any quirk authorizations and, for a
/// chained target, a second authority delegating to the deployed program — and,
/// in the same transaction, calls the EOA (so the delegated code runs in the
/// EOA's context); then an optional re-delegation/clearing setcode; then the
/// program's remaining calls, each aimed at the EOA; then, when the plan
/// observes, a call to the observation probe carrying the EOA's address.
///
/// `sender` (the deployer) sends every transaction, from `sender_start_nonce`,
/// so the tx nonce is decoupled from each authorization nonce (which must be the
/// authority's own). Because every transaction here is sent by `sender`, the tx
/// at position `i` in the returned list uses nonce `sender_start_nonce + i`.
#[allow(clippy::too_many_arguments)]
fn build_delegation_block(
    sender: &PrivateKeySigner,
    sender_start_nonce: u64,
    authority: &PrivateKeySigner,
    deployed: Address,
    probe: Option<Address>,
    plan: &DelegationPlan,
    calls: &[CallPlan],
    case_seed: u64,
    chain_id: u64,
) -> Vec<ZKsyncTxEnvelope> {
    let eoa = authority.address();
    let secondary = deterministic_signer(case_seed, SECONDARY_AUTHORITY_ROLE, chain_id);
    let empty = deterministic_signer(case_seed, EMPTY_TARGET_ROLE, chain_id).address();
    let primary_target = resolve_target(plan.target, deployed, eoa, secondary.address(), empty);

    let mut txs = Vec::new();

    // First setcode transaction: the primary authorization, plus any quirks.
    let mut auths = vec![signed_auth(authority, 0, primary_target, chain_id)];
    if plan.quirks.duplicate_primary {
        auths.push(signed_auth(authority, 0, primary_target, chain_id));
    }
    if plan.quirks.invalid_nonce {
        auths.push(signed_auth(authority, u64::MAX, primary_target, chain_id));
    }
    // A second, independent authority delegating to the program: present when the
    // plan asks for it, and always for a chained target so the target EOA is
    // itself delegated (making the chain real).
    if plan.quirks.second_authority || plan.target == DelegationTarget::Chained {
        let auth_chain = if plan.quirks.second_authority_any_chain {
            0
        } else {
            chain_id
        };
        auths.push(signed_auth(&secondary, 0, deployed, auth_chain));
    }

    // The first program call (or an empty call) drives the EOA in the setcode tx.
    let (calldata, value, gas) = match calls.first() {
        Some(c) => (c.calldata.clone(), c.value, c.gas),
        None => (Vec::new(), 0, DELEGATION_DEFAULT_GAS),
    };
    let nonce = sender_start_nonce + txs.len() as u64;
    txs.push(setcode_tx_multi(
        sender, nonce, auths, eoa, calldata, value, gas, chain_id,
    ));

    // Optional re-delegation / clearing. The authority applied exactly one valid
    // authorization above (the quirks are all invalid or for another authority),
    // so its account nonce is now 1.
    if let Some(redel) = plan.redelegate {
        let redirect = resolve_target(redel, deployed, eoa, secondary.address(), empty);
        let auth = signed_auth(authority, 1, redirect, chain_id);
        let nonce = sender_start_nonce + txs.len() as u64;
        txs.push(setcode_tx_multi(
            sender,
            nonce,
            vec![auth],
            eoa,
            Vec::new(),
            0,
            DELEGATION_DEFAULT_GAS,
            chain_id,
        ));
    }

    // The program's remaining calls, aimed at the EOA.
    for call in calls.iter().skip(1) {
        let nonce = sender_start_nonce + txs.len() as u64;
        txs.push(call_tx(sender, eoa, nonce, call, chain_id));
    }

    // Observation probe: call it with the EOA's address as calldata, so it records
    // what it observes about the delegated account.
    if let Some(probe) = probe {
        let probe_call = CallPlan {
            calldata: address_calldata(eoa),
            value: 0,
            gas: DELEGATION_DEFAULT_GAS,
        };
        let nonce = sender_start_nonce + txs.len() as u64;
        txs.push(call_tx(sender, probe, nonce, &probe_call, chain_id));
    }

    txs
}

/// Signs one EIP-7702 authorization: `authority` authorizes a delegation to
/// `delegate_to` at authorization nonce `nonce`, over `auth_chain_id` (0 means
/// valid on any chain).
fn signed_auth(
    authority: &PrivateKeySigner,
    nonce: u64,
    delegate_to: Address,
    auth_chain_id: u64,
) -> SignedAuthorization {
    let authorization = Authorization {
        chain_id: AlloyU256::from(auth_chain_id),
        address: delegate_to,
        nonce,
    };
    let signature = authority
        .sign_hash_sync(&authorization.signature_hash())
        .expect("signing an EIP-7702 authorization cannot fail for a local key");
    authorization.into_signed(signature)
}

/// Builds a signed EIP-7702 setcode transaction carrying `authorizations`, sent
/// by `sender`, calling `call_to`.
#[allow(clippy::too_many_arguments)]
fn setcode_tx_multi(
    sender: &PrivateKeySigner,
    sender_nonce: u64,
    authorizations: Vec<SignedAuthorization>,
    call_to: Address,
    calldata: Vec<u8>,
    value: u128,
    gas_limit: u64,
    chain_id: u64,
) -> ZKsyncTxEnvelope {
    let tx = TxEip7702 {
        chain_id,
        nonce: sender_nonce,
        max_fee_per_gas: DEFAULT_MAX_FEE,
        max_priority_fee_per_gas: DEFAULT_PRIORITY_FEE,
        gas_limit,
        to: call_to,
        value: AlloyU256::from(value),
        access_list: Default::default(),
        authorization_list: authorizations,
        input: calldata.into(),
    };
    ZKsyncTxEnvelope::from_eth_tx(tx, sender.clone())
}

/// A single-authorization setcode transaction: `authority` authorizes a
/// delegation to `delegate_to`, and the transaction, sent by `sender`, calls
/// `call_to`. A thin wrapper over [`setcode_tx_multi`], used by the tests (the
/// runner builds its authorization lists directly).
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
fn setcode_tx(
    sender: &PrivateKeySigner,
    sender_nonce: u64,
    authority: &PrivateKeySigner,
    authority_nonce: u64,
    delegate_to: Address,
    call_to: Address,
    calldata: Vec<u8>,
    value: u128,
    gas_limit: u64,
    chain_id: u64,
) -> ZKsyncTxEnvelope {
    setcode_tx_multi(
        sender,
        sender_nonce,
        vec![signed_auth(
            authority,
            authority_nonce,
            delegate_to,
            chain_id,
        )],
        call_to,
        calldata,
        value,
        gas_limit,
        chain_id,
    )
}

/// The 32-byte word the observation probe reads as its target: the address
/// right-aligned in the low 20 bytes, which is how CALLDATALOAD and then the code
/// and CALL opcodes consume it.
fn address_calldata(addr: Address) -> Vec<u8> {
    let mut word = vec![0u8; 32];
    word[12..32].copy_from_slice(addr.as_slice());
    word
}

/// Runtime of the observation probe: a fixed contract that reads its target
/// address from calldata and records what it observes about that account in its
/// own storage — code size (slot 0), code hash (slot 1), the first word of the
/// copied code (slot 2) and the account's balance (slot 3). Pointed at a
/// delegated EOA, these are exactly the observables where a 7702 implementation
/// can diverge: the observable code of a delegated account is its 23-byte
/// designator, `0xef0100 ‖ target`, so its code size (23), code hash
/// (`keccak256(designator)`) and copied bytes are the classic divergence points.
///
/// The probe deliberately does *not* CALL the delegated EOA. A nested CALL would
/// run the delegated code with a gas budget the 63/64 rule makes
/// gas-schedule-dependent, so a non-trivial delegated program would complete on
/// one engine and run out of gas on the other — a gas-model artifact that is not
/// masked by the forced-gas comparison (that only pins the top-level total). All
/// four observations here are pure account reads, independent of the gas
/// schedule, so the differential comparison stays sound. Calling a delegated EOA
/// from contract context is covered soundly by a dedicated unit test with a cheap
/// target instead.
fn probe_runtime() -> Vec<u8> {
    use program_gen::opcodes as op;
    const DUP4: u8 = op::DUP1 + 3;
    vec![
        // target address t = calldata[0..32]
        op::PUSH1,
        0x00,
        op::CALLDATALOAD, //
        // slot0 = EXTCODESIZE(t)
        op::DUP1,
        op::EXTCODESIZE,
        op::PUSH1,
        0x00,
        op::SSTORE, //
        // slot1 = EXTCODEHASH(t)
        op::DUP1,
        op::EXTCODEHASH,
        op::PUSH1,
        0x01,
        op::SSTORE, //
        // copy 32 bytes of t's code to memory[0..32], slot2 = first word
        op::PUSH1,
        0x20,
        op::PUSH1,
        0x00,
        op::PUSH1,
        0x00,
        DUP4,
        op::EXTCODECOPY, //
        op::PUSH1,
        0x00,
        op::MLOAD,
        op::PUSH1,
        0x02,
        op::SSTORE, //
        // slot3 = BALANCE(t)
        op::BALANCE,
        op::PUSH1,
        0x03,
        op::SSTORE, //
        op::STOP,
    ]
}

/// A deterministic secp256k1 signer for `(case_seed, role)`, so a case and its
/// deploy address reproduce byte-for-byte on replay.
fn deterministic_signer(case_seed: u64, role: u64, chain_id: u64) -> PrivateKeySigner {
    use rand::{RngCore, SeedableRng};
    use rand_chacha::ChaCha8Rng;

    let mixed = case_seed ^ role.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let mut rng = ChaCha8Rng::seed_from_u64(mixed);
    loop {
        let mut key = [0u8; 32];
        rng.fill_bytes(&mut key);
        // A random 32-byte value is a valid key with overwhelming probability;
        // the rare reject (zero or >= curve order) just draws again.
        if let Ok(signer) = PrivateKeySigner::from_str(&hex::encode(key)) {
            return signer.with_chain_id(Some(chain_id));
        }
    }
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "non-string panic payload".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zk_ee::utils::Bytes32;

    /// Proves the EIP-7702 flow actually delegates: the target's `SSTORE(0, 0x42)`
    /// must land in the *authorized EOA's* storage, which only happens if the
    /// setcode transaction set the EOA's code to the delegation designator and the
    /// call to the EOA then ran the target's code in the EOA's context. A no-op
    /// (an invalid authorization) would leave the slot empty.
    #[test]
    fn delegation_7702_runs_target_code_in_the_eoa_context() {
        // Target runtime: PUSH1 0x42; PUSH1 0x00; SSTORE; STOP.
        let runtime = vec![0x60, 0x42, 0x60, 0x00, 0x55, 0x00];
        let initcode = program_gen::wrap_runtime_as_initcode(&runtime);
        let chain_id = TEST_CHAIN_ID;
        let deployer = deterministic_signer(0xABC, DEPLOYER_ROLE, chain_id);
        let authority = deterministic_signer(0xABC, CALLER_ROLE, chain_id);

        let mut run_config = RunConfig::without_riscv_run();
        run_config.do_prover_input_run = false;
        run_config.disable_revm_consistency_check();
        let mut tester = TestingFramework::new()
            .with_system_contracts(true, true)
            .with_run_config(run_config)
            .with_block_context(block_context_for_mode(Mode::Delegation7702));
        let cap = tester.chain_config().max_tx_gas_limit();
        tester.set_balance(deployer.address(), U256::from(FUNDING_WEI));
        tester.set_balance(authority.address(), U256::from(FUNDING_WEI));

        let target = deployer.address().create(0);
        tester
            .execute_block_no_panic(vec![create_tx(&deployer, 0, cap, initcode, chain_id)])
            .expect("deploy block should not error");
        let setcode = setcode_tx(
            &deployer,
            1,
            &authority,
            0,
            target,
            authority.address(),
            Vec::new(),
            0,
            DELEGATION_DEFAULT_GAS,
            chain_id,
        );
        tester
            .execute_block_no_panic(vec![setcode])
            .expect("setcode block should not error (7702 must be supported)");

        let slot = tester.get_storage_slot(&authority.address(), U256::ZERO);
        assert_eq!(
            slot,
            Some(Bytes32::from_u256_be(&U256::from(0x42u64))),
            "the delegated EOA's slot 0 was not written by the target: 7702 delegation did not take effect"
        );
    }

    /// A forward-only tester configured for the 7702 delegation mode, matching how
    /// the runner sets one up (no RISC-V, no prover-input run, built-in REVM check
    /// off, native price zero).
    fn delegation_tester() -> TestingFramework {
        let mut run_config = RunConfig::without_riscv_run();
        run_config.do_prover_input_run = false;
        run_config.disable_revm_consistency_check();
        TestingFramework::new()
            .with_system_contracts(true, true)
            .with_run_config(run_config)
            .with_block_context(block_context_for_mode(Mode::Delegation7702))
    }

    /// The plan is a pure function of the case seed, so it must reproduce exactly —
    /// this is what lets a finding replay from its seed.
    #[test]
    fn delegation_plan_is_deterministic() {
        for seed in [0u64, 1, 2, 7, 42, 1000, 999_999, u64::MAX] {
            assert_eq!(
                DelegationPlan::derive(seed),
                DelegationPlan::derive(seed),
                "delegation plan was not deterministic for seed {seed}"
            );
        }
    }

    /// Across a spread of seeds the derivation must reach every structural target
    /// variant, or the extra coverage those variants add would never be exercised.
    #[test]
    fn delegation_plans_span_all_targets() {
        use std::collections::HashSet;
        let mut kinds = HashSet::new();
        for seed in 0..2000u64 {
            let name = match DelegationPlan::derive(seed).target {
                DelegationTarget::Deployed => "deployed",
                DelegationTarget::Precompile(_) => "precompile",
                DelegationTarget::Zero => "zero",
                DelegationTarget::SelfAddress => "self",
                DelegationTarget::Empty => "empty",
                DelegationTarget::Chained => "chained",
            };
            kinds.insert(name);
        }
        assert_eq!(
            kinds.len(),
            6,
            "not every delegation target kind was generated: {kinds:?}"
        );
    }

    /// Proves the observation probe works and that ZKsync OS exposes a delegated
    /// EOA's code as the 23-byte designator: the probe's stored code size and code
    /// hash must equal those of `0xef0100 ‖ target`.
    #[test]
    fn probe_observes_delegated_eoa_designator() {
        let chain_id = TEST_CHAIN_ID;
        let deployer = deterministic_signer(0xBEEF, DEPLOYER_ROLE, chain_id);
        let authority = deterministic_signer(0xBEEF, CALLER_ROLE, chain_id);

        let mut tester = delegation_tester();
        let cap = tester.chain_config().max_tx_gas_limit();
        tester.set_balance(deployer.address(), U256::from(FUNDING_WEI));
        tester.set_balance(authority.address(), U256::from(FUNDING_WEI));

        // A trivial STOP target program; only its designator matters here.
        let target = deployer.address().create(0);
        let probe = deployer.address().create(1);
        tester
            .execute_block_no_panic(vec![
                create_tx(
                    &deployer,
                    0,
                    cap,
                    wrap_runtime_as_initcode(&[0u8]),
                    chain_id,
                ),
                create_tx(
                    &deployer,
                    1,
                    cap,
                    wrap_runtime_as_initcode(&probe_runtime()),
                    chain_id,
                ),
            ])
            .expect("deploy block should not error");

        let setcode = setcode_tx(
            &deployer,
            2,
            &authority,
            0,
            target,
            authority.address(),
            Vec::new(),
            0,
            DELEGATION_DEFAULT_GAS,
            chain_id,
        );
        let probe_call = call_tx(
            &deployer,
            probe,
            3,
            &CallPlan {
                calldata: address_calldata(authority.address()),
                value: 0,
                gas: DELEGATION_DEFAULT_GAS,
            },
            chain_id,
        );
        tester
            .execute_block_no_panic(vec![setcode, probe_call])
            .expect("setcode + probe block should not error");

        // The delegated EOA's observable code is 0xef0100 followed by the target.
        let mut designator = vec![0xefu8, 0x01, 0x00];
        designator.extend_from_slice(target.as_slice());
        assert_eq!(designator.len(), 23);

        assert_eq!(
            tester.get_storage_slot(&probe, U256::ZERO),
            Some(Bytes32::from_u256_be(&U256::from(23u64))),
            "probe observed the wrong code size for the delegated EOA"
        );
        let hash = rig::alloy::primitives::keccak256(&designator);
        assert_eq!(
            tester.get_storage_slot(&probe, U256::from(1u64)),
            Some(Bytes32::from_u256_be(&U256::from_be_slice(hash.as_slice()))),
            "probe observed the wrong code hash for the delegated EOA"
        );
    }

    /// Proves the re-delegation/clearing step: delegating a second time to the zero
    /// address clears the EOA's code, and each valid authorization bumps the
    /// authority's account nonce.
    #[test]
    fn redelegation_to_zero_clears_code_and_bumps_nonce() {
        let chain_id = TEST_CHAIN_ID;
        let deployer = deterministic_signer(0xF00D, DEPLOYER_ROLE, chain_id);
        let authority = deterministic_signer(0xF00D, CALLER_ROLE, chain_id);

        let mut tester = delegation_tester();
        let cap = tester.chain_config().max_tx_gas_limit();
        tester.set_balance(deployer.address(), U256::from(FUNDING_WEI));
        tester.set_balance(authority.address(), U256::from(FUNDING_WEI));

        let target = deployer.address().create(0);
        tester
            .execute_block_no_panic(vec![create_tx(
                &deployer,
                0,
                cap,
                wrap_runtime_as_initcode(&[0u8]),
                chain_id,
            )])
            .expect("deploy block should not error");

        // Delegate the EOA to the target.
        let setcode = setcode_tx(
            &deployer,
            1,
            &authority,
            0,
            target,
            authority.address(),
            Vec::new(),
            0,
            DELEGATION_DEFAULT_GAS,
            chain_id,
        );
        tester
            .execute_block_no_panic(vec![setcode])
            .expect("setcode should not error");

        let props = tester.get_account_properties(&authority.address());
        assert_eq!(
            props.observable_bytecode_len, 23,
            "a delegated EOA should expose the 23-byte designator"
        );
        assert_eq!(
            props.nonce, 1,
            "a valid authorization must bump the authority's nonce"
        );

        // Re-delegate to the zero address, which clears the delegation. The
        // authority's nonce is now 1, so this authorization uses nonce 1.
        let clear = setcode_tx(
            &deployer,
            2,
            &authority,
            1,
            Address::ZERO,
            authority.address(),
            Vec::new(),
            0,
            DELEGATION_DEFAULT_GAS,
            chain_id,
        );
        tester
            .execute_block_no_panic(vec![clear])
            .expect("clearing setcode should not error");

        let props = tester.get_account_properties(&authority.address());
        assert_eq!(
            props.observable_bytecode_len, 0,
            "delegating to the zero address must clear the EOA's code"
        );
        assert_eq!(
            props.nonce, 2,
            "clearing is still a valid authorization and bumps the nonce"
        );
    }

    /// A contract that CALLs the address in its calldata, forwarding all of its
    /// gas, and stores the call's success in its own slot 0. Used to prove a
    /// delegated EOA runs its delegated code when called from *contract* context.
    #[cfg(test)]
    fn caller_runtime() -> Vec<u8> {
        use program_gen::opcodes as op;
        const DUP6: u8 = op::DUP1 + 5;
        vec![
            op::PUSH1,
            0x00,
            op::CALLDATALOAD, // t = calldata[0..32]
            op::PUSH1,
            0x00,
            op::PUSH1,
            0x00,
            op::PUSH1,
            0x00,
            op::PUSH1,
            0x00,
            op::PUSH1,
            0x00,
            DUP6,
            op::GAS,
            op::CALL, // CALL(t) forwarding all gas
            op::PUSH1,
            0x00,
            op::SSTORE, // slot0 = success
            op::STOP,
        ]
    }

    /// Distinguishes a genuine node divergence from a probe gas artifact: a
    /// contract calls a delegated EOA whose target is a *cheap* program
    /// (`SSTORE(0,0x42)`), forwarding all gas so there is no gas straddle. If
    /// ZKsync OS runs the delegated code from contract context, the write lands in
    /// the EOA's storage. (It does — which is why the soak's probe findings, where
    /// the delegated target was a full generated program, are the probe's own
    /// nested-call gas straddle, not a node bug.)
    #[test]
    fn contract_context_call_runs_delegated_code() {
        let chain_id = TEST_CHAIN_ID;
        let deployer = deterministic_signer(0xCA11, DEPLOYER_ROLE, chain_id);
        let authority = deterministic_signer(0xCA11, CALLER_ROLE, chain_id);

        let mut tester = delegation_tester();
        let cap = tester.chain_config().max_tx_gas_limit();
        tester.set_balance(deployer.address(), U256::from(FUNDING_WEI));
        tester.set_balance(authority.address(), U256::from(FUNDING_WEI));

        // Cheap target: PUSH1 0x42; PUSH1 0x00; SSTORE; STOP.
        let target_runtime = vec![0x60u8, 0x42, 0x60, 0x00, 0x55, 0x00];
        let target = deployer.address().create(0);
        let caller = deployer.address().create(1);
        tester
            .execute_block_no_panic(vec![
                create_tx(
                    &deployer,
                    0,
                    cap,
                    wrap_runtime_as_initcode(&target_runtime),
                    chain_id,
                ),
                create_tx(
                    &deployer,
                    1,
                    cap,
                    wrap_runtime_as_initcode(&caller_runtime()),
                    chain_id,
                ),
            ])
            .expect("deploy block should not error");

        // Set the delegation without calling the EOA at the top level (call a
        // neutral address), so the only thing that runs the delegated code is the
        // caller contract.
        let setcode = setcode_tx(
            &deployer,
            2,
            &authority,
            0,
            target,
            deployer.address(),
            Vec::new(),
            0,
            DELEGATION_DEFAULT_GAS,
            chain_id,
        );
        let call_caller = call_tx(
            &deployer,
            caller,
            3,
            &CallPlan {
                calldata: address_calldata(authority.address()),
                value: 0,
                gas: cap,
            },
            chain_id,
        );
        tester
            .execute_block_no_panic(vec![setcode, call_caller])
            .expect("setcode + caller block should not error");

        assert_eq!(
            tester.get_storage_slot(&caller, U256::ZERO),
            Some(Bytes32::from_u256_be(&U256::from(1u64))),
            "the contract's CALL to the delegated EOA did not succeed"
        );
        assert_eq!(
            tester.get_storage_slot(&authority.address(), U256::ZERO),
            Some(Bytes32::from_u256_be(&U256::from(0x42u64))),
            "the delegated EOA did not run its code when called from contract context"
        );
    }
}
