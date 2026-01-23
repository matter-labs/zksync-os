use crate::system_functions::keccak256::keccak256_native_cost_u64;

pub fn per_root_computational_native_cost() -> u64 {
    keccak256_native_cost_u64(96 + 32)
}
