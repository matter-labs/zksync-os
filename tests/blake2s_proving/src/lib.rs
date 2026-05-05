#[cfg(test)]
mod tests {
    use riscv_transpiler::abstractions::non_determinism::QuasiUARTSource;

    #[test]
    pub fn run_naive_test() {
        let non_determinism_source = QuasiUARTSource::default();
        let results = zksync_os_runner::run(
            "tests/blake2s_test_program/app_native_blake.bin".into(),
            None,
            1 << 25,
            non_determinism_source,
        );
        assert_eq!(results[0], 1);
    }

    #[test]
    pub fn run_extended_delegation_test() {
        let non_determinism_source = QuasiUARTSource::default();
        let results = zksync_os_runner::run(
            "tests/blake2s_test_program/app_extended_delegation_blake.bin".into(),
            None,
            1 << 25,
            non_determinism_source,
        );
        assert_eq!(results[0], 1);
    }
}
