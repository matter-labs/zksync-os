#[cfg(test)]
mod tests {
    use zksync_os_runner::Runner;

    #[test]
    pub fn run_naive_test() {
        let result = Runner::new("tests/blake2s_test_program/app_native_blake".into())
            .with_cycles(1 << 25)
            .run(&[]);
        assert_eq!(result.output[0], 1);
    }

    #[test]
    pub fn run_extended_delegation_test() {
        let result = Runner::new("tests/blake2s_test_program/app_extended_delegation_blake".into())
            .with_cycles(1 << 25)
            .run(&[]);
        assert_eq!(result.output[0], 1);
    }
}
