//! Runs the bytecode based example scenarios through the built binary.
//!
//! Each scenario must be reported as a match, which means exit code 0. The
//! examples that compile Solidity are left out so the suite does not depend on
//! a Foundry installation.

use std::process::Command;

fn assert_match(scenario: &str) {
    let binary = env!("CARGO_BIN_EXE_evm-divergence-validator");
    let path = format!("{}/examples/{}", env!("CARGO_MANIFEST_DIR"), scenario);

    let output = Command::new(binary)
        .arg(&path)
        .output()
        .unwrap_or_else(|err| panic!("failed to run the validator on {scenario}: {err}"));

    assert_eq!(
        output.status.code(),
        Some(0),
        "{scenario} was not reported as a match\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
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
