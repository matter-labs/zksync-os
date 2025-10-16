// First - please run the ./dump.bin from test_program directory - it will compile a riscV program that will be calling
// the run_tests() method below.
// This script will produce binaries.

// Afterwards, you can run the tests below.

#[test]
pub fn run_keccak_simple_test() {
    use risc_v_simulator::abstractions::non_determinism::QuasiUARTSource;
    let non_determinism_source = QuasiUARTSource::default();
    let results = zksync_os_runner::run(
        "src/sha3/test_program/app_keccak_simple.bin".into(),
        None,
        1 << 25,
        non_determinism_source,
    );
    // Make sure it is successful;
    assert_eq!(results[0], 1);
}

#[test]
#[should_panic]
pub fn run_keccak_bad_test() {
    use risc_v_simulator::abstractions::non_determinism::QuasiUARTSource;
    let non_determinism_source = QuasiUARTSource::default();
    let results = zksync_os_runner::run(
        "src/sha3/test_program/app_keccak_bad.bin".into(),
        None,
        1 << 25,
        non_determinism_source,
    );
    // Make sure it is successful;
    assert_eq!(results[0], 1);
}

#[test]
pub fn run_keccak_complex_test() {
    use risc_v_simulator::abstractions::non_determinism::QuasiUARTSource;
    let non_determinism_source = QuasiUARTSource::default();
    let results = zksync_os_runner::run(
        "src/sha3/test_program/app_keccak_complex.bin".into(),
        None,
        1 << 26,
        non_determinism_source,
    );
    // Make sure it is successful;
    assert_eq!(results[0], 1);
}

#[test]
pub fn run_keccak_bench_test() {
    use risc_v_simulator::abstractions::non_determinism::QuasiUARTSource;
    let non_determinism_source = QuasiUARTSource::default();
    let results = zksync_os_runner::run(
        "src/sha3/test_program/app_keccak_bench.bin".into(),
        None,
        1 << 30,
        non_determinism_source,
    );
    // Make sure it is successful;
    assert_eq!(results[0], 1);
}
