//! Runs the bytecode based example scenarios through the built binary.
//!
//! Each scenario must be reported as a match, which means exit code 0, and every
//! step must have run successfully. The examples that compile Solidity are left
//! out so the suite does not depend on a Foundry installation.

use std::process::Command;

fn run(path: &str) -> (Option<i32>, serde_json::Value) {
    let binary = env!("CARGO_BIN_EXE_evm-divergence-validator");
    let output = Command::new(binary)
        .arg(path)
        .output()
        .unwrap_or_else(|err| panic!("failed to run the validator on {path}: {err}"));

    let report = serde_json::from_slice(&output.stdout).unwrap_or_else(|err| {
        panic!(
            "{path} printed an invalid report: {err}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stderr)
        )
    });
    (output.status.code(), report)
}

fn assert_match(scenario: &str) {
    let path = format!("{}/examples/{}", env!("CARGO_MANIFEST_DIR"), scenario);
    let (code, report) = run(&path);

    assert_eq!(
        code,
        Some(0),
        "{scenario} was not reported as a match\n{report:#}"
    );
    for step in report["steps"].as_array().expect("report has no steps") {
        assert_eq!(
            step["success"], true,
            "{scenario}: a step did not run successfully\n{report:#}"
        );
    }
}

/// ZKsync OS rejects this step, and REVM only replays accepted transactions, so
/// nothing was compared and the scenario must not be reported as a match.
#[test]
fn rejected_step() {
    let path = format!("{}/tests/rejected_step.yaml", env!("CARGO_MANIFEST_DIR"));
    let (code, report) = run(&path);

    assert_eq!(code, Some(2), "{report:#}");
    assert_eq!(report["status"], "error", "{report:#}");
    assert!(report["steps"][0]["error"].is_string(), "{report:#}");
}

#[test]
fn predeployed_bytecode() {
    assert_match("predeployed_bytecode.yaml");
}

/// Covers the `block:` override path, which the bootloader rejects unless the
/// blob base fee is set to 1 while EIP-4844 is disabled.
#[test]
fn block_context() {
    assert_match("block_context.yaml");
}

/// Covers a block base fee above the default 1000 wei fee cap, which used to
/// get every transaction rejected.
#[test]
fn block_basefee() {
    assert_match("block_basefee.yaml");
}

/// Covers an account holding an EIP-7702 delegation designator. The designator
/// has to survive preimage trimming, reach REVM as a delegation rather than as
/// legacy code, and be hashable by the consistency checker.
#[test]
fn eip7702_delegation() {
    assert_match("eip7702_delegation.yaml");
}

/// Covers a scenario nesting frames up to the 1024 call depth limit, which
/// exceeds the default stack of the main thread.
#[test]
fn deep_call_stack() {
    assert_match("deep_call_stack.yaml");
}
