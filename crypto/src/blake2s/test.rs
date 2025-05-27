// This file contains tests that compare the blake2 implemetnations with the native one and circuit based one.
// They are designed to be run in riscV environment.

// First - please run the ./dump.bin from test_program directory - it will compile a riscV program that will be calling
// the run_tests() method below.
// This script will produce 2 binaries - one using native riscV blake and one using a delegation (precompile) one.

// Afterwards, you can run the tests below.

#[test]
pub fn run_naive_test() {
    use risc_v_simulator::abstractions::non_determinism::QuasiUARTSource;
    let non_determinism_source = QuasiUARTSource::default();
    let results = zksync_os_runner::run(
        "src/blake2s/test_program/app_native_blake.bin".into(),
        None,
        1 << 25,
        non_determinism_source,
    );
    // Make sure it is successful;
    assert_eq!(results[0], 1);
}

#[test]
pub fn run_extended_delegation_test() {
    use risc_v_simulator::abstractions::non_determinism::QuasiUARTSource;
    let non_determinism_source = QuasiUARTSource::default();
    let results = zksync_os_runner::run(
        "src/blake2s/test_program/app_extended_delegation_blake.bin".into(),
        None,
        1 << 25,
        non_determinism_source,
    );
    // Make sure it is successful;
    assert_eq!(results[0], 1);
}
