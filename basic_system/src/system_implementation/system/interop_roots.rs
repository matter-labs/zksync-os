use crate::system_functions::keccak256::keccak256_native_cost_u64;

const INTEROP_ROOT_ROLLING_HASH_PREIMAGE_BYTES: usize = 5 * 32;

pub fn per_root_computational_native_cost() -> u64 {
    keccak256_native_cost_u64(INTEROP_ROOT_ROLLING_HASH_PREIMAGE_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interop_root_rolling_hash_charges_five_words() {
        assert_eq!(INTEROP_ROOT_ROLLING_HASH_PREIMAGE_BYTES, 160);
        assert_eq!(per_root_computational_native_cost(), 8_842);
    }
}
