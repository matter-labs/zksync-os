// This file contains tests that compare the blake2 implementations with the native one and circuit based one.
// They are designed to be run in riscV environment.

// First - please run the ./dump_bin.sh from test_program directory - it will compile a riscV program that will be calling
// the run_tests() method below.
// This script will produce binaries (.bin + .text) - one using native riscV blake and one using a delegation (precompile) one.

// Afterwards, you can run the tests below.

use std::path::Path;

fn require_bin_and_text(bin_path: &str) -> bool {
    let text_path = Path::new(bin_path).with_extension("text");
    if !Path::new(bin_path).exists() || !text_path.exists() {
        eprintln!(
            "skipping test: {} or {} not found (run dump_bin.sh first)",
            bin_path,
            text_path.display()
        );
        return false;
    }
    true
}

#[test]
pub fn run_naive_test() {
    let bin = "src/blake2s/test_program/app_native_blake.bin";
    if !require_bin_and_text(bin) {
        return;
    }
    use riscv_transpiler::abstractions::non_determinism::QuasiUARTSource;
    let non_determinism_source = QuasiUARTSource::default();
    let results = zksync_os_runner::run(bin.into(), 1 << 25, non_determinism_source);
    // Make sure it is successful;
    assert_eq!(results[0], 1);
}

#[test]
pub fn run_extended_delegation_test() {
    let bin = "src/blake2s/test_program/app_extended_delegation_blake.bin";
    if !require_bin_and_text(bin) {
        return;
    }
    use riscv_transpiler::abstractions::non_determinism::QuasiUARTSource;
    let non_determinism_source = QuasiUARTSource::default();
    let results = zksync_os_runner::run(bin.into(), 1 << 25, non_determinism_source);
    // Make sure it is successful;
    assert_eq!(results[0], 1);
}
