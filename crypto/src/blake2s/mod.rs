#[cfg(not(all(feature = "single_round_with_control", target_arch = "riscv32")))]
mod naive;

#[cfg(not(all(feature = "single_round_with_control", target_arch = "riscv32")))]
pub use naive::Blake2s256;

#[cfg(all(feature = "single_round_with_control", target_arch = "riscv32"))]
mod delegated_extended;

#[cfg(all(feature = "single_round_with_control", target_arch = "riscv32"))]
pub use delegated_extended::{initialize_blake2s_delegation_context, Blake2s256};

// Multiple tests to compare delegation blake with external implementation.
// To run - please execute the run_tests inside the main workload method.
// Then compile the zksync_os (dump_bin.sh) - and run it (cargo test from zksync_os_runner)
#[cfg(feature = "blake2s_tests")]
pub mod blake2s_tests {
    pub fn run_tests() {
        test_empty();
        test_single_byte();
        test_one_different_byte();
        test_single_input();
        test_increasing();
        test_large();
    }

    // Compare delegated vs external implementations
    #[track_caller]
    fn compare_blakes(input: &[u8]) {
        use crate::MiniDigest;
        let output = crate::blake2s::Blake2s256::digest(&input);
        use crate::blake2_ext::Digest;
        let expected = crate::blake2_ext::Blake2s256::digest(&input);
        assert_eq!(&output, expected.as_slice());
    }

    pub fn test_empty() {
        let input = [];
        compare_blakes(&input);
    }

    pub fn test_single_byte() {
        let input = [1];
        compare_blakes(&input);
    }

    pub fn test_one_different_byte() {
        for i in 0..255u8 {
            let mut input = [0u8; 256];
            input[42] = i;
            compare_blakes(&input);
        }
    }

    pub fn test_single_input() {
        let input = [0, 1, 2, 3, 4, 5];
        compare_blakes(&input);
    }

    pub fn test_increasing() {
        let mut input = [0u8; 200];
        for i in 0..200 {
            input[i] = i as u8;
            compare_blakes(&input[0..i]);
        }
    }

    pub fn test_large() {
        let mut input = [0u8; 20_000];
        for i in 0..20_000 {
            input[i] = i as u8;
        }
        compare_blakes(&input);
    }
}

#[cfg(test)]
mod test;
