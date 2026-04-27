// This file contains tests that compare the blake2 implementations with the native one and circuit based one.
// They are designed to be run in riscV environment.

// First - please run the ./dump_bin.sh from test_program directory - it will compile a riscV program that will be calling
// the run_tests() method below.
// This script will produce binaries (.bin + .text) - one using native riscV blake and one using a delegation (precompile) one.

// Afterwards, you can run the tests below.

#[test]
pub fn run_naive_test() {
    let result =
        zksync_os_runner::Runner::new("src/blake2s/test_program/dist/app_native_blake".into())
            .with_cycles(1 << 25)
            .run(&[]);
    // Make sure it is successful;
    assert_eq!(result.output[0], 1);
}

#[test]
pub fn run_extended_delegation_test() {
    let result = zksync_os_runner::Runner::new(
        "src/blake2s/test_program/dist/app_extended_delegation_blake".into(),
    )
    .with_cycles(1 << 25)
    .run(&[]);
    // Make sure it is successful;
    assert_eq!(result.output[0], 1);
}
