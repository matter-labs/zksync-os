#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use zksync_os_runner::Runner;

    fn blake2s_dist_dir(app_name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_WORKSPACE_DIR"))
            .join("tests/blake2s/test_program/dist")
            .join(app_name)
    }

    #[test]
    pub fn run_naive_test() {
        let result = Runner::new(blake2s_dist_dir("app_native_blake"))
            .with_cycles(1 << 25)
            .run(&[]);
        assert_eq!(result.output[0], 1);
    }

    #[test]
    pub fn run_extended_delegation_test() {
        let result = Runner::new(blake2s_dist_dir("app_extended_delegation_blake"))
            .with_cycles(1 << 25)
            .run(&[]);
        assert_eq!(result.output[0], 1);
    }
}
